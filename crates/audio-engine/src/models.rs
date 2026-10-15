use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthOptions {
    pub browser: Option<Browser>,
    /// Stable, opaque catalog ID. Native code resolves it on each operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser_profile: Option<String>,
    #[serde(skip)]
    pub browser_profile_path: Option<std::path::PathBuf>,
    #[serde(skip)]
    pub browser_container: Option<String>,
    /// Opaque local session generation; never cookie contents or a caller path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub managed_session: Option<String>,
    /// Native-only handoff, deliberately excluded from IPC and recovery files.
    #[serde(skip)]
    pub cookie_file: Option<std::path::PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Browser {
    Chrome,
    Firefox,
    Safari,
    Edge,
}

impl Browser {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Chrome => "chrome",
            Self::Firefox => "firefox",
            Self::Safari => "safari",
            Self::Edge => "edge",
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub release_year: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: String,
    pub source_url: String,
    pub uploader: String,
    pub duration: Option<f64>,
    pub artwork_url: Option<String>,
    pub source_metadata: Metadata,
    pub metadata: Metadata,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CollectionMode {
    Album,
    Collection,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Inspection {
    pub title: String,
    pub source_url: String,
    pub suggested_mode: CollectionMode,
    pub artwork_url: Option<String>,
    #[serde(default)]
    pub album_release_year: Option<u32>,
    pub tracks: Vec<Track>,
    pub warnings: Vec<String>,
}

/// An append-only, ordered track result. Video IDs are not unique within a
/// playlist, so index identifies an occurrence rather than deduplicating it.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectionUpdate {
    pub title: String,
    pub source_url: String,
    pub suggested_mode: CollectionMode,
    pub artwork_url: Option<String>,
    pub index: usize,
    pub total: Option<usize>,
    pub track: Track,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    Compatible,
    Preserve,
    Mp3,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ArtworkMode {
    PerTrack,
    Shared,
    None,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPlan {
    pub title: String,
    pub source_url: String,
    pub mode: CollectionMode,
    pub format: AudioFormat,
    pub artwork_mode: ArtworkMode,
    pub shared_artwork_url: Option<String>,
    pub auth: AuthOptions,
    pub tracks: Vec<Track>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub stage: String,
    pub message: String,
    pub completed: usize,
    pub total: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestTrack {
    pub source_id: String,
    pub source_url: String,
    pub source_metadata: Metadata,
    pub metadata: Metadata,
    #[serde(default)]
    pub embedded_metadata: Option<Metadata>,
    #[serde(default)]
    pub embedded_artwork: Option<bool>,
    pub relative_path: String,
    pub artwork_path: Option<String>,
    pub source_codec: String,
    pub output_codec: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub output_path: String,
    pub track_count: usize,
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn older_manifest_tracks_have_unknown_embedding_status() {
        let old = serde_json::json!({
            "sourceId": "id", "sourceUrl": "https://www.youtube.com/watch?v=id",
            "sourceMetadata": Metadata::default(), "metadata": Metadata::default(),
            "relativePath": "song.mp3", "artworkPath": "cover.jpg",
            "sourceCodec": "mp3", "outputCodec": "mp3", "sha256": "digest"
        });
        let track: ManifestTrack = serde_json::from_value(old).unwrap();
        assert!(track.embedded_metadata.is_none());
        assert!(track.embedded_artwork.is_none());
    }
}
