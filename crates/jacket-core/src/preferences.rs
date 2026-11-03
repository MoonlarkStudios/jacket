use audio_engine::{
    ArtworkMode, AudioFormat, CollectionMode, ExportPlan, Inspection, OutputKind, Track,
};
use serde::{Deserialize, Serialize};

/// Committed export choices. Browser credentials never belong in review data.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Preferences {
    pub title: String,
    pub mode: CollectionMode,
    pub album_title: String,
    pub album_artist: String,
    pub year: Option<u32>,
    pub format: AudioFormat,
    pub artwork_mode: ArtworkMode,
    pub shared_artwork: Option<String>,
    #[serde(default)]
    pub shared_artwork_asset: Option<String>,
    #[serde(default)]
    pub output_kind: OutputKind,
}

impl Preferences {
    pub fn from_inspection(inspection: &Inspection, format: AudioFormat) -> Self {
        let album_title = shared(
            inspection
                .tracks
                .iter()
                .map(|track| track.metadata.album.trim()),
        );
        let album_artist = shared(inspection.tracks.iter().map(|track| {
            if track.metadata.album_artist.trim().is_empty() {
                track.metadata.artist.trim()
            } else {
                track.metadata.album_artist.trim()
            }
        }));
        let song =
            inspection.suggested_mode == CollectionMode::Collection && inspection.tracks.len() == 1;
        let title = if inspection.suggested_mode == CollectionMode::Album {
            album_title
                .filter(|title| !title.is_empty())
                .unwrap_or(&inspection.title)
                .to_owned()
        } else if song {
            song_title(&inspection.tracks[0])
        } else {
            inspection.title.clone()
        };
        Self {
            title,
            mode: inspection.suggested_mode,
            album_title: album_title
                .filter(|title| !title.is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    if inspection.suggested_mode == CollectionMode::Album {
                        inspection.title.clone()
                    } else {
                        String::new()
                    }
                }),
            album_artist: album_artist.unwrap_or_default().to_owned(),
            year: if inspection.suggested_mode == CollectionMode::Album {
                inspection.album_release_year
            } else {
                None
            }
            .or_else(|| {
                shared(
                    inspection
                        .tracks
                        .iter()
                        .map(|track| track.metadata.release_year),
                )
                .flatten()
            }),
            format,
            artwork_mode: if inspection.suggested_mode == CollectionMode::Album {
                ArtworkMode::Shared
            } else {
                ArtworkMode::PerTrack
            },
            shared_artwork: inspection.artwork_url.clone().or_else(|| {
                inspection
                    .tracks
                    .iter()
                    .find_map(|track| track.artwork_url.clone())
            }),
            shared_artwork_asset: None,
            output_kind: if song {
                OutputKind::File
            } else {
                OutputKind::Folder
            },
        }
    }

    pub fn from_plan(plan: &ExportPlan) -> Self {
        Self {
            title: plan.title.clone(),
            mode: plan.mode,
            album_title: shared(
                plan.tracks
                    .iter()
                    .map(|track| track.metadata.album.as_str()),
            )
            .unwrap_or_default()
            .to_owned(),
            album_artist: shared(
                plan.tracks
                    .iter()
                    .map(|track| track.metadata.album_artist.as_str()),
            )
            .unwrap_or_default()
            .to_owned(),
            year: shared(plan.tracks.iter().map(|track| track.metadata.release_year)).flatten(),
            format: plan.format,
            artwork_mode: plan.artwork_mode,
            shared_artwork: plan.shared_artwork_url.clone(),
            shared_artwork_asset: plan.shared_artwork_asset.clone(),
            output_kind: plan.output_kind,
        }
    }
}

pub(crate) fn song_title(track: &Track) -> String {
    [track.metadata.artist.trim(), track.metadata.title.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" - ")
}

fn shared<T: PartialEq>(mut values: impl Iterator<Item = T>) -> Option<T> {
    let first = values.next()?;
    values.all(|value| value == first).then_some(first)
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            title: String::new(),
            mode: CollectionMode::Collection,
            album_title: String::new(),
            album_artist: String::new(),
            year: None,
            format: AudioFormat::Compatible,
            artwork_mode: ArtworkMode::PerTrack,
            shared_artwork: None,
            shared_artwork_asset: None,
            output_kind: OutputKind::Folder,
        }
    }
}
