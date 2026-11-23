//! Public YouTube Music discovery for the local development build.
use audio_engine::{Cancellation, CollectionMode, EngineError, Inspection, Metadata, Track};
use jacket_core::SourceLink;
use serde::{de::DeserializeOwned, Deserialize};
use std::{collections::HashSet, path::PathBuf, process::Command, time::Duration};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub enum SearchKind {
    Artists,
    Albums,
    Songs,
}

impl SearchKind {
    pub const ALL: [Self; 3] = [Self::Artists, Self::Albums, Self::Songs];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Artists => "Artists",
            Self::Albums => "Albums",
            Self::Songs => "Songs",
        }
    }

    fn filter(self) -> &'static str {
        match self {
            Self::Artists => "artists",
            Self::Albums => "albums",
            Self::Songs => "songs",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct SearchHit {
    pub featured: bool,
    #[serde(flatten)]
    pub item: SearchItem,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "result_type", rename_all = "lowercase")]
pub enum SearchItem {
    Artist(Artist),
    Album(Album),
    Song(Song),
}

impl SearchItem {
    pub fn id(&self) -> &str {
        match self {
            Self::Artist(artist) => &artist.id,
            Self::Album(album) => &album.id,
            Self::Song(song) => &song.id,
        }
    }

    pub const fn kind(&self) -> SearchKind {
        match self {
            Self::Artist(_) => SearchKind::Artists,
            Self::Album(_) => SearchKind::Albums,
            Self::Song(_) => SearchKind::Songs,
        }
    }

    pub fn artwork_url(&self) -> Option<&str> {
        match self {
            Self::Artist(artist) => artist.artwork_url.as_deref(),
            Self::Album(album) => album.artwork_url.as_deref(),
            Self::Song(song) => song.artwork_url.as_deref(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub artwork_url: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ArtistPage {
    pub artist: Artist,
    pub hits: Vec<SearchHit>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Song {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: Option<u64>,
    pub artwork_url: Option<String>,
    available: bool,
}

impl Song {
    pub fn needs_details(&self) -> bool {
        self.artist.trim().is_empty() || self.album.trim().is_empty()
    }

    fn fill_details(&mut self, detail: Song) {
        if self.id != detail.id || !detail.available {
            return;
        }
        if self.artist.trim().is_empty() {
            self.artist = detail.artist;
        }
        if self.album.trim().is_empty() {
            self.album = detail.album;
        }
        self.duration = self.duration.or(detail.duration);
        self.artwork_url = self.artwork_url.take().or(detail.artwork_url);
    }

    pub fn inspection(&self) -> Result<Inspection, String> {
        let source_url = song_url(&self.id)
            .filter(|_| self.available && !self.title.trim().is_empty())
            .ok_or("YouTube Music did not return an available recording.")?;
        let mut artwork_url = self.artwork_url.clone();
        discard_untrusted_artwork(&mut artwork_url);
        let metadata = Metadata {
            title: self.title.clone(),
            artist: self.artist.clone(),
            album: self.album.clone(),
            ..Metadata::default()
        };
        let track = Track {
            id: self.id.clone(),
            source_url: source_url.clone(),
            uploader: String::new(),
            duration: self.duration.map(|seconds| seconds as f64),
            artwork_url: artwork_url.clone(),
            source_metadata: metadata.clone(),
            metadata,
        };
        Ok(Inspection {
            title: self.title.clone(),
            source_url,
            suggested_mode: CollectionMode::Collection,
            artwork_url,
            album_release_year: None,
            tracks: vec![track],
            warnings: Vec::new(),
        })
    }
}

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
    filter: Option<SearchKind>,
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
        filter.map_or("", SearchKind::filter),
    ]);
    let timeout = Duration::from_secs(if operation == "song" { 6 } else { 12 });
    let bytes =
        audio_engine::process::run(&mut command, cancel, timeout).map_err(|error| match error {
            EngineError::Tool(message) if !message.is_empty() => message,
            EngineError::Cancelled => "YouTube Music lookup cancelled.".into(),
            EngineError::Invalid(_) => "YouTube Music took too long to respond. Try again.".into(),
            _ => {
                "YouTube Music did not complete the request. Try again or open it in your browser."
                    .into()
            }
        })?;
    serde_json::from_slice(&bytes)
        .map_err(|_| "YouTube Music returned unreadable catalog data.".into())
}

pub(crate) fn search(
    query: &str,
    filter: Option<SearchKind>,
    cancel: &Cancellation,
) -> Result<Vec<SearchHit>, String> {
    if query.trim().is_empty() || query.chars().count() > 200 {
        return Err("Enter an artist, album, or song name up to 200 characters.".into());
    }
    let mut results = request("search", query, filter, cancel)?;
    validate_hits(&mut results);
    Ok(results)
}

fn validate_hits(results: &mut Vec<SearchHit>) {
    let mut ids = HashSet::new();
    results.retain_mut(|hit| {
        let valid = match &mut hit.item {
            SearchItem::Artist(artist) => {
                discard_untrusted_artwork(&mut artist.artwork_url);
                valid_artist_id(&artist.id) && !artist.name.trim().is_empty()
            }
            SearchItem::Album(album) => {
                discard_untrusted_artwork(&mut album.artwork_url);
                valid_album_id(&album.id) && !album.title.trim().is_empty()
            }
            SearchItem::Song(song) => {
                discard_untrusted_artwork(&mut song.artwork_url);
                song.available && !song.title.trim().is_empty() && song_url(&song.id).is_some()
            }
        };
        valid && ids.insert((hit.item.kind(), hit.item.id().to_owned()))
    });
}

pub(crate) fn artist(id: &str, cancel: &Cancellation) -> Result<ArtistPage, String> {
    if !valid_artist_id(id) {
        return Err("YouTube Music returned an invalid artist identifier.".into());
    }
    let mut page: ArtistPage = request("artist", id, None, cancel)?;
    if page.artist.id != id || page.artist.name.trim().is_empty() {
        return Err("YouTube Music did not return the requested artist.".into());
    }
    discard_untrusted_artwork(&mut page.artist.artwork_url);
    validate_hits(&mut page.hits);
    Ok(page)
}

pub(crate) fn album(id: &str, cancel: &Cancellation) -> Result<Inspection, String> {
    if !valid_album_id(id) {
        return Err("YouTube Music returned an invalid album identifier.".into());
    }
    let detail = request("album", id, None, cancel)?;
    inspection(id, detail)
}

pub(crate) fn song(mut song: Song, cancel: &Cancellation) -> Result<Inspection, String> {
    if song_url(&song.id).is_none() {
        return Err("YouTube Music returned an invalid recording identifier.".into());
    }
    if let Ok(Some(detail)) = request::<Option<Song>>("song", &song.id, None, cancel) {
        song.fill_details(detail);
    }
    cancel.check().map_err(|error| error.to_string())?;
    song.inspection()
}

fn valid_artist_id(id: &str) -> bool {
    id.starts_with("UC")
        && id.len() == 24
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn song_url(id: &str) -> Option<String> {
    if id.len() != 11 {
        return None;
    }
    SourceLink::parse(&format!("https://music.youtube.com/watch?v={id}"))
        .ok()?
        .song_url
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
            if !track.available || track.title.trim().is_empty() {
                return None;
            }
            let source_url = song_url(&id)?;
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
    use super::{
        inspection, validate_hits, AlbumDetail, AlbumTrack, SearchHit, SearchItem, SearchKind,
    };
    use audio_engine::CollectionMode;
    use serde_json::json;

    #[test]
    fn mixed_results_keep_provider_order_and_validate_song_review_identity() {
        let song = json!({
            "result_type": "song", "featured": false, "id": "abcdefghijk",
            "title": "First Light", "artist": "The Paper Trails", "album": "Room to breathe",
            "duration": 192, "available": true, "artwork_url": "https://other.example/cover.jpg"
        });
        let mut hits: Vec<SearchHit> = serde_json::from_value(json!([
            {"result_type": "artist", "featured": true, "id": "UCtest000000000000000001",
                "name": "The Paper Trails"},
            {"result_type": "album", "featured": false, "id": "MPREb_example",
                "title": "Room to breathe", "artist": "The Paper Trails", "year": 2024,
                "kind": "Album"},
            song.clone(),
            song
        ]))
        .unwrap();
        validate_hits(&mut hits);
        assert_eq!(
            hits.iter().map(|hit| hit.item.kind()).collect::<Vec<_>>(),
            [SearchKind::Artists, SearchKind::Albums, SearchKind::Songs]
        );
        assert!(hits[0].featured);
        let SearchItem::Song(song) = &hits[2].item else {
            panic!("expected a song result")
        };
        let review = song.inspection().unwrap();
        assert_eq!(review.suggested_mode, CollectionMode::Collection);
        assert_eq!(review.tracks.len(), 1);
        assert_eq!(
            review.source_url,
            "https://music.youtube.com/watch?v=abcdefghijk"
        );
        assert_eq!(review.tracks[0].metadata.album, "Room to breathe");
        assert_eq!(review.tracks[0].duration, Some(192.0));
        assert!(review.artwork_url.is_none());
    }

    #[test]
    fn details_fill_missing_tags_only_for_the_exact_recording() {
        let mut song: super::Song = serde_json::from_value(json!({
            "id": "abcdefghijk", "title": "First Light", "artist": "",
            "album": "", "available": true,
        }))
        .unwrap();
        let mut details = song.clone();
        details.id = "lmnopqrstuv".into();
        details.title = "First Light (Live)".into();
        details.album = "Room to breathe".into();
        details.artist = "The Paper Trails".into();
        song.fill_details(details.clone());
        assert!(song.needs_details());
        details.id.clone_from(&song.id);
        song.fill_details(details);
        let result = song.inspection().unwrap();
        assert_eq!(result.title, "First Light");
        assert_eq!(result.tracks[0].metadata.artist, "The Paper Trails");
        assert_eq!(result.tracks[0].metadata.album, "Room to breathe");
    }

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
