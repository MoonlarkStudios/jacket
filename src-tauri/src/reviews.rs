//! Private, versioned review storage. Export credentials are deliberately absent.
use audio_engine::{ArtworkMode, AudioFormat, CollectionMode, Inspection};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::Path,
    sync::Mutex,
};

const SCHEMA_VERSION: u32 = 1;
const RECORD: &str = "review.json";
const MAX_RECORD_BYTES: usize = 16 * 1024 * 1024;
const MAX_TRACKS: usize = 10_000;
const MAX_JS_INTEGER: u64 = 9_007_199_254_740_991;

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
    #[serde(default = "zip")]
    pub output_kind: String,
}

fn zip() -> String {
    "zip".into()
}

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

/// The exact committed review snapshot used to start an export. This token
/// contains no source data or credentials and remains valid across restarts.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompletionIdentity {
    pub session_id: String,
    pub revision: u64,
}

impl CompletionIdentity {
    pub fn validate(&self) -> Result<(), String> {
        validate_identity(&self.session_id, self.revision)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Record {
    schema_version: u32,
    session_id: String,
    revision: u64,
    retired_sessions: HashSet<String>,
    draft: Option<ReviewDraft>,
}

/// All reads, replacements, and discards share a lock, including the revision
/// check. A debounced old save cannot overwrite a newer save or a tombstone.
#[derive(Default)]
pub struct Store {
    serial: Mutex<()>,
}

impl Store {
    pub fn load(&self, root: &Path) -> Result<Option<ReviewDraft>, String> {
        let _guard = self.serial.lock().map_err(|_| unavailable())?;
        Ok(read(root)?.and_then(|record| record.draft))
    }

    /// Returns false when a delayed save belongs to an older revision/session.
    pub fn save(&self, root: &Path, draft: ReviewDraft) -> Result<bool, String> {
        validate(&draft)?;
        let _guard = self.serial.lock().map_err(|_| unavailable())?;
        let record = next_record(root, &draft.session_id, draft.revision)?;
        let Some(mut record) = record else {
            return Ok(false);
        };
        record.draft = Some(draft);
        write(root, &record)?;
        Ok(true)
    }

    /// Persist a tombstone rather than deleting the file: in-flight autosaves
    /// must not bring back a review the user explicitly discarded.
    pub fn discard(&self, root: &Path, session_id: &str, revision: u64) -> Result<bool, String> {
        validate_identity(session_id, revision)?;
        let _guard = self.serial.lock().map_err(|_| unavailable())?;
        // Explicit discard can also recover from a damaged/unknown record.
        // Reading and saving keep reporting such records instead of silently
        // overwriting them, while the user's discard remains usable.
        let next = next_record(root, session_id, revision).or_else(|_| {
            Ok::<_, String>(Some(Record {
                schema_version: SCHEMA_VERSION,
                session_id: session_id.into(),
                revision,
                retired_sessions: HashSet::new(),
                draft: None,
            }))
        })?;
        let Some(record) = next else {
            return Ok(false);
        };
        write(root, &record)?;
        Ok(true)
    }

    /// Clear only the snapshot associated with a successful export, retaining
    /// a tombstone so its delayed autosaves cannot restore the finished review.
    /// Newer edits and unrelated reviews keep their complete contents. An
    /// unrelated record also retires the finished session to fence old saves.
    ///
    /// Both Ok values mean completion has been reconciled: true cleared or
    /// fenced the matching snapshot; false preserved another/newer review or
    /// found an existing tombstone. Only Err requires retrying the cleanup.
    pub fn complete_export(
        &self,
        root: &Path,
        identity: &CompletionIdentity,
    ) -> Result<bool, String> {
        identity.validate()?;
        let _guard = self.serial.lock().map_err(|_| unavailable())?;
        let Some(mut record) = read(root)? else {
            write(
                root,
                &Record {
                    schema_version: SCHEMA_VERSION,
                    session_id: identity.session_id.clone(),
                    revision: identity.revision,
                    retired_sessions: HashSet::new(),
                    draft: None,
                },
            )?;
            return Ok(true);
        };
        if record.session_id != identity.session_id {
            if record.retired_sessions.insert(identity.session_id.clone()) {
                write(root, &record)?;
            }
            return Ok(false);
        }
        if record.revision > identity.revision {
            return Ok(false);
        }
        if record.revision == identity.revision && record.draft.is_none() {
            return Ok(false);
        }
        record.revision = identity.revision;
        record.draft = None;
        write(root, &record)?;
        Ok(true)
    }
}

fn next_record(root: &Path, session_id: &str, revision: u64) -> Result<Option<Record>, String> {
    let previous = read(root)?;
    let mut retired_sessions = HashSet::new();
    if let Some(previous) = previous {
        if previous.retired_sessions.contains(session_id)
            || (previous.session_id == session_id && revision <= previous.revision)
        {
            return Ok(None);
        }
        retired_sessions = previous.retired_sessions;
        if previous.session_id != session_id {
            retired_sessions.insert(previous.session_id);
        }
    }
    Ok(Some(Record {
        schema_version: SCHEMA_VERSION,
        session_id: session_id.into(),
        revision,
        retired_sessions,
        draft: None,
    }))
}

fn unavailable() -> String {
    "Your saved review is unavailable.".into()
}

fn validate_identity(session_id: &str, revision: u64) -> Result<(), String> {
    if session_id.is_empty()
        || session_id.len() > 100
        || !session_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        || revision == 0
        || revision > MAX_JS_INTEGER
    {
        return Err("Invalid review revision.".into());
    }
    Ok(())
}

fn validate(draft: &ReviewDraft) -> Result<(), String> {
    validate_identity(&draft.session_id, draft.revision)?;
    if draft.schema_version != SCHEMA_VERSION {
        return Err("This saved review needs a newer version of Jacket.".into());
    }
    let inspection = &draft.inspection;
    audio_engine::source::validate_source_url(&draft.url).map_err(|error| error.to_string())?;
    audio_engine::source::validate_source_url(&inspection.source_url)
        .map_err(|error| error.to_string())?;
    if inspection.tracks.is_empty() || inspection.tracks.len() > MAX_TRACKS {
        return Err("The saved review has an unsupported number of tracks.".into());
    }
    let mut indices = HashSet::new();
    if draft
        .selected
        .iter()
        .any(|index| *index >= inspection.tracks.len() || !indices.insert(*index))
    {
        return Err("Invalid saved track selection.".into());
    }
    let prefs = &draft.preferences;
    if !matches!(prefs.output_kind.as_str(), "zip" | "folder") {
        return Err("Invalid saved output destination.".into());
    }
    if prefs
        .shared_artwork_asset
        .as_ref()
        .is_some_and(|id| !crate::artwork_assets::valid_id(id))
    {
        return Err("Invalid saved artwork identifier.".into());
    }
    if prefs
        .shared_artwork
        .as_ref()
        .is_some_and(|url| !audio_engine::source::valid_artwork_url(url))
    {
        return Err("Invalid saved artwork source.".into());
    }
    if [
        &inspection.title,
        &prefs.title,
        &prefs.album_title,
        &prefs.album_artist,
    ]
    .iter()
    .any(|value| value.len() > 16_384)
    {
        return Err("A saved review field is too long.".into());
    }
    for track in &inspection.tracks {
        audio_engine::source::validate_source_url(&track.source_url)
            .map_err(|error| error.to_string())?;
        if track
            .duration
            .is_some_and(|duration| !duration.is_finite() || duration < 0.0)
        {
            return Err("Invalid saved track duration.".into());
        }
    }
    Ok(())
}

fn read(root: &Path) -> Result<Option<Record>, String> {
    if root
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(unavailable());
    }
    let path = root.join(RECORD);
    match path.symlink_metadata() {
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
            return Err(unavailable());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(unavailable()),
        _ => {}
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| unavailable())?
        .take(MAX_RECORD_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| unavailable())?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err("The saved review exceeds the 16 MiB limit.".into());
    }
    let record: Record = serde_json::from_slice(&bytes)
        .map_err(|_| "The saved review is damaged. Discard it to save a new review.".to_string())?;
    if record.schema_version != SCHEMA_VERSION {
        return Err("This saved review needs a newer version of Jacket.".into());
    }
    validate_identity(&record.session_id, record.revision)?;
    if record.retired_sessions.contains(&record.session_id)
        || record
            .retired_sessions
            .iter()
            .any(|session_id| validate_identity(session_id, 1).is_err())
    {
        return Err(unavailable());
    }
    if let Some(draft) = &record.draft {
        validate(draft)?;
        if draft.session_id != record.session_id || draft.revision != record.revision {
            return Err(unavailable());
        }
    }
    Ok(Some(record))
}

fn write(root: &Path, record: &Record) -> Result<(), String> {
    let bytes = serde_json::to_vec(record).map_err(|_| unavailable())?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err("The saved review exceeds the 16 MiB limit.".into());
    }
    private_directory(root).map_err(|_| unavailable())?;
    let mut temporary = tempfile::NamedTempFile::new_in(root).map_err(|_| unavailable())?;
    temporary.write_all(&bytes).map_err(|_| unavailable())?;
    temporary
        .as_file_mut()
        .sync_all()
        .map_err(|_| unavailable())?;
    temporary
        .persist(root.join(RECORD))
        .map_err(|_| unavailable())?;
    sync_directory(root).map_err(|_| unavailable())?;
    Ok(())
}

pub(crate) fn sync_directory(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    fs::File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

pub(crate) fn private_directory(path: &Path) -> std::io::Result<()> {
    if path
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Private storage is unavailable",
        ));
    }
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use audio_engine::{Metadata, Track};

    fn draft(session: &str, revision: u64) -> ReviewDraft {
        ReviewDraft {
            schema_version: 1,
            session_id: session.into(),
            revision,
            url: "https://www.youtube.com/watch?v=test".into(),
            inspection: Inspection {
                title: "Album".into(),
                source_url: "https://www.youtube.com/watch?v=test".into(),
                suggested_mode: CollectionMode::Album,
                artwork_url: None,
                album_release_year: None,
                tracks: vec![Track {
                    id: "test".into(),
                    source_url: "https://www.youtube.com/watch?v=test".into(),
                    uploader: String::new(),
                    duration: None,
                    artwork_url: None,
                    source_metadata: Metadata::default(),
                    metadata: Metadata {
                        title: "Edited".into(),
                        ..Metadata::default()
                    },
                }],
                warnings: Vec::new(),
            },
            selected: vec![0],
            preferences: Preferences {
                title: "Album".into(),
                mode: CollectionMode::Album,
                album_title: "Album".into(),
                album_artist: "Artist".into(),
                year: Some(2020),
                format: AudioFormat::Mp3,
                artwork_mode: ArtworkMode::None,
                shared_artwork: None,
                shared_artwork_asset: None,
                output_kind: "folder".into(),
            },
        }
    }

    #[test]
    fn restores_committed_review_and_ignores_stale_saves() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::default();
        assert!(store.save(root.path(), draft("first", 3)).unwrap());
        assert!(!store.save(root.path(), draft("first", 2)).unwrap());
        let restored = Store::default().load(root.path()).unwrap().unwrap();
        assert_eq!(restored.revision, 3);
        assert_eq!(restored.inspection.tracks[0].metadata.title, "Edited");
        assert_eq!(restored.selected, vec![0]);
        assert_eq!(restored.preferences.output_kind, "folder");
        assert_eq!(restored.preferences.year, Some(2020));
        assert!(store.discard(root.path(), "first", 4).unwrap());
        assert!(!store.save(root.path(), draft("first", 3)).unwrap());
        assert!(store.load(root.path()).unwrap().is_none());
        assert!(store.save(root.path(), draft("second", 1)).unwrap());
        assert!(!store.save(root.path(), draft("first", 99)).unwrap());
    }

    #[test]
    fn completion_clears_its_review_without_retaining_collection_data() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::default();
        store.save(root.path(), draft("first", 5)).unwrap();
        let identity = CompletionIdentity {
            session_id: "first".into(),
            revision: 5,
        };
        assert!(store.complete_export(root.path(), &identity).unwrap());
        let restarted = Store::default();
        assert!(restarted.load(root.path()).unwrap().is_none());
        assert!(!restarted.save(root.path(), draft("first", 4)).unwrap());
        assert!(!restarted.save(root.path(), draft("first", 5)).unwrap());
        let bytes = fs::read(root.path().join(RECORD)).unwrap();
        assert!(!restarted.complete_export(root.path(), &identity).unwrap());
        assert_eq!(fs::read(root.path().join(RECORD)).unwrap(), bytes);
        let record: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(record["draft"].is_null());
        assert!(!String::from_utf8(bytes).unwrap().contains("Edited"));
        // A later review in the same running app is still allowed.
        assert!(restarted.save(root.path(), draft("first", 6)).unwrap());
        assert!(!restarted.complete_export(root.path(), &identity).unwrap());
        assert_eq!(restarted.load(root.path()).unwrap().unwrap().revision, 6);
    }

    #[test]
    fn completion_fences_an_absent_or_older_matching_snapshot() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::default();
        let identity = CompletionIdentity {
            session_id: "first".into(),
            revision: 5,
        };
        assert!(store.complete_export(root.path(), &identity).unwrap());
        assert!(!store.save(root.path(), draft("first", 5)).unwrap());
        assert!(!store.save(root.path(), draft("first", 1)).unwrap());
        let other = tempfile::tempdir().unwrap();
        store.save(other.path(), draft("first", 3)).unwrap();
        assert!(store.complete_export(other.path(), &identity).unwrap());
        assert!(!store.save(other.path(), draft("first", 4)).unwrap());
        assert!(store.load(other.path()).unwrap().is_none());
    }

    #[test]
    fn completion_preserves_newer_or_unrelated_reviews_and_fences_old_saves() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::default();
        let identity = CompletionIdentity {
            session_id: "first".into(),
            revision: 5,
        };
        store.save(root.path(), draft("first", 6)).unwrap();
        let newer = fs::read(root.path().join(RECORD)).unwrap();
        assert!(!store.complete_export(root.path(), &identity).unwrap());
        assert_eq!(fs::read(root.path().join(RECORD)).unwrap(), newer);
        store.save(root.path(), draft("second", 1)).unwrap();
        let unrelated = serde_json::to_value(store.load(root.path()).unwrap().unwrap()).unwrap();
        assert!(!store.complete_export(root.path(), &identity).unwrap());
        assert_eq!(
            serde_json::to_value(store.load(root.path()).unwrap().unwrap()).unwrap(),
            unrelated
        );
        assert!(!store.save(root.path(), draft("first", 99)).unwrap());
        // Even if the original session was never persisted in this store,
        // completing it cannot let a queued save replace an unrelated review.
        let missing = CompletionIdentity {
            session_id: "missing".into(),
            revision: 7,
        };
        assert!(!store.complete_export(root.path(), &missing).unwrap());
        assert!(!store.save(root.path(), draft("missing", 7)).unwrap());
        assert_eq!(
            serde_json::to_value(store.load(root.path()).unwrap().unwrap()).unwrap(),
            unrelated
        );
    }

    #[test]
    fn completion_never_overwrites_unreadable_data_or_accepts_invalid_identity() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::default();
        let identity = CompletionIdentity {
            session_id: "first".into(),
            revision: 5,
        };
        fs::write(root.path().join(RECORD), b"damaged").unwrap();
        assert!(store.complete_export(root.path(), &identity).is_err());
        assert_eq!(fs::read(root.path().join(RECORD)).unwrap(), b"damaged");
        for (session, revision) in [
            ("", 5),
            ("../outside", 5),
            ("first", 0),
            ("first", MAX_JS_INTEGER + 1),
        ] {
            assert!(store
                .complete_export(
                    root.path(),
                    &CompletionIdentity {
                        session_id: session.into(),
                        revision
                    }
                )
                .is_err());
        }
    }

    #[test]
    fn completion_and_queued_autosaves_are_serialized() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::default();
        store.save(root.path(), draft("first", 1)).unwrap();
        std::thread::scope(|scope| {
            for revision in 2..=24 {
                let store = &store;
                let root = root.path();
                scope.spawn(move || store.save(root, draft("first", revision)).unwrap());
            }
            scope.spawn(|| {
                store
                    .complete_export(
                        root.path(),
                        &CompletionIdentity {
                            session_id: "first".into(),
                            revision: 24,
                        },
                    )
                    .unwrap()
            });
        });
        assert!(Store::default().load(root.path()).unwrap().is_none());
        assert!(!store.save(root.path(), draft("first", 24)).unwrap());
    }

    #[test]
    fn validates_schema_selection_and_rejects_credentials() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::default();
        let mut invalid = draft("first", 1);
        invalid.selected = vec![0, 0];
        assert!(store.save(root.path(), invalid).is_err());
        let mut invalid = draft("first", 1);
        invalid.schema_version = 2;
        assert!(store.save(root.path(), invalid).is_err());
        let mut value = serde_json::to_value(draft("first", 1)).unwrap();
        value["auth"] = serde_json::json!({"managedSession": "secret"});
        assert!(serde_json::from_value::<ReviewDraft>(value).is_err());
        let valid = draft("first", 1);
        store.save(root.path(), valid).unwrap();
        let bytes = fs::read(root.path().join(RECORD)).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(!text.contains("auth"));
        assert!(!text.contains("cookie"));
    }

    #[test]
    fn explicit_discard_recovers_from_an_unreadable_record() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join(RECORD), b"truncated-json").unwrap();
        let store = Store::default();
        assert!(store.load(root.path()).is_err());
        assert!(store.save(root.path(), draft("first", 1)).is_err());
        assert!(store.discard(root.path(), "first", 2).unwrap());
        assert!(store.load(root.path()).unwrap().is_none());
        assert!(store.save(root.path(), draft("first", 3)).unwrap());
    }

    #[test]
    fn concurrent_autosaves_and_discard_preserve_the_latest_revision() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::default();
        std::thread::scope(|scope| {
            for revision in 1..=24 {
                let store = &store;
                let root = root.path();
                scope.spawn(move || store.save(root, draft("first", revision)).unwrap());
            }
        });
        assert_eq!(store.load(root.path()).unwrap().unwrap().revision, 24);
        std::thread::scope(|scope| {
            scope.spawn(|| store.save(root.path(), draft("first", 25)).unwrap());
            scope.spawn(|| store.discard(root.path(), "first", 26).unwrap());
        });
        assert!(store.load(root.path()).unwrap().is_none());
    }

    #[test]
    fn oversized_save_preserves_the_last_valid_review() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::default();
        store.save(root.path(), draft("first", 1)).unwrap();
        let mut oversized = draft("first", 2);
        oversized.inspection.warnings = vec!["x".repeat(MAX_RECORD_BYTES)];
        assert!(store.save(root.path(), oversized).is_err());
        assert_eq!(store.load(root.path()).unwrap().unwrap().revision, 1);
        let file = fs::OpenOptions::new()
            .write(true)
            .open(root.path().join(RECORD))
            .unwrap();
        file.set_len(MAX_RECORD_BYTES as u64 + 1).unwrap();
        assert!(store.load(root.path()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn private_storage_rejects_symlink_and_keeps_restrictive_permissions() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let root = tempfile::tempdir().unwrap();
        let store = Store::default();
        let directory = root.path().join("reviews");
        store.save(&directory, draft("first", 1)).unwrap();
        assert_eq!(
            fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(directory.join(RECORD))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let alias = root.path().join("alias");
        symlink(&directory, &alias).unwrap();
        assert!(store.load(&alias).is_err());
        assert!(store.save(&alias, draft("first", 2)).is_err());
    }
}
