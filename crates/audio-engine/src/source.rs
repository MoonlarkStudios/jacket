use crate::{
    models::*,
    process::{run_lines, Cancellation, Toolchain},
    EngineError, Result,
};
use serde_json::Value;
use std::{process::Command, time::Duration};

pub fn validate_source_url(input: &str) -> Result<String> {
    let url = url::Url::parse(input.trim())
        .map_err(|_| EngineError::Invalid("Enter a valid YouTube or YouTube Music URL.".into()))?;
    let supported = matches!(
        url.host_str(),
        Some(
            "youtube.com" | "www.youtube.com" | "music.youtube.com" | "m.youtube.com" | "youtu.be"
        )
    );
    if url.scheme() != "https"
        || !supported
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return Err(EngineError::Invalid(
            "Use an HTTPS YouTube or YouTube Music link.".into(),
        ));
    }
    Ok(url.to_string())
}

pub fn downloader(tools: &Toolchain, auth: &AuthOptions) -> Result<Command> {
    let mut cmd = Command::new(&tools.yt_dlp);
    cmd.args(["--ignore-config", "--no-progress"]);
    if auth.managed_session.is_some() && (auth.cookie_file.is_none() || auth.browser.is_some()) {
        return Err(EngineError::Invalid(
            "Reconnect the Jacket YouTube session before continuing.".into(),
        ));
    }
    if let Some(file) = &auth.cookie_file {
        cmd.arg("--cookies").arg(file);
    }
    if let Some(browser) = &auth.browser {
        let profile = auth.browser_profile_path.as_ref().ok_or_else(|| EngineError::Invalid(
            "Choose your browser profile in Session settings before continuing. Jacket will not select a different account automatically.".into(),
        ))?;
        let path = profile.to_str().filter(|value| !value.contains("::") && !value.chars().any(char::is_control))
            .ok_or_else(|| EngineError::Invalid("This browser profile path is not supported. Choose another profile in Session settings.".into()))?;
        let mut specification = format!("{}:{path}", browser.as_str());
        if *browser == Browser::Firefox {
            let container = auth.browser_container.as_deref().unwrap_or("none");
            if container.contains("::") || container.chars().any(char::is_control) {
                return Err(EngineError::Invalid(
                    "Choose a supported Firefox container in Session settings.".into(),
                ));
            }
            specification.push_str(&format!("::{container}"));
        }
        cmd.args(["--cookies-from-browser", &specification]);
    }
    if let Some(node) = &tools.node {
        cmd.arg("--js-runtimes")
            .arg(format!("node:{}", node.display()));
    }
    Ok(cmd)
}

fn string(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned()
}

fn first(value: &Value, keys: &[&str]) -> String {
    keys.iter()
        .map(|key| string(value, key))
        .find(|s| !s.is_empty())
        .unwrap_or_default()
}

pub fn artwork_url(value: &Value) -> Option<String> {
    value
        .get("thumbnails")
        .and_then(Value::as_array)
        .and_then(|images| {
            images.iter().rev().find_map(|image| {
                let candidate = string(image, "url");
                usable_artwork_candidate(&candidate).then_some(candidate)
            })
        })
        .or_else(|| {
            let candidate = string(value, "thumbnail");
            usable_artwork_candidate(&candidate).then_some(candidate)
        })
}

fn usable_artwork_candidate(input: &str) -> bool {
    if !valid_artwork_url(input) {
        return false;
    }
    let url = url::Url::parse(input).expect("validated artwork URL");
    // yt-dlp adds a guessed 1200px playlist thumbnail after the actual images
    // provided by YouTube. This unsigned /s_p/.../maxresdefault.jpg commonly
    // returns 404; its declared dimensions do not prove it exists. Prefer the
    // signed provider images instead, or retain the inspected track cover.
    !(url.path().starts_with("/s_p/")
        && url.path().ends_with("/maxresdefault.jpg")
        && url.query().is_none())
}

fn collection_mode(input: &str) -> CollectionMode {
    let Ok(url) = url::Url::parse(input) else {
        return CollectionMode::Collection;
    };
    let youtube = matches!(
        url.host_str(),
        Some("youtube.com" | "www.youtube.com" | "music.youtube.com" | "m.youtube.com")
    );
    let album = youtube
        && ((url.host_str() == Some("music.youtube.com") && url.path().starts_with("/browse/MP"))
            || url
                .query_pairs()
                .any(|(key, value)| key == "list" && value.starts_with("OLAK5uy")));
    if album {
        CollectionMode::Album
    } else {
        CollectionMode::Collection
    }
}

pub fn valid_artwork_url(input: &str) -> bool {
    url::Url::parse(input).is_ok_and(|url| {
        url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none()
            && url.host_str().is_some_and(|host| {
                ["ytimg.com", "googleusercontent.com", "ggpht.com"]
                    .iter()
                    .any(|allowed| host == *allowed || host.ends_with(&format!(".{allowed}")))
            })
    })
}

pub fn track_from_json(value: &Value) -> Option<Track> {
    let id = string(value, "id");
    if id.is_empty() {
        return None;
    }
    let title = first(value, &["track", "title", "fulltitle"]);
    if title.is_empty() || title == "[Deleted video]" || title == "[Private video]" {
        return None;
    }
    let mut artist = string(value, "artist");
    if artist.is_empty() {
        artist = value
            .get("artists")
            .and_then(Value::as_array)
            .map(|artists| {
                artists
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_default();
    }
    let release_year = value
        .get("release_year")
        .and_then(Value::as_u64)
        .or_else(|| {
            string(value, "release_date")
                .get(..4)
                .and_then(|year| year.parse().ok())
        })
        .filter(|year| (1000..=9999).contains(year))
        .map(|year| year as u32);
    let source_url = first(value, &["webpage_url", "original_url"]);
    let source_url = if validate_source_url(&source_url).is_ok() {
        source_url
    } else {
        format!("https://www.youtube.com/watch?v={id}")
    };
    let metadata = Metadata {
        title,
        artist,
        album: string(value, "album"),
        album_artist: string(value, "album_artist"),
        track_number: value
            .get("track_number")
            .and_then(Value::as_u64)
            .and_then(|n| n.try_into().ok()),
        disc_number: value
            .get("disc_number")
            .and_then(Value::as_u64)
            .and_then(|n| n.try_into().ok()),
        release_year,
    };
    Some(Track {
        id,
        source_url,
        uploader: first(value, &["uploader", "channel"]),
        duration: value.get("duration").and_then(Value::as_f64),
        artwork_url: artwork_url(value),
        source_metadata: metadata.clone(),
        metadata,
    })
}

pub fn inspection_from_json(value: &Value, input: &str) -> Result<Inspection> {
    let entries = value.get("entries").and_then(Value::as_array);
    let tracks: Vec<Track> = entries
        .map(|items| items.iter().filter_map(track_from_json).collect())
        .unwrap_or_else(|| track_from_json(value).into_iter().collect());
    if tracks.is_empty() {
        return Err(EngineError::Invalid(
            "No available audio entries were found in this source.".into(),
        ));
    }
    let mut warnings = Vec::new();
    if let Some(entries) = entries {
        let missing = entries.len() - tracks.len();
        if missing > 0 {
            warnings.push(format!(
                "{missing} unavailable or incomplete entries were omitted."
            ));
        }
    }
    warnings.push("Artwork candidates are source images and may be video thumbnails rather than album covers.".into());
    let title = first(value, &["title", "playlist_title"]);
    Ok(Inspection {
        title: if title.is_empty() {
            "Audio collection".into()
        } else {
            title
        },
        source_url: input.to_owned(),
        suggested_mode: collection_mode(input),
        artwork_url: artwork_url(value),
        album_release_year: None,
        tracks,
        warnings,
    })
}

pub fn inspect(
    tools: &Toolchain,
    input: &str,
    auth: &AuthOptions,
    cancel: &Cancellation,
) -> Result<Inspection> {
    inspect_streaming(tools, input, auth, cancel, |_| {})
}

struct StreamingInspection {
    input: String,
    title: String,
    artwork_url: Option<String>,
    total: Option<usize>,
    tracks: Vec<Track>,
}

impl StreamingInspection {
    fn new(input: String) -> Self {
        Self {
            input,
            title: String::new(),
            artwork_url: None,
            total: None,
            tracks: Vec::new(),
        }
    }

    fn mode(&self) -> CollectionMode {
        collection_mode(&self.input)
    }

    fn read_line(&mut self, line: &[u8]) -> Result<Option<InspectionUpdate>> {
        // The final playlist summary preserves collection artwork and its
        // total, including entries for which yt-dlp could not emit metadata.
        let summary = line.strip_prefix(b"PLAYLIST ");
        let value: Value = serde_json::from_slice(summary.unwrap_or(line))?;
        if !value.is_object() {
            return Err(EngineError::Invalid("Invalid inspection metadata.".into()));
        }
        let total = value
            .get("playlist_count")
            .and_then(Value::as_u64)
            .or_else(|| value.get("n_entries").and_then(Value::as_u64))
            .and_then(|count| usize::try_from(count).ok());
        if let Some(total) = total {
            self.total = Some(self.total.unwrap_or(0).max(total));
        }
        if summary.is_some() {
            let title = first(&value, &["title", "playlist_title"]);
            if !title.is_empty() {
                self.title = title;
            }
            self.artwork_url = artwork_url(&value).or(self.artwork_url.take());
            return Ok(None);
        }
        let Some(track) = track_from_json(&value) else {
            return Ok(None);
        };
        if self.title.is_empty() {
            self.title = first(&value, &["playlist_title", "title"]);
        }
        if self.artwork_url.is_none() {
            self.artwork_url = track.artwork_url.clone();
        }
        let update = InspectionUpdate {
            title: self.title.clone(),
            source_url: self.input.clone(),
            suggested_mode: self.mode(),
            artwork_url: self.artwork_url.clone(),
            index: self.tracks.len(),
            total: self.total,
            track: track.clone(),
        };
        self.tracks.push(track);
        Ok(Some(update))
    }

    fn finish(self) -> Result<Inspection> {
        if self.tracks.is_empty() {
            return Err(EngineError::Invalid(
                "No available audio entries were found in this source.".into(),
            ));
        }
        let mut warnings = Vec::new();
        let missing = self
            .total
            .unwrap_or(self.tracks.len())
            .saturating_sub(self.tracks.len());
        if missing > 0 {
            warnings.push(format!(
                "{missing} unavailable or incomplete entries were omitted."
            ));
        }
        warnings.push("Artwork candidates are source images and may be video thumbnails rather than album covers.".into());
        Ok(Inspection {
            suggested_mode: self.mode(),
            title: self.title,
            source_url: self.input,
            artwork_url: self.artwork_url,
            album_release_year: None,
            tracks: self.tracks,
            warnings,
        })
    }
}

pub fn inspect_streaming(
    tools: &Toolchain,
    input: &str,
    auth: &AuthOptions,
    cancel: &Cancellation,
    mut emit: impl FnMut(InspectionUpdate) + Send,
) -> Result<Inspection> {
    let input = validate_source_url(input)?;
    let album_source = input.clone();
    let album_cancel = cancel.clone();
    let album_lookup = (collection_mode(&input) == CollectionMode::Album)
        .then(|| std::thread::spawn(move || crate::album::lookup(&album_source, &album_cancel)));
    let mut cmd = downloader(tools, auth)?;
    cmd.args([
        "--dump-json",
        "--skip-download",
        "--ignore-errors",
        "--print",
        "playlist:PLAYLIST %(.{title,playlist_title,thumbnail,thumbnails,playlist_count})j",
        "--",
        &input,
    ]);
    let mut inspection = StreamingInspection::new(input);
    run_lines(&mut cmd, cancel, Duration::from_secs(600), |line| {
        if let Some(update) = inspection.read_line(line)? {
            emit(update);
        }
        Ok(())
    })?;
    let mut result = inspection.finish()?;
    if let Some(album) = album_lookup.and_then(|job| job.join().ok()).flatten() {
        crate::album::apply(&mut result, album);
    }
    cancel.check()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn managed_auth_never_falls_back_and_never_deserializes_a_cookie_path() {
        let mut auth: AuthOptions = serde_json::from_value(json!({"browser":null,"managedSession":"session-fixture", "cookieFile":"/caller/secret.txt"})).unwrap();
        let tools = Toolchain {
            yt_dlp: "yt-dlp".into(),
            ffmpeg: "ffmpeg".into(),
            ffprobe: "ffprobe".into(),
            node: None,
        };
        assert!(auth.cookie_file.is_none());
        assert!(downloader(&tools, &auth).is_err());
        auth.cookie_file = Some("/native/fixture.txt".into());
        assert!(downloader(&tools, &auth).is_ok());
        assert!(serde_json::to_value(&auth)
            .unwrap()
            .get("cookieFile")
            .is_none());
        auth.browser = Some(Browser::Chrome);
        assert!(downloader(&tools, &auth).is_err());
    }

    #[test]
    fn browser_profile_and_firefox_container_are_one_exact_argument() {
        let tools = Toolchain {
            yt_dlp: "yt-dlp".into(),
            ffmpeg: "ffmpeg".into(),
            ffprobe: "ffprobe".into(),
            node: None,
        };
        let mut auth = AuthOptions {
            browser: Some(Browser::Firefox),
            browser_profile: Some("opaque-profile".into()),
            browser_profile_path: Some("/fixture/Music Profile".into()),
            browser_container: Some("Listening".into()),
            ..AuthOptions::default()
        };
        let command = downloader(&tools, &auth).unwrap();
        let args: Vec<_> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert!(args.windows(2).any(|args| args
            == [
                "--cookies-from-browser",
                "firefox:/fixture/Music Profile::Listening"
            ]));
        let serialized = serde_json::to_string(&auth).unwrap();
        assert!(!serialized.contains("Music Profile"));
        assert!(!serialized.contains("Listening"));
        auth.browser_container = None;
        assert!(downloader(&tools, &auth)
            .unwrap()
            .get_args()
            .any(|arg| arg == "firefox:/fixture/Music Profile::none"));
        auth.browser_profile_path = None;
        assert!(downloader(&tools, &auth).is_err());
    }

    #[test]
    fn album_playlist_links_choose_album_mode_from_the_real_list_parameter() {
        for input in [
            "https://music.youtube.com/playlist?list=OLAK5uy_mUJg8SRBv0XUvkn4To7sIQs1TBjAhJYE4",
            "https://www.youtube.com/playlist?list=OLAK5uy_example",
            "https://music.youtube.com/watch?v=abc&list=OLAK5uy_example",
            "https://music.youtube.com/browse/MPexample",
        ] {
            assert_eq!(collection_mode(input), CollectionMode::Album);
        }
        for input in [
            "https://music.youtube.com/playlist?list=PLmix",
            "https://music.youtube.com/watch?v=OLAK5uy_example",
            "https://music.youtube.com/playlist?other=OLAK5uy_example&list=PLmix",
            "https://example.com/playlist?list=OLAK5uy_example",
        ] {
            assert_eq!(collection_mode(input), CollectionMode::Collection);
        }
    }

    #[test]
    fn actual_album_cover_wins_over_a_synthetic_maxres_thumbnail() {
        let value = json!({"thumbnails": [
            {"url":"https://i9.ytimg.com/s_p/OLAK5uy_example/mqdefault.jpg?rs=signed", "width":180,"height":180},
            {"url":"https://i9.ytimg.com/s_p/OLAK5uy_example/sddefault.jpg?rs=signed", "width":640,"height":640},
            {"url":"https://i9.ytimg.com/s_p/OLAK5uy_example/maxresdefault.jpg", "width":1200,"height":1200}
        ]});
        assert_eq!(
            artwork_url(&value).as_deref(),
            Some("https://i9.ytimg.com/s_p/OLAK5uy_example/sddefault.jpg?rs=signed")
        );
        // This rule applies to guessed playlist images, not legitimate video
        // or signed playlist covers at the same resolution.
        assert!(usable_artwork_candidate(
            "https://i.ytimg.com/vi/abc/maxresdefault.jpg"
        ));
        assert!(usable_artwork_candidate(
            "https://i9.ytimg.com/s_p/album/maxresdefault.jpg?rs=signed"
        ));
        assert!(artwork_url(
            &json!({"thumbnail":"https://i9.ytimg.com/s_p/album/maxresdefault.jpg"})
        )
        .is_none());
    }

    #[test]
    fn streamed_tracks_preserve_occurrences_and_finalize_collection_artwork() {
        let mut stream =
            StreamingInspection::new("https://music.youtube.com/browse/MPexample".into());
        let entry = serde_json::to_vec(&json!({
            "id": "abc", "title": "Song", "playlist_title": "Album",
            "playlist_count": 3, "track_number": 7,
            "thumbnail": "https://i.ytimg.com/track.jpg"
        }))
        .unwrap();
        let first = stream.read_line(&entry).unwrap().unwrap();
        assert_eq!(first.title, "Album");
        assert_eq!(first.index, 0);
        assert_eq!(first.total, Some(3));
        assert_eq!(first.suggested_mode, CollectionMode::Album);
        assert_eq!(first.track.metadata.track_number, Some(7));
        let second = stream.read_line(&entry).unwrap().unwrap();
        assert_eq!(second.index, 1);
        assert_eq!(first.track.id, second.track.id);
        stream.read_line(br#"PLAYLIST {"title":"Album", "playlist_count":3,"thumbnail":"https://i.ytimg.com/cover.jpg"}"#).unwrap();
        let result = stream.finish().unwrap();
        assert_eq!(result.tracks.len(), 2);
        assert_eq!(
            result.artwork_url.as_deref(),
            Some("https://i.ytimg.com/cover.jpg")
        );
        assert!(result.warnings[0].contains("1 unavailable"));
    }

    #[test]
    fn streaming_handles_unknown_totals_and_rejects_invalid_or_empty_output() {
        let mut stream = StreamingInspection::new("https://www.youtube.com/watch?v=abc".into());
        assert!(stream.read_line(b"not JSON").is_err());
        assert!(stream.read_line(b"null").is_err());
        assert!(stream.finish().is_err());
        let mut stream = StreamingInspection::new("https://www.youtube.com/watch?v=abc".into());
        let update = stream
            .read_line(br#"{"id":"abc","title":"Song", "playlist_count":null,"n_entries":5}"#)
            .unwrap()
            .unwrap();
        assert_eq!(update.total, Some(5));
        assert_eq!(update.title, "Song");
    }

    #[test]
    fn uploader_and_playlist_position_are_not_music_tags() {
        let track = track_from_json(&json!({"id":"abc", "title":"A live performance", "uploader":"Festival", "playlist_index":3, "upload_date":"20250101"})).unwrap();
        assert!(track.metadata.artist.is_empty());
        assert!(track.metadata.album.is_empty());
        assert_eq!(track.metadata.track_number, None);
        assert_eq!(track.metadata.release_year, None);
        assert_eq!(track.uploader, "Festival");
    }

    #[test]
    fn explicit_music_fields_survive_and_empty_artist_uses_artists() {
        let track = track_from_json(&json!({"id":"abc", "title":"video", "track":"Song", "artist":"", "artists":["One", "Two"], "album":"Release", "track_number":7, "release_date":"20220410"})).unwrap();
        assert_eq!(track.metadata.title, "Song");
        assert_eq!(track.metadata.artist, "One; Two");
        assert_eq!(track.metadata.track_number, Some(7));
        assert_eq!(track.metadata.release_year, Some(2022));
    }

    #[test]
    fn generic_playlists_preserve_repeated_entries_and_report_missing_items() {
        let preview = inspection_from_json(&json!({"title":"Mix", "entries":[{"id":"abc","title":"Song"}, null, {"id":"abc","title":"Song"}]}), "https://www.youtube.com/playlist?list=x").unwrap();
        assert_eq!(preview.tracks.len(), 2);
        assert_eq!(preview.suggested_mode, CollectionMode::Collection);
        assert!(preview.warnings[0].contains("1 unavailable"));
    }

    #[test]
    fn source_and_artwork_hosts_are_checked_structurally() {
        assert!(validate_source_url("https://music.youtube.com/browse/MPexample").is_ok());
        assert!(validate_source_url("https://youtube.com.evil.test/watch?v=x").is_err());
        assert!(validate_source_url("https://user:password@youtube.com/watch?v=x").is_err());
        assert!(!valid_artwork_url("https://ytimg.com.evil.test/image.jpg"));
    }
}
