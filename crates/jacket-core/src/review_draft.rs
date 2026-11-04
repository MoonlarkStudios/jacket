use crate::Preferences;
use audio_engine::Inspection;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const MAX_REVISION: u64 = 9_007_199_254_740_991;

/// The persisted review snapshot used to conditionally retire a completed export.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompletionIdentity {
    pub session_id: String,
    pub revision: u64,
}

impl CompletionIdentity {
    pub fn validate(&self) -> Result<(), String> {
        if self.session_id.is_empty()
            || self.session_id.len() > 100
            || !self
                .session_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || self.revision == 0
            || self.revision > MAX_REVISION
        {
            return Err("Invalid review revision.".into());
        }
        Ok(())
    }
}

/// Version one deliberately preserves the previous desktop application's schema.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewDraft {
    pub schema_version: u32,
    pub session_id: String,
    pub revision: u64,
    pub url: String,
    pub inspection: Inspection,
    pub selected: Vec<usize>,
    pub preferences: Preferences,
}

impl ReviewDraft {
    pub fn validate(&self) -> Result<(), String> {
        CompletionIdentity {
            session_id: self.session_id.clone(),
            revision: self.revision,
        }
        .validate()?;
        if self.schema_version != 1 {
            return Err("This saved review needs a newer version of Jacket.".into());
        }
        audio_engine::source::validate_source_url(&self.url).map_err(|error| error.to_string())?;
        audio_engine::source::validate_source_url(&self.inspection.source_url)
            .map_err(|error| error.to_string())?;
        if self.inspection.tracks.is_empty() || self.inspection.tracks.len() > 10_000 {
            return Err("The saved review has an unsupported number of tracks.".into());
        }
        let mut selected = BTreeSet::new();
        if self
            .selected
            .iter()
            .any(|index| *index >= self.inspection.tracks.len() || !selected.insert(*index))
        {
            return Err("Invalid saved track selection.".into());
        }
        if [
            &self.inspection.title,
            &self.preferences.title,
            &self.preferences.album_title,
            &self.preferences.album_artist,
        ]
        .iter()
        .any(|value| value.len() > 16_384)
        {
            return Err("A saved review field is too long.".into());
        }
        for track in &self.inspection.tracks {
            audio_engine::source::validate_source_url(&track.source_url)
                .map_err(|error| error.to_string())?;
            if track
                .duration
                .is_some_and(|value| !value.is_finite() || value < 0.0)
            {
                return Err("Invalid saved track duration.".into());
            }
        }
        Ok(())
    }
}
