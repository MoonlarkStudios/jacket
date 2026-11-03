use crate::bulk_edit::{self, Change};
use crate::preferences;
use crate::{BulkEdits, BulkPreview, CompletionIdentity, Preferences, ReviewDraft};
use audio_engine::{
    ArtworkMode, AudioFormat, AuthOptions, CollectionMode, ExportPlan, Inspection, Metadata,
    OutputKind,
};
use std::collections::{BTreeMap, BTreeSet};

/// A review identifies playlist occurrences by position, never by source video ID.
#[derive(Clone, Debug)]
pub struct Review {
    pub inspection: Inspection,
    pub selected: BTreeSet<usize>,
    pub preferences: Preferences,
    session_id: String,
    revision: u64,
    bulk_undo: Vec<Change>,
    numbering_undo: Vec<NumberChange>,
}

#[derive(Clone, Debug)]
struct NumberChange {
    index: usize,
    id: String,
    source_url: String,
    before: Option<u32>,
    after: Option<u32>,
}

impl Review {
    pub fn new(mut inspection: Inspection, format: AudioFormat) -> Self {
        let preferences = Preferences::from_inspection(&inspection, format);
        if preferences.mode == CollectionMode::Album {
            fill_numbers(&mut inspection, false);
        }
        Self {
            selected: (0..inspection.tracks.len()).collect(),
            inspection,
            preferences,
            session_id: uuid::Uuid::new_v4().to_string(),
            revision: 1,
            bulk_undo: Vec::new(),
            numbering_undo: Vec::new(),
        }
    }

    pub fn from_draft(draft: ReviewDraft) -> Result<Self, String> {
        draft.validate()?;
        Ok(Self {
            inspection: draft.inspection,
            selected: draft.selected.into_iter().collect(),
            preferences: draft.preferences,
            session_id: draft.session_id,
            revision: draft.revision,
            bulk_undo: Vec::new(),
            numbering_undo: Vec::new(),
        })
    }

    /// Recovery already contains the exact committed tags and source-order selection.
    pub fn from_plan(plan: &ExportPlan) -> Self {
        Self {
            inspection: Inspection {
                title: plan.title.clone(),
                source_url: plan.source_url.clone(),
                suggested_mode: plan.mode,
                artwork_url: plan.shared_artwork_url.clone(),
                album_release_year: None,
                tracks: plan.tracks.clone(),
                warnings: Vec::new(),
            },
            selected: (0..plan.tracks.len()).collect(),
            preferences: Preferences::from_plan(plan),
            session_id: uuid::Uuid::new_v4().to_string(),
            revision: 1,
            bulk_undo: Vec::new(),
            numbering_undo: Vec::new(),
        }
    }

    pub fn draft(&self) -> ReviewDraft {
        ReviewDraft {
            schema_version: 1,
            session_id: self.session_id.clone(),
            revision: self.revision,
            url: self.inspection.source_url.clone(),
            inspection: self.inspection.clone(),
            selected: self.selected.iter().copied().collect(),
            preferences: self.preferences.clone(),
        }
    }

    pub fn identity(&self) -> CompletionIdentity {
        CompletionIdentity {
            session_id: self.session_id.clone(),
            revision: self.revision,
        }
    }

    /// Call after changing public preference fields as one committed edit.
    pub fn changed(&mut self) -> Result<(), String> {
        if self.revision >= 9_007_199_254_740_991 {
            return Err("This review has reached its revision limit. Start a new review.".into());
        }
        self.revision += 1;
        Ok(())
    }

    pub fn select(&mut self, index: usize, selected: bool) -> Result<(), String> {
        if index >= self.inspection.tracks.len() {
            return Err("This track is no longer in the review.".into());
        }
        if self.selected.contains(&index) != selected {
            self.changed()?;
            if selected {
                self.selected.insert(index);
            } else {
                self.selected.remove(&index);
            }
        }
        Ok(())
    }

    pub fn select_all(&mut self, selected: bool) -> Result<(), String> {
        self.changed()?;
        self.selected = if selected {
            (0..self.inspection.tracks.len()).collect()
        } else {
            BTreeSet::new()
        };
        Ok(())
    }

    pub fn edit_track(&mut self, index: usize, metadata: Metadata) -> Result<(), String> {
        validate_metadata(&metadata)?;
        let before = self
            .inspection
            .tracks
            .get(index)
            .ok_or("This track is no longer in the review.")?;
        if bulk_edit::same_metadata(&before.metadata, &metadata) {
            return Ok(());
        }
        let follows_song = self.inspection.tracks.len() == 1
            && self.preferences.mode == CollectionMode::Collection
            && self.preferences.title == preferences::song_title(before);
        self.changed()?;
        self.inspection.tracks[index].metadata = metadata;
        if follows_song {
            self.preferences.title = preferences::song_title(&self.inspection.tracks[index]);
        }
        Ok(())
    }

    pub fn set_mode(&mut self, mode: CollectionMode) -> Result<(), String> {
        if self.preferences.mode == mode {
            return Ok(());
        }
        self.changed()?;
        self.preferences.mode = mode;
        if mode == CollectionMode::Album {
            fill_numbers(&mut self.inspection, false);
        }
        Ok(())
    }

    pub fn update_album(
        &mut self,
        title: String,
        artist: String,
        year: Option<u32>,
    ) -> Result<(), String> {
        if year.is_some_and(|value| !(1000..=9999).contains(&value)) {
            return Err("Enter a year from 1000 to 9999, or leave it blank.".into());
        }
        let follows_album = self.preferences.title == self.preferences.album_title.trim()
            || self.preferences.title == self.inspection.title;
        self.changed()?;
        self.preferences.album_title = title;
        self.preferences.album_artist = artist;
        self.preferences.year = year;
        if follows_album {
            self.preferences.title = if self.preferences.album_title.trim().is_empty() {
                self.inspection.title.clone()
            } else {
                self.preferences.album_title.trim().into()
            };
        }
        Ok(())
    }

    /// A one-track source has one cover and one metadata presentation even when
    /// its source identified it as an album. Selection does not change intent.
    pub fn is_song(&self) -> bool {
        self.inspection.tracks.len() == 1
    }

    /// The tags used in the export, without changing the inspected or edited
    /// per-track values. Shared album values only override nonblank fields.
    pub fn effective_metadata(&self, index: usize) -> Option<Metadata> {
        let mut metadata = self.inspection.tracks.get(index)?.metadata.clone();
        if self.preferences.mode == CollectionMode::Album {
            if !self.preferences.album_title.trim().is_empty() {
                metadata.album = self.preferences.album_title.trim().into();
            }
            if !self.preferences.album_artist.trim().is_empty() {
                metadata.album_artist = self.preferences.album_artist.trim().into();
            }
            if let Some(year) = self.preferences.year {
                metadata.release_year = Some(year);
            }
        }
        Some(metadata)
    }

    pub fn plan(&self, auth: AuthOptions) -> Result<ExportPlan, String> {
        if self.preferences.title.trim().is_empty() {
            return Err("Enter a name for the export.".into());
        }
        if self.selected.is_empty() {
            return Err("Select at least one track to export.".into());
        }
        if self
            .selected
            .iter()
            .any(|index| *index >= self.inspection.tracks.len())
        {
            return Err("Invalid track selection.".into());
        }
        if self.preferences.output_kind == OutputKind::File && self.selected.len() != 1 {
            return Err("Audio file output requires exactly one selected track.".into());
        }
        if self
            .preferences
            .year
            .is_some_and(|value| !(1000..=9999).contains(&value))
        {
            return Err("Enter a year from 1000 to 9999, or leave it blank.".into());
        }
        let mut tracks = Vec::with_capacity(self.selected.len());
        for &index in &self.selected {
            let mut track = self.inspection.tracks[index].clone();
            track.metadata = self
                .effective_metadata(index)
                .ok_or("Invalid track selection.")?;
            validate_metadata(&track.metadata)?;
            tracks.push(track);
        }
        // Older drafts used per-track artwork for a song. Resolve the same
        // single image as Shared so the simplified cover control, preview and
        // actual export agree without rewriting a saved draft on restoration.
        let artwork_mode =
            if self.is_song() && matches!(self.preferences.artwork_mode, ArtworkMode::PerTrack) {
                ArtworkMode::Shared
            } else {
                self.preferences.artwork_mode
            };
        let shared_artwork_url = self.preferences.shared_artwork.clone().or_else(|| {
            self.is_song()
                .then(|| {
                    self.inspection.tracks[0]
                        .artwork_url
                        .clone()
                        .or_else(|| self.inspection.artwork_url.clone())
                })
                .flatten()
        });
        Ok(ExportPlan {
            title: self.preferences.title.clone(),
            source_url: self.inspection.source_url.clone(),
            mode: self.preferences.mode,
            format: self.preferences.format,
            artwork_mode,
            shared_artwork_url,
            shared_artwork_asset: self.preferences.shared_artwork_asset.clone(),
            shared_artwork_path: None,
            output_kind: self.preferences.output_kind,
            auth,
            tracks,
        })
    }

    pub fn preview_bulk(&self, edits: &BulkEdits) -> Result<BulkPreview, String> {
        bulk_edit::preview(&self.inspection.tracks, &self.selected, edits)
    }

    pub fn apply_bulk(&mut self, edits: &BulkEdits) -> Result<usize, String> {
        let preview = self.preview_bulk(edits)?;
        if preview.changed_count == 0 {
            return Ok(0);
        }
        self.changed()?;
        let before_song = self.inspection.tracks.first().map(preferences::song_title);
        let follows_song = self.inspection.tracks.len() == 1
            && self.preferences.mode == CollectionMode::Collection
            && before_song.as_ref() == Some(&self.preferences.title);
        self.bulk_undo.clear();
        for row in preview.rows.into_iter().filter(|row| row.changed) {
            let track = &mut self.inspection.tracks[row.index];
            self.bulk_undo.push(Change {
                index: row.index,
                id: track.id.clone(),
                source_url: track.source_url.clone(),
                before: row.before,
                after: row.after.clone(),
            });
            track.metadata = row.after;
        }
        if follows_song {
            self.preferences.title = preferences::song_title(&self.inspection.tracks[0]);
        }
        Ok(preview.changed_count)
    }

    pub fn can_undo_bulk(&self) -> bool {
        !self.bulk_undo.is_empty()
    }

    pub fn undo_bulk(&mut self) -> Result<bool, String> {
        if self.bulk_undo.is_empty() {
            return Ok(false);
        }
        self.changed()?;
        let follows_song = self.inspection.tracks.len() == 1
            && self.preferences.mode == CollectionMode::Collection
            && self.preferences.title == preferences::song_title(&self.inspection.tracks[0]);
        let reverted = bulk_edit::undo(&mut self.inspection.tracks, &self.bulk_undo);
        self.bulk_undo.clear();
        if follows_song {
            self.preferences.title = preferences::song_title(&self.inspection.tracks[0]);
        }
        Ok(reverted)
    }

    pub fn number_album_tracks(&mut self) -> Result<(), String> {
        self.changed()?;
        let before = self
            .inspection
            .tracks
            .iter()
            .map(|track| track.metadata.track_number)
            .collect::<Vec<_>>();
        fill_numbers(&mut self.inspection, true);
        self.numbering_undo = self
            .inspection
            .tracks
            .iter()
            .enumerate()
            .filter(|(index, track)| before[*index] != track.metadata.track_number)
            .map(|(index, track)| NumberChange {
                index,
                id: track.id.clone(),
                source_url: track.source_url.clone(),
                before: before[index],
                after: track.metadata.track_number,
            })
            .collect();
        Ok(())
    }

    pub fn can_undo_numbering(&self) -> bool {
        !self.numbering_undo.is_empty()
    }

    pub fn undo_numbering(&mut self) -> Result<bool, String> {
        if self.numbering_undo.is_empty() {
            return Ok(false);
        }
        self.changed()?;
        let mut reverted = false;
        for change in self.numbering_undo.drain(..) {
            if let Some(track) = self.inspection.tracks.get_mut(change.index) {
                if track.id == change.id
                    && track.source_url == change.source_url
                    && track.metadata.track_number == change.after
                {
                    track.metadata.track_number = change.before;
                    reverted = true;
                }
            }
        }
        Ok(reverted)
    }
}

fn fill_numbers(inspection: &mut Inspection, replace: bool) {
    let mut next_by_disc = BTreeMap::<u32, u32>::new();
    for track in &mut inspection.tracks {
        let next = next_by_disc
            .entry(track.metadata.disc_number.unwrap_or(1))
            .or_default();
        *next += 1;
        if replace || track.metadata.track_number.is_none() {
            track.metadata.track_number = Some(*next);
        }
    }
}

fn validate_metadata(metadata: &Metadata) -> Result<(), String> {
    if metadata.title.trim().is_empty() {
        return Err("Enter a title for each selected track.".into());
    }
    if metadata
        .release_year
        .is_some_and(|value| !(1000..=9999).contains(&value))
    {
        return Err("Enter a year from 1000 to 9999, or leave it blank.".into());
    }
    if metadata.track_number == Some(0) || metadata.disc_number == Some(0) {
        return Err("Track and disc numbers must be positive.".into());
    }
    if [
        &metadata.title,
        &metadata.artist,
        &metadata.album,
        &metadata.album_artist,
    ]
    .iter()
    .any(|value| value.len() > 16_384)
    {
        return Err("A metadata field is too long.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
