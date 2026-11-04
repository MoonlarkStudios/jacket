use audio_engine::{Metadata, Track};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Default)]
pub struct TitleCleanup {
    pub prefix: String,
    pub suffix: String,
}

/// Absent values retain each occurrence's tags; Some(None) explicitly clears year.
#[derive(Clone, Debug, Default)]
pub struct BulkEdits {
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub year: Option<Option<u32>>,
    pub title_cleanup: Option<TitleCleanup>,
}

#[derive(Clone, Debug)]
pub struct BulkPreviewRow {
    pub index: usize,
    pub before: Metadata,
    pub after: Metadata,
    pub changed: bool,
}

#[derive(Clone, Debug, Default)]
pub struct BulkPreview {
    pub rows: Vec<BulkPreviewRow>,
    pub changed_count: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct Change {
    pub index: usize,
    pub id: String,
    pub source_url: String,
    pub before: Metadata,
    pub after: Metadata,
}

pub(crate) fn preview(
    tracks: &[Track],
    selected: &BTreeSet<usize>,
    edits: &BulkEdits,
) -> Result<BulkPreview, String> {
    if edits
        .year
        .flatten()
        .is_some_and(|year| !(1000..=9999).contains(&year))
    {
        return Err("Enter a year from 1000 to 9999, or clear the field.".into());
    }
    if [&edits.artist, &edits.album, &edits.album_artist]
        .into_iter()
        .any(|value| value.as_ref().is_some_and(|value| value.len() > 16_384))
    {
        return Err("A metadata field is too long.".into());
    }
    let mut result = BulkPreview::default();
    for (index, track) in tracks.iter().enumerate() {
        if !selected.contains(&index) {
            continue;
        }
        let mut after = track.metadata.clone();
        if let Some(value) = &edits.artist {
            after.artist = value.trim().into();
        }
        if let Some(value) = &edits.album {
            after.album = value.trim().into();
        }
        if let Some(value) = &edits.album_artist {
            after.album_artist = value.trim().into();
        }
        if let Some(value) = edits.year {
            after.release_year = value;
        }
        if let Some(cleanup) = &edits.title_cleanup {
            let mut title = track.metadata.title.as_str();
            if !cleanup.prefix.is_empty() {
                title = title.strip_prefix(&cleanup.prefix).unwrap_or(title);
            }
            if !cleanup.suffix.is_empty() {
                title = title.strip_suffix(&cleanup.suffix).unwrap_or(title);
            }
            if title != track.metadata.title {
                after.title = title.trim().into();
                if after.title.is_empty() {
                    return Err(format!("Track {} would have an empty title.", index + 1));
                }
            }
        }
        let changed = !same_metadata(&track.metadata, &after);
        result.changed_count += usize::from(changed);
        result.rows.push(BulkPreviewRow {
            index,
            before: track.metadata.clone(),
            after,
            changed,
        });
    }
    Ok(result)
}

pub(crate) fn undo(tracks: &mut [Track], changes: &[Change]) -> bool {
    let mut reverted = false;
    for change in changes {
        let Some(track) = tracks.get_mut(change.index) else {
            continue;
        };
        if track.id != change.id || track.source_url != change.source_url {
            continue;
        }
        let current = &mut track.metadata;
        // Each field has its own compare-and-restore fence, so later manual edits survive.
        macro_rules! restore {
            ($field:ident) => {
                if change.before.$field != change.after.$field
                    && current.$field == change.after.$field
                {
                    current.$field = change.before.$field.clone();
                    reverted = true;
                }
            };
        }
        restore!(title);
        restore!(artist);
        restore!(album);
        restore!(album_artist);
        restore!(release_year);
    }
    reverted
}

pub(crate) fn same_metadata(left: &Metadata, right: &Metadata) -> bool {
    left.title == right.title
        && left.artist == right.artist
        && left.album == right.album
        && left.album_artist == right.album_artist
        && left.track_number == right.track_number
        && left.disc_number == right.disc_number
        && left.release_year == right.release_year
}
