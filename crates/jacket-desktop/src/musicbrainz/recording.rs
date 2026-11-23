//! Recording detail and validation of a song's audio source.
use super::{link, request, validate_id, Album, Relation, Song};
use audio_engine::{Cancellation, Inspection};
use jacket_core::{AlbumRelease, ReleaseTrack, SourceLink};
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct SongDetails {
    pub song: Song,
    pub albums: Vec<Album>,
    pub sources: Vec<String>,
}

#[derive(Deserialize)]
struct Response {
    #[serde(flatten)]
    song: Song,
    #[serde(default)]
    relations: Vec<Relation>,
}

pub(crate) fn recording_details(
    selected: Song,
    cancel: &Cancellation,
) -> Result<SongDetails, String> {
    validate_id(&selected.id)?;
    let response: Response = request(
        &format!("recording/{}", selected.id),
        &[
            ("inc", "artist-credits+releases+release-groups+url-rels"),
            ("status", "official"),
        ],
        cancel,
    )?;
    Ok(from_response(response, selected))
}

fn from_response(response: Response, selected: Song) -> SongDetails {
    let Response { song, relations } = response;
    let mut groups = HashSet::new();
    let mut releases: Vec<_> = song.releases.iter().chain(&selected.releases).collect();
    releases.sort_by_cached_key(|release| {
        let group = release.release_group.as_ref();
        let album = group.and_then(|group| group.primary_type.as_deref()) == Some("Album");
        let compilation = group.is_some_and(|group| {
            group
                .secondary_types
                .iter()
                .any(|kind| kind == "Compilation")
        });
        (
            compilation,
            !album,
            release.date.is_none(),
            release.date.as_deref(),
        )
    });
    let albums = releases
        .into_iter()
        .filter(|release| {
            release
                .status
                .as_deref()
                .is_none_or(|status| status == "Official")
        })
        .filter_map(|release| release.release_group.as_ref())
        .filter(|group| groups.insert(group.id.as_str()))
        .take(12)
        .cloned()
        .collect();
    let mut videos = HashSet::new();
    let sources = relations
        .iter()
        .filter(|relation| matches!(relation.kind.as_str(), "free streaming" | "streaming"))
        .filter_map(link)
        .filter_map(|url| SourceLink::parse(url).ok()?.song_url)
        .filter(|url| videos.insert(video_id(url).to_owned()))
        .take(8)
        .collect();
    SongDetails {
        song,
        albums,
        sources,
    }
}

fn video_id(url: &str) -> &str {
    url.rsplit_once("?v=").map_or("", |(_, id)| id)
}

impl SongDetails {
    pub(crate) fn match_source(&self, mut inspection: Inspection) -> Result<Inspection, String> {
        let [track] = inspection.tracks.as_slice() else {
            return Err("Choose a YouTube link for a single song.".into());
        };
        let linked = self
            .sources
            .iter()
            .any(|source| video_id(source) == track.id);
        if !linked && track.source_metadata.artist.trim().is_empty() {
            return Err(
                "This link has no artist credit to verify the recording. Try another source."
                    .into(),
            );
        }
        let artist = self.song.artist();
        let candidate = AlbumRelease {
            title: self.song.title.clone(),
            artist: artist.clone(),
            year: None,
            tracks: vec![ReleaseTrack {
                title: self.song.title.clone(),
                artist: artist.clone(),
                disc: 1,
                number: 1,
                duration_ms: self.song.length,
                source_urls: self.sources.clone(),
            }],
        };
        if candidate
            .match_source(inspection.clone())?
            .inspection
            .tracks
            .is_empty()
        {
            return Err("The source could not be confirmed as this recording. Check the artist and version or try another link.".into());
        }
        inspection.title.clone_from(&self.song.title);
        let track = &mut inspection.tracks[0];
        track.metadata.title.clone_from(&self.song.title);
        if !artist.is_empty() {
            track.metadata.artist = artist;
        }
        Ok(inspection)
    }
}

#[cfg(test)]
mod tests {
    use super::{from_response, Response, Song};
    use audio_engine::source::inspection_from_json;
    use serde_json::json;

    #[test]
    fn song_detail_keeps_official_album_groups_and_canonical_single_video_links() {
        let response: Response = serde_json::from_value(json!({
            "id":"recording", "title":"Song", "length":202000,
            "artist-credit":[{"name":"Artist"}],
            "releases":[
                {"title":"The album", "status":"Official",
                 "release-group":{"id":"group","title":"The album","primary-type":"Album"}},
                {"title":"Another edition", "status":"Official",
                 "release-group":{"id":"group","title":"The album","primary-type":"Album"}},
                {"title":"Bootleg", "status":"Bootleg", "release-group":{"id":"bootleg","title":"Bootleg"}}
            ],
            "relations":[
                {"type":"free streaming","url":{"resource":"https://youtu.be/fixture0001"}},
                {"type":"free streaming","url":{"resource":"https://music.youtube.com/watch?v=fixture0001"}},
                {"type":"free streaming","url":{"resource":"https://music.youtube.com/playlist?list=OLAK5uy_fixture123"}},
                {"type":"free streaming","ended":true,"url":{"resource":"https://youtu.be/fixture0002"}}
            ]
        })).unwrap();
        let selected = response.song.clone();
        let details = from_response(response, selected);
        assert_eq!(details.albums.len(), 1);
        assert_eq!(
            details.sources,
            ["https://www.youtube.com/watch?v=fixture0001"]
        );
        let inspection = |duration| {
            inspection_from_json(
                &json!({"id":"fixture0001","title":"Display title","duration":duration}),
                "https://www.youtube.com/watch?v=fixture0001",
            )
            .unwrap()
        };
        let checked = details.match_source(inspection(202)).unwrap();
        assert_eq!(checked.tracks[0].metadata.title, "Song");
        assert_eq!(checked.tracks[0].source_metadata.title, "Display title");
        assert!(details.match_source(inspection(180)).is_err());
    }

    #[test]
    fn a_pasted_song_with_a_different_artist_is_rejected() {
        let song: Song = serde_json::from_value(json!({
            "id":"recording","title":"Song","length":202000,
            "artist-credit":[{"name":"Artist"}]
        }))
        .unwrap();
        let details = super::SongDetails {
            song,
            albums: vec![],
            sources: vec![],
        };
        let inspection = inspection_from_json(
            &json!({"id":"fixture0001","title":"Song","artist":"Another Artist","duration":202}),
            "https://www.youtube.com/watch?v=fixture0001",
        )
        .unwrap();
        assert!(details.match_source(inspection).is_err());
    }
}
