//! Public YouTube Music album discovery for the local development build.
use audio_engine::{Cancellation, CollectionMode, EngineError, Inspection, Metadata, Track};
use jacket_core::SourceLink;
use serde::{de::DeserializeOwned, Deserialize};
use std::{collections::HashSet, path::PathBuf, process::Command, time::Duration};

#[derive(Clone, Debug, Deserialize)]
pub struct Album {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub year: Option<u32>,
    pub kind: String,
    pub artwork_url: Option<String>,
}

#[derive(Deserialize)]
struct AlbumDetail {
    title: String,
    artist: String,
    year: Option<u32>,
    artwork_url: Option<String>,
    playlist_id: Option<String>,
    track_count: Option<usize>,
    tracks: Vec<AlbumTrack>,
}

#[derive(Deserialize)]
struct AlbumTrack {
    id: Option<String>,
    title: String,
    artist: String,
    duration: Option<u64>,
    available: bool,
}

pub fn enabled() -> bool {
    python().is_some()
}

fn python() -> Option<PathBuf> {
    std::env::var_os("JACKET_YOUTUBE_MUSIC_PYTHON")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

fn request<T: DeserializeOwned>(
    operation: &str,
    value: &str,
    cancel: &Cancellation,
) -> Result<T, String> {
    let python = python()
        .filter(|path| path.is_file())
        .ok_or("Set up YouTube Music with mise run setup:youtube-music, then try again.")?;
    let mut command = Command::new(python);
    command.args([
        "-I",
        "-B",
        "-c",
        include_str!("youtube_music.py"),
        operation,
        value,
    ]);
    let bytes = audio_engine::process::run(&mut command, cancel, Duration::from_secs(12)).map_err(
        |error| match error {
            EngineError::Tool(message) if !message.is_empty() => message,
            EngineError::Cancelled => "YouTube Music lookup cancelled.".into(),
            EngineError::Invalid(_) => "YouTube Music took too long to respond. Try again.".into(),
            _ => {
                "YouTube Music did not complete the request. Try again or open it in your browser."
                    .into()
            }
        },
    )?;
    serde_json::from_slice(&bytes)
        .map_err(|_| "YouTube Music returned unreadable album data.".into())
}

pub(crate) fn search(query: &str, cancel: &Cancellation) -> Result<Vec<Album>, String> {
    if query.trim().is_empty() || query.chars().count() > 200 {
        return Err("Enter an album or artist name up to 200 characters.".into());
    }
    let mut results: Vec<Album> = request("search", query, cancel)?;
    let mut ids = HashSet::new();
    results.retain_mut(|album| {
        discard_untrusted_artwork(&mut album.artwork_url);
        valid_album_id(&album.id) && !album.title.trim().is_empty() && ids.insert(album.id.clone())
    });
    Ok(results)
}

pub(crate) fn album(id: &str, cancel: &Cancellation) -> Result<Inspection, String> {
    if !valid_album_id(id) {
        return Err("YouTube Music returned an invalid album identifier.".into());
    }
    let detail = request("album", id, cancel)?;
    inspection(id, detail)
}

fn valid_album_id(id: &str) -> bool {
    id.strip_prefix("MPRE").is_some_and(|suffix| {
        !suffix.is_empty()
            && id.len() <= 100
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    })
}

fn discard_untrusted_artwork(url: &mut Option<String>) {
    if url
        .as_deref()
        .is_some_and(|url| !audio_engine::source::valid_artwork_url(url))
    {
        *url = None;
    }
}

fn inspection(id: &str, mut detail: AlbumDetail) -> Result<Inspection, String> {
    if detail.title.trim().is_empty() {
        return Err("This YouTube Music album has no title.".into());
    }
    discard_untrusted_artwork(&mut detail.artwork_url);
    let source_url = detail
        .playlist_id
        .filter(|id| id.starts_with("OLAK5uy"))
        .and_then(|id| {
            let url = format!("https://music.youtube.com/playlist?list={id}");
            let link = SourceLink::parse(&url).ok()?;
            (link.playlist_url.as_deref() == Some(url.as_str())).then_some(url)
        })
        .unwrap_or_else(|| format!("https://music.youtube.com/browse/{id}"));
    let expected = detail
        .track_count
        .unwrap_or_default()
        .max(detail.tracks.len());
    let tracks: Vec<_> = detail
        .tracks
        .into_iter()
        .enumerate()
        .filter_map(|(index, track)| {
            let id = track.id?;
            if id.len() != 11 || !track.available || track.title.trim().is_empty() {
                return None;
            }
            let source_url = SourceLink::parse(&format!("https://music.youtube.com/watch?v={id}"))
                .ok()?
                .song_url?;
            let metadata = Metadata {
                title: track.title,
                artist: if track.artist.trim().is_empty() {
                    detail.artist.clone()
                } else {
                    track.artist
                },
                album: detail.title.clone(),
                album_artist: detail.artist.clone(),
                track_number: u32::try_from(index + 1).ok(),
                disc_number: None,
                release_year: detail.year,
            };
            Some(Track {
                id,
                source_url,
                uploader: String::new(),
                duration: track.duration.map(|seconds| seconds as f64),
                artwork_url: detail.artwork_url.clone(),
                source_metadata: metadata.clone(),
                metadata,
            })
        })
        .collect();
    if tracks.is_empty() {
        return Err("YouTube Music did not return any available tracks for this edition.".into());
    }
    let warnings = (expected > tracks.len())
        .then(|| {
            format!(
                "YouTube Music returned {} of {expected} tracks for this edition.",
                tracks.len()
            )
        })
        .into_iter()
        .collect();
    Ok(Inspection {
        title: detail.title,
        source_url,
        suggested_mode: CollectionMode::Album,
        artwork_url: detail.artwork_url,
        album_release_year: detail.year,
        tracks,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::{inspection, AlbumDetail, AlbumTrack};
    use audio_engine::CollectionMode;

    fn edition(track_count: usize) -> AlbumDetail {
        AlbumDetail {
            title: "Room to breathe (Expanded Edition)".into(),
            artist: "The Paper Trails".into(),
            year: Some(2024),
            artwork_url: None,
            playlist_id: Some("OLAK5uy_fixture".into()),
            track_count: Some(track_count),
            tracks: (1..=track_count)
                .map(|number| AlbumTrack {
                    // Reused video IDs still represent separate occurrences.
                    id: Some("abcdefghijk".into()),
                    title: format!("Recording {number}"),
                    artist: String::new(),
                    duration: Some(120),
                    available: true,
                })
                .collect(),
        }
    }

    #[test]
    fn album_inspection_keeps_all_twenty_ordered_recording_occurrences() {
        let album = inspection("MPREb_fixture", edition(20)).unwrap();
        assert_eq!(album.tracks.len(), 20);
        assert_eq!(album.suggested_mode, CollectionMode::Album);
        assert!(album.warnings.is_empty());
        assert!(album.source_url.ends_with("list=OLAK5uy_fixture"));
        for (index, track) in album.tracks.iter().enumerate() {
            assert_eq!(track.metadata.track_number, Some(index as u32 + 1));
            assert_eq!(track.metadata.title, format!("Recording {}", index + 1));
            assert_eq!(track.metadata.album, album.title);
            assert_eq!(track.metadata.artist, "The Paper Trails");
            assert_eq!(track.metadata.release_year, Some(2024));
        }
    }

    #[test]
    fn unavailable_tracks_are_reported_without_shifting_track_numbers() {
        let mut detail = edition(3);
        detail.tracks[1].available = false;
        let album = inspection("MPREb_fixture", detail).unwrap();
        assert_eq!(album.tracks.len(), 2);
        assert_eq!(album.tracks[1].metadata.track_number, Some(3));
        assert_eq!(
            album.warnings,
            ["YouTube Music returned 2 of 3 tracks for this edition."]
        );
    }
}
