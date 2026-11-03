use url::Url;

/// A video link shared from a playlist starts with the song unless requested otherwise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SourceIntent {
    #[default]
    Song,
    Playlist,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceLink {
    pub url: String,
    pub song_url: Option<String>,
    pub playlist_url: Option<String>,
}

impl SourceLink {
    /// Validate complete public source links before invoking the downloader.
    pub fn parse(input: &str) -> Result<Self, String> {
        let url = Url::parse(input.trim()).map_err(|_| invalid())?;
        if url.scheme() != "https"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
        {
            return Err(invalid());
        }
        let host = url.host_str().ok_or_else(invalid)?;
        if !matches!(
            host,
            "youtube.com" | "www.youtube.com" | "music.youtube.com" | "m.youtube.com" | "youtu.be"
        ) {
            return Err(invalid());
        }
        let values = url.query_pairs().collect::<Vec<_>>();
        if ["v", "list"]
            .iter()
            .any(|key| values.iter().filter(|(name, _)| name == key).count() > 1)
        {
            return Err(
                "This link repeats a video or playlist identifier. Copy a fresh YouTube link."
                    .into(),
            );
        }
        let video = if host == "youtu.be" {
            url.path()
                .strip_prefix('/')
                .filter(|value| valid_identifier(value, 11, Some(11)))
                .map(str::to_owned)
        } else if url.path() == "/watch" {
            values
                .iter()
                .find(|(key, _)| key == "v")
                .map(|(_, value)| value.to_string())
                .filter(|value| valid_identifier(value, 11, Some(11)))
        } else {
            None
        };
        if (host == "youtu.be"
            || (url.path() == "/watch" && values.iter().any(|(key, _)| key == "v")))
            && video.is_none()
        {
            return Err(invalid());
        }
        let playlist = values
            .iter()
            .find(|(key, _)| key == "list")
            .map(|(_, value)| value.to_string())
            .filter(|value| valid_identifier(value, 10, None));
        let browse = host == "music.youtube.com"
            && url
                .path()
                .strip_prefix("/browse/MP")
                .is_some_and(|value| valid_identifier(value, 1, None));
        let complete = if host == "youtu.be" {
            video.is_some()
        } else {
            (url.path() == "/watch" && video.is_some())
                || (matches!(url.path(), "/watch" | "/playlist") && playlist.is_some())
                || browse
        };
        if !complete {
            return Err(invalid());
        }
        let origin = if host == "music.youtube.com" {
            "https://music.youtube.com"
        } else {
            "https://www.youtube.com"
        };
        let song_url = video.map(|id| format!("{origin}/watch?v={id}"));
        let playlist_url = playlist
            .map(|id| format!("{origin}/playlist?list={id}"))
            .or_else(|| {
                if song_url.is_none() {
                    Some(url.to_string())
                } else {
                    None
                }
            });
        Ok(Self {
            url: url.to_string(),
            song_url,
            playlist_url,
        })
    }

    pub fn target(&self, intent: SourceIntent) -> &str {
        match intent {
            SourceIntent::Song => self.song_url.as_deref().or(self.playlist_url.as_deref()),
            SourceIntent::Playlist => self.playlist_url.as_deref().or(self.song_url.as_deref()),
        }
        .unwrap_or(&self.url)
    }

    pub fn has_playlist_context(&self) -> bool {
        self.song_url.is_some() && self.playlist_url.is_some()
    }
}

fn valid_identifier(value: &str, minimum: usize, maximum: Option<usize>) -> bool {
    value.len() >= minimum
        && maximum.is_none_or(|limit| value.len() <= limit)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn invalid() -> String {
    "Paste a complete HTTPS YouTube or YouTube Music song, album, or playlist link.".into()
}

#[cfg(test)]
mod tests {
    use super::{SourceIntent, SourceLink};

    #[test]
    fn shared_playlist_song_requires_explicit_collection_intent() {
        let link = SourceLink::parse(
            "https://music.youtube.com/watch?v=dQw4w9WgXcQ&list=PLabcdefghijk&index=2",
        )
        .expect("valid link");
        assert!(link.has_playlist_context());
        assert_eq!(
            link.target(SourceIntent::Song),
            "https://music.youtube.com/watch?v=dQw4w9WgXcQ"
        );
        assert_eq!(
            link.target(SourceIntent::Playlist),
            "https://music.youtube.com/playlist?list=PLabcdefghijk"
        );
    }

    #[test]
    fn reject_ambiguous_targets_and_untrusted_hosts() {
        for input in [
            "http://youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtube.com.evil.test/watch?v=dQw4w9WgXcQ",
            "https://user@youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtube.com:123/watch?v=dQw4w9WgXcQ",
            "https://youtube.com/watch?v=dQw4w9WgXcQ&v=abcdefghijk",
            "https://youtube.com/watch?v=broken&list=PLabcdefghijk",
            "https://youtube.com/playlist?list=PLabcdefghijk&list=PLabcdefghijk",
            "https://youtu.be/dQw4w9WgXcQ/extra",
            "https://youtube.com/watch?v=",
            "https://youtube.com/",
        ] {
            assert!(SourceLink::parse(input).is_err(), "accepted {input}");
        }
    }

    #[test]
    fn accepts_short_links_albums_and_playlist_only_watch_links() {
        for input in [
            " https://youtu.be/dQw4w9WgXcQ?t=2 ",
            "https://music.youtube.com/browse/MPREb_123",
            "https://youtube.com/watch?list=PLabcdefghijk",
        ] {
            assert!(SourceLink::parse(input).is_ok(), "rejected {input}");
        }
    }
}
