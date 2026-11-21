//! Optional exact-song album artwork from the public YouTube Music queue.
//! A video thumbnail remains the fallback when music metadata is unavailable.
use crate::{Cancellation, Inspection};
use serde_json::{json, Value};
use std::{io::Read, time::Duration};

pub(crate) struct SongArtwork {
    video_id: String,
    url: String,
}

fn from_queue(value: &Value, video_id: &str) -> Option<SongArtwork> {
    // Restrict lookup to the requested queue, never recommendation shelves,
    // menus, or a recursively found image belonging to a different recording.
    let contents = value.pointer("/contents/singleColumnMusicWatchNextResultsRenderer/tabbedRenderer/watchNextTabbedResultsRenderer/tabs/0/tabRenderer/content/musicQueueRenderer/content/playlistPanelRenderer/contents")?.as_array()?;
    let mut songs = contents
        .iter()
        .filter_map(|item| item.get("playlistPanelVideoRenderer"))
        .filter(|song| song.get("videoId").and_then(Value::as_str) == Some(video_id));
    let song = songs.next()?;
    if songs.next().is_some() {
        return None;
    }
    // Only an album-linked song row establishes album art. Ordinary video rows
    // may have square channel images, which must not become an album cover.
    let album_link = song.pointer("/longBylineText/runs")?.as_array()?.iter().any(|run| {
        run.pointer("/navigationEndpoint/browseEndpoint/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType").and_then(Value::as_str)
            == Some("MUSIC_PAGE_TYPE_ALBUM")
    });
    if !album_link {
        return None;
    }
    let cover = song
        .pointer("/thumbnail/thumbnails")?
        .as_array()?
        .iter()
        .filter(|image| {
            let width = image.get("width").and_then(Value::as_u64).unwrap_or(0);
            let height = image.get("height").and_then(Value::as_u64).unwrap_or(0);
            width > 0
                && width == height
                && image
                    .get("url")
                    .and_then(Value::as_str)
                    .is_some_and(crate::source::valid_artwork_url)
        })
        .max_by_key(|image| image.get("width").and_then(Value::as_u64).unwrap_or(0))?;
    Some(SongArtwork {
        video_id: video_id.to_owned(),
        url: cover.get("url")?.as_str()?.to_owned(),
    })
}

pub(crate) fn lookup(input: &str, cancel: &Cancellation) -> Option<SongArtwork> {
    let input = crate::source::validate_source_url(input).ok()?;
    let url = url::Url::parse(&input).ok()?;
    let video_id = if url.host_str() == Some("youtu.be") {
        url.path().strip_prefix('/')?.to_owned()
    } else {
        url.query_pairs()
            .find(|(key, _)| key == "v")?
            .1
            .into_owned()
    };
    if video_id.len() != 11
        || !video_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return None;
    }
    cancel.check().ok()?;
    let body = json!({"context":{"client":{"clientName":"WEB_REMIX", "clientVersion":"1.20261012.01.00", "hl":"en"}}, "videoId": video_id});
    let response = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(4))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .ok()?
        .post("https://music.youtube.com/youtubei/v1/next?prettyPrint=false")
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(serde_json::to_vec(&body).ok()?)
        .send()
        .ok()?
        .error_for_status()
        .ok()?;
    let mut bytes = Vec::new();
    response
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    cancel.check().ok()?;
    if bytes.len() > 4 * 1024 * 1024 {
        return None;
    }
    from_queue(&serde_json::from_slice::<Value>(&bytes).ok()?, &video_id)
}

pub(crate) fn apply(inspection: &mut Inspection, artwork: SongArtwork) {
    if inspection.tracks.len() != 1 || inspection.tracks[0].id != artwork.video_id {
        return;
    }
    inspection.tracks[0].artwork_url = Some(artwork.url.clone());
    inspection.artwork_url = Some(artwork.url);
    inspection
        .warnings
        .retain(|warning| warning != crate::source::ARTWORK_WARNING);
}

#[cfg(test)]
mod tests {
    use super::{apply, from_queue};
    use crate::source::inspection_from_json;
    use serde_json::{json, Value};

    fn queue() -> Value {
        let song = json!({"playlistPanelVideoRenderer":{
            "videoId":"abc12345678",
            "longBylineText":{"runs":[{"text":"Album", "navigationEndpoint":{"browseEndpoint":{"browseEndpointContextSupportedConfigs":{"browseEndpointContextMusicConfig":{"pageType":"MUSIC_PAGE_TYPE_ALBUM"}}}}}]},
            "thumbnail":{"thumbnails":[
                {"url":"https://lh3.googleusercontent.com/small", "width":60,"height":60},
                {"url":"https://lh3.googleusercontent.com/cover", "width":544,"height":544},
                {"url":"https://i.ytimg.com/video.jpg", "width":1920,"height":1080}
            ]}
        }});
        json!({"contents":{"singleColumnMusicWatchNextResultsRenderer":{"tabbedRenderer":{"watchNextTabbedResultsRenderer":{"tabs":[{"tabRenderer":{"content":{"musicQueueRenderer":{"content":{"playlistPanelRenderer":{"contents":[song]}}}}}}]}}}}})
    }

    #[test]
    fn exact_album_linked_song_uses_its_largest_square_cover() {
        let artwork = from_queue(&queue(), "abc12345678").expect("album cover");
        assert_eq!(artwork.url, "https://lh3.googleusercontent.com/cover");
        assert!(from_queue(&queue(), "other123456").is_none());
        let mut no_album = queue();
        *no_album.pointer_mut("/contents/singleColumnMusicWatchNextResultsRenderer/tabbedRenderer/watchNextTabbedResultsRenderer/tabs/0/tabRenderer/content/musicQueueRenderer/content/playlistPanelRenderer/contents/0/playlistPanelVideoRenderer/longBylineText/runs").unwrap() = json!([]);
        assert!(from_queue(&no_album, "abc12345678").is_none());
        // An image in an unrelated recommendation must never fill a missing queue.
        assert!(from_queue(&json!({"recommendations":queue()}), "abc12345678").is_none());
    }

    #[test]
    fn enrichment_changes_only_the_matching_single_song_artwork() {
        let entry = json!({"id":"abc12345678", "title":"Song", "album":"Album", "release_year":1960, "thumbnail":"https://i.ytimg.com/video.webp"});
        let mut inspection =
            inspection_from_json(&entry, "https://music.youtube.com/watch?v=abc12345678").unwrap();
        let metadata = inspection.tracks[0].metadata.clone();
        apply(
            &mut inspection,
            from_queue(&queue(), "abc12345678").unwrap(),
        );
        assert_eq!(
            inspection.artwork_url.as_deref(),
            Some("https://lh3.googleusercontent.com/cover")
        );
        assert_eq!(inspection.artwork_url, inspection.tracks[0].artwork_url);
        assert_eq!(
            serde_json::to_value(&inspection.tracks[0].metadata).unwrap(),
            serde_json::to_value(metadata).unwrap()
        );
        assert!(inspection.warnings.is_empty());
        let mut collection = inspection_from_json(
            &json!({"title":"Mix", "entries":[entry.clone(),entry]}),
            "https://www.youtube.com/playlist?list=PLmix",
        )
        .unwrap();
        apply(
            &mut collection,
            from_queue(&queue(), "abc12345678").unwrap(),
        );
        assert_ne!(collection.artwork_url, inspection.artwork_url);
    }
}
