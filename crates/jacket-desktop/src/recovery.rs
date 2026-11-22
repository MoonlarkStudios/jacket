use crate::reviews::CompletionIdentity;
use audio_engine::{ExportPlan, ProgressEvent};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const RECORD: &str = "recovery.json";
const CHECKPOINT: &str = "recovery-progress.json";
const SCHEMA_VERSION: u32 = 2;
const MAX_RECORD_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recovery {
    schema_version: u32,
    pub job_id: String,
    pub plan: ExportPlan,
    pub output_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) music_root: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) music_album_stems: Option<Vec<String>>,
    pub progress: ProgressEvent,
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub review_identity: Option<CompletionIdentity>,
    updated_at: u64,
    #[serde(default)]
    dismissed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    checkpoint_key: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Checkpoint {
    job_id: String,
    key: String,
    progress: ProgressEvent,
    error: Option<String>,
    updated_at: u64,
    dismissed: bool,
    completed_path: Option<String>,
}

pub fn valid_job_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id != "inspection"
        && id != "downloader-update"
        && id != "session"
        && id != "browser-access"
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

impl Recovery {
    pub fn new(job_id: String, plan: ExportPlan, output_path: String) -> Self {
        let total = plan.tracks.len();
        Self {
            schema_version: 1,
            job_id,
            plan,
            output_path,
            music_root: None,
            music_album_stems: None,
            progress: ProgressEvent {
                stage: "queued".into(),
                message: "Starting your download".into(),
                completed: 0,
                total,
            },
            error: None,
            review_identity: None,
            updated_at: 0,
            dismissed: false,
            checkpoint_key: None,
        }
    }

    /// Persist the export plan and a baseline state before an attempt.
    pub fn save(&mut self, workdir: &Path) -> std::io::Result<()> {
        if let Some(identity) = &self.review_identity {
            identity
                .validate()
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
        }
        self.updated_at = timestamp();
        private_directory(workdir)?;
        let temporary = tempfile::Builder::new()
            .rand_bytes(16)
            .tempfile_in(workdir)?;
        // Bind checkpoints to this snapshot so a crash between writing a new
        // baseline and its first progress update cannot replay an older attempt.
        let key = temporary
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .expect("temporary record filenames are ASCII");
        self.schema_version = SCHEMA_VERSION;
        self.checkpoint_key = Some(key.to_owned());
        write_record(workdir, RECORD, temporary, self)
    }

    pub(crate) fn save_checkpoint(&mut self, workdir: &Path) -> std::io::Result<()> {
        let Some(key) = &self.checkpoint_key else {
            return self.save(workdir);
        };
        private_directory(workdir)?;
        if !workdir
            .join(RECORD)
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.is_file())
        {
            return self.save(workdir);
        }
        self.updated_at = timestamp();
        let checkpoint = Checkpoint {
            job_id: self.job_id.clone(),
            key: key.clone(),
            progress: self.progress.clone(),
            error: self.error.clone(),
            updated_at: self.updated_at,
            dismissed: self.dismissed,
            completed_path: (self.progress.stage == "complete").then(|| self.output_path.clone()),
        };
        let temporary = tempfile::NamedTempFile::new_in(workdir)?;
        write_record(workdir, CHECKPOINT, temporary, &checkpoint)
    }
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

fn write_record(
    workdir: &Path,
    name: &str,
    mut temporary: tempfile::NamedTempFile,
    value: &impl Serialize,
) -> std::io::Result<()> {
    temporary.write_all(&serde_json::to_vec(value)?)?;
    temporary.as_file_mut().sync_all()?;
    temporary
        .persist(workdir.join(name))
        .map_err(|error| error.error)?;
    crate::reviews::sync_directory(workdir)
}

fn private_directory(path: &Path) -> std::io::Result<()> {
    if path
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Download cache is unavailable",
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
        // Also protect retained jobs created by older versions or a loose umask.
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn read_record<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let metadata = path.symlink_metadata().ok()?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_RECORD_BYTES
    {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(MAX_RECORD_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_RECORD_BYTES {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

fn load(directory: &Path) -> Option<Recovery> {
    if directory.symlink_metadata().ok()?.file_type().is_symlink() {
        return None;
    }
    let mut record: Recovery = read_record(&directory.join(RECORD))?;
    if !valid_job_id(&record.job_id)
        || directory.file_name()?.to_str()? != record.job_id
        || record.plan.tracks.is_empty()
        || (record.plan.output_kind == audio_engine::OutputKind::File
            && record.plan.tracks.len() != 1)
        || record
            .review_identity
            .as_ref()
            .is_some_and(|identity| identity.validate().is_err())
    {
        return None;
    }
    match (record.schema_version, record.checkpoint_key.as_deref()) {
        (1, None) => (),
        (SCHEMA_VERSION, Some(key)) => {
            if let Some(checkpoint) =
                read_record::<Checkpoint>(&directory.join(CHECKPOINT)).filter(|checkpoint| {
                    checkpoint.job_id == record.job_id
                        && checkpoint.key == key
                        && (checkpoint.progress.stage == "complete")
                            == checkpoint.completed_path.is_some()
                })
            {
                record.progress = checkpoint.progress;
                record.error = checkpoint.error;
                record.updated_at = checkpoint.updated_at;
                record.dismissed = checkpoint.dismissed;
                if let Some(path) = checkpoint.completed_path {
                    record.output_path = path;
                }
            }
        }
        _ => return None,
    }
    Some(record)
}

pub fn latest(root: &Path) -> Option<Recovery> {
    fs::read_dir(root)
        .ok()?
        .filter_map(|entry| load(&entry.ok()?.path()))
        .filter(|record| {
            !record.dismissed
                && record.progress.stage != "complete"
                && record.plan.auth.managed_session.is_none()
        })
        .max_by_key(|record| record.updated_at)
}

/// Startup maintenance only: successful exports may retain a terminal record
/// until their associated review has been cleared. These records are never
/// offered as interrupted downloads by `latest`. The caller may delete each
/// job directory only after its conditional review cleanup returns Ok.
/// Malformed records are left untouched; older complete records without a
/// review identity may have their cache removed without touching any draft.
pub fn completed(root: &Path) -> std::io::Result<Vec<Recovery>> {
    if root
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Download cache is unavailable",
        ));
    }
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut records = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        if let Some(record) = load(&entry.path()) {
            if record.progress.stage == "complete" {
                records.push(record);
            }
        }
    }
    Ok(records)
}

/// Inventory all retained jobs before startup-only artwork cleanup. An
/// unreadable record conservatively prevents cleanup rather than losing art.
pub fn artwork_asset_ids(root: &Path) -> std::io::Result<std::collections::HashSet<String>> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Default::default()),
        Err(error) => return Err(error),
    };
    let mut ids = std::collections::HashSet::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let record = load(&entry.path()).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Download recovery inventory is unavailable",
            )
        })?;
        if let Some(id) = record.plan.shared_artwork_asset {
            ids.insert(id);
        }
    }
    Ok(ids)
}

pub fn clear_managed(root: &Path) -> std::io::Result<()> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let path = entry?.path();
        if load(&path).is_some_and(|record| record.plan.auth.managed_session.is_some()) {
            fs::remove_dir_all(path)?;
        }
    }
    Ok(())
}

pub fn dismiss(root: &Path, job_id: &str) -> std::io::Result<()> {
    if !valid_job_id(job_id) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid download identifier",
        ));
    }
    let directory = root.join(job_id);
    if let Some(mut record) = load(&directory) {
        record.dismissed = true;
        record.save_checkpoint(&directory)?;
    }
    Ok(())
}

pub fn job_directory(root: &Path, job_id: &str) -> std::io::Result<PathBuf> {
    if !valid_job_id(job_id) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid download identifier",
        ));
    }
    let path = root.join(job_id);
    if path
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Download cache is unavailable",
        ));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use audio_engine::*;

    fn fixture(id: &str) -> Recovery {
        Recovery::new(
            id.into(),
            ExportPlan {
                title: "Album".into(),
                source_url: "https://music.youtube.com/playlist?list=OLAK5uy_test".into(),
                mode: CollectionMode::Album,
                format: AudioFormat::Mp3,
                artwork_mode: ArtworkMode::None,
                shared_artwork_url: None,
                shared_artwork_asset: None,
                shared_artwork_path: None,
                output_kind: audio_engine::OutputKind::Zip,
                auth: AuthOptions::default(),
                tracks: vec![Track {
                    id: "a".into(),
                    source_url: "https://www.youtube.com/watch?v=a".into(),
                    uploader: String::new(),
                    duration: None,
                    artwork_url: None,
                    source_metadata: Metadata::default(),
                    metadata: Metadata {
                        title: "Song".into(),
                        track_number: Some(3),
                        release_year: Some(1976),
                        ..Metadata::default()
                    },
                }],
            },
            "/tmp/Album.zip".into(),
        )
    }

    #[cfg(unix)]
    #[test]
    fn cached_media_and_recovery_are_private_even_for_existing_jobs() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("job-private");
        let mut record = fixture("job-private");
        record.save(&directory).unwrap();
        assert_eq!(
            fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).unwrap();
        record.save(&directory).unwrap();
        assert_eq!(
            fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let alias = root.path().join("alias");
        symlink(&directory, &alias).unwrap();
        assert!(record.save(&alias).is_err());
    }

    #[test]
    fn a_restart_restores_the_plan_destination_and_completed_progress() {
        let root = tempfile::tempdir().unwrap();
        let mut record = fixture("job-1");
        record.progress.stage = "preparing".into();
        record.progress.completed = 1;
        record.save(&root.path().join("job-1")).unwrap();
        let restored = latest(root.path()).unwrap();
        assert_eq!(restored.output_path, "/tmp/Album.zip");
        assert_eq!(restored.progress.completed, 1);
        assert_eq!(restored.plan.tracks[0].metadata.track_number, Some(3));
        assert_eq!(restored.plan.tracks[0].metadata.release_year, Some(1976));
        assert!(root.path().join("job-1/recovery.json").is_file());
        dismiss(root.path(), "job-1").unwrap();
        assert!(latest(root.path()).is_none());
        assert!(root.path().join("job-1/recovery.json").is_file());
    }

    #[test]
    fn progress_failure_and_dismissal_checkpoint_without_rewriting_the_plan() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("long-export");
        let mut record = fixture("long-export");
        record.plan.tracks = vec![record.plan.tracks[0].clone(); 1_000];
        record.progress.total = record.plan.tracks.len();
        record.save(&directory).unwrap();
        let baseline = fs::read(directory.join(RECORD)).unwrap();

        record.progress.stage = "preparing".into();
        record.progress.completed = 413;
        record.save_checkpoint(&directory).unwrap();
        let progress = fs::read(directory.join(CHECKPOINT)).unwrap();
        assert!(progress.len() < 1_024);
        let restored = latest(root.path()).unwrap();
        assert_eq!(restored.progress.completed, 413);
        assert_eq!(restored.progress.total, 1_000);
        assert_eq!(restored.plan.tracks.len(), 1_000);
        assert_eq!(restored.output_path, "/tmp/Album.zip");

        record.progress.stage = "failed".into();
        record.error = Some("Tool failed.".into());
        record.save_checkpoint(&directory).unwrap();
        let restored = latest(root.path()).unwrap();
        assert_eq!(restored.progress.stage, "failed");
        assert_eq!(restored.error.as_deref(), Some("Tool failed."));

        dismiss(root.path(), "long-export").unwrap();
        assert!(latest(root.path()).is_none());
        assert_eq!(fs::read(directory.join(RECORD)).unwrap(), baseline);
    }

    #[test]
    fn checkpoint_from_an_earlier_baseline_does_not_replace_restart_state() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("resumed");
        let mut record = fixture("resumed");
        record.save(&directory).unwrap();
        record.progress.stage = "failed".into();
        record.error = Some("First attempt failed.".into());
        record.save_checkpoint(&directory).unwrap();

        let mut resumed = latest(root.path()).unwrap();
        resumed.progress.stage = "preparing".into();
        resumed.error = None;
        resumed.save(&directory).unwrap();
        let restarted = latest(root.path()).unwrap();
        assert_eq!(restarted.progress.stage, "preparing");
        assert!(restarted.error.is_none());

        resumed.progress.stage = "packaging".into();
        resumed.save_checkpoint(&directory).unwrap();
        assert_eq!(latest(root.path()).unwrap().progress.stage, "packaging");
    }

    #[test]
    fn a_damaged_checkpoint_leaves_the_durable_plan_available_for_resume() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("fallback");
        let mut record = fixture("fallback");
        record.plan.shared_artwork_asset = Some("existing-cover".into());
        record.save(&directory).unwrap();
        fs::write(directory.join(CHECKPOINT), b"incomplete checkpoint").unwrap();

        let restored = latest(root.path()).unwrap();
        assert_eq!(restored.progress.stage, "queued");
        assert_eq!(restored.output_path, "/tmp/Album.zip");
        assert!(artwork_asset_ids(root.path())
            .unwrap()
            .contains("existing-cover"));
    }

    #[test]
    fn file_recovery_retains_the_destination_stem_format_and_album_metadata() {
        let root = tempfile::tempdir().unwrap();
        let mut record = fixture("file-job");
        record.plan.output_kind = OutputKind::File;
        record.plan.format = AudioFormat::Preserve;
        record.output_path = "/tmp/Artist - Song.v2".into();
        record.progress.stage = "preparing".into();
        record.save(&root.path().join("file-job")).unwrap();

        let restored = latest(root.path()).unwrap();
        assert_eq!(restored.output_path, "/tmp/Artist - Song.v2");
        assert!(restored.music_root.is_none());
        assert_eq!(restored.plan.output_kind, OutputKind::File);
        assert!(matches!(restored.plan.format, AudioFormat::Preserve));
        assert_eq!(restored.plan.mode, CollectionMode::Album);
        assert_eq!(restored.plan.tracks[0].metadata.track_number, Some(3));
        assert_eq!(restored.plan.tracks[0].metadata.release_year, Some(1976));
    }

    #[test]
    fn organized_recovery_restores_the_original_root_and_resolved_path() {
        let data = tempfile::tempdir().unwrap();
        let root = data.path().join("External music");
        let stem = root.join("Artist/Album/Song");
        let mut record = fixture("music-folder-job");
        record.plan.output_kind = OutputKind::File;
        record.output_path = stem.to_str().unwrap().into();
        record.music_root = Some(root.clone());
        record.save(&data.path().join("music-folder-job")).unwrap();

        let restored = latest(data.path()).unwrap();
        assert_eq!(restored.music_root.as_deref(), Some(root.as_path()));
        assert_eq!(Path::new(&restored.output_path), stem);
        assert!(
            !root.exists(),
            "a disconnected root should still be recoverable"
        );
    }

    #[test]
    fn file_recovery_rejects_plans_without_exactly_one_track() {
        for count in [0, 2] {
            let root = tempfile::tempdir().unwrap();
            let mut record = fixture("invalid-file");
            record.plan.output_kind = OutputKind::File;
            record.plan.tracks = vec![record.plan.tracks[0].clone(); count];
            record.save(&root.path().join("invalid-file")).unwrap();
            assert!(latest(root.path()).is_none());
            assert!(root.path().join("invalid-file/recovery.json").exists());
        }
    }

    #[test]
    fn legacy_recovery_without_output_kind_remains_a_zip() {
        let root = tempfile::tempdir().unwrap();
        let record = fixture("legacy-zip");
        let mut serialized = serde_json::to_value(record).unwrap();
        serialized["plan"]
            .as_object_mut()
            .unwrap()
            .remove("outputKind");
        let directory = root.path().join("legacy-zip");
        fs::create_dir(&directory).unwrap();
        fs::write(
            directory.join(RECORD),
            serde_json::to_vec(&serialized).unwrap(),
        )
        .unwrap();

        let mut restored = latest(root.path()).unwrap();
        assert_eq!(restored.plan.output_kind, OutputKind::Zip);
        assert_eq!(restored.output_path, "/tmp/Album.zip");
        assert_eq!(restored.schema_version, 1);

        restored.progress.stage = "preparing".into();
        restored.save_checkpoint(&directory).unwrap();
        let migrated = latest(root.path()).unwrap();
        assert_eq!(migrated.schema_version, SCHEMA_VERSION);
        assert_eq!(migrated.progress.stage, "preparing");
        assert_eq!(migrated.plan.output_kind, OutputKind::Zip);
    }

    #[test]
    fn completed_corrupt_and_mismatched_records_are_not_offered() {
        let root = tempfile::tempdir().unwrap();
        let mut record = fixture("complete");
        record.progress.stage = "complete".into();
        record.save(&root.path().join("complete")).unwrap();
        fixture("wrong-id")
            .save(&root.path().join("different-folder"))
            .unwrap();
        fs::create_dir(root.path().join("broken")).unwrap();
        fs::write(root.path().join("broken/recovery.json"), b"not JSON").unwrap();
        assert!(latest(root.path()).is_none());
        assert!(!valid_job_id("../escape"));
        assert!(!valid_job_id("inspection"));
        assert!(dismiss(root.path(), "../escape").is_err());
    }

    #[test]
    fn completed_inventory_preserves_identity_and_ignores_unfinished_or_malformed_jobs() {
        let root = tempfile::tempdir().unwrap();
        let mut complete = fixture("finished");
        complete.review_identity = Some(CompletionIdentity {
            session_id: "review-one".into(),
            revision: 7,
        });
        complete.progress.stage = "complete".into();
        complete.dismissed = true;
        complete.save(&root.path().join("finished")).unwrap();
        for stage in ["queued", "preparing", "failed", "cancelled"] {
            let mut record = fixture(stage);
            record.progress.stage = stage.into();
            record.save(&root.path().join(stage)).unwrap();
        }
        dismiss(root.path(), "cancelled").unwrap();
        fs::create_dir(root.path().join("malformed")).unwrap();
        fs::write(root.path().join("malformed/recovery.json"), b"damaged").unwrap();
        let records = completed(root.path()).unwrap();
        assert_eq!(records.len(), 1);
        let restored = &records[0];
        assert_eq!(restored.job_id, "finished");
        assert_eq!(restored.review_identity, complete.review_identity);
        assert_eq!(restored.plan.title, complete.plan.title);
        assert_eq!(restored.output_path, complete.output_path);
        assert_ne!(latest(root.path()).unwrap().progress.stage, "complete");
        assert!(root.path().join("malformed/recovery.json").exists());
        assert!(completed(&root.path().join("missing")).unwrap().is_empty());
    }

    #[test]
    fn older_completed_recovery_defaults_to_no_review_identity() {
        let root = tempfile::tempdir().unwrap();
        let mut record = fixture("legacy");
        record.progress.stage = "complete".into();
        let mut serialized = serde_json::to_value(&record).unwrap();
        serialized.as_object_mut().unwrap().remove("reviewIdentity");
        let directory = root.path().join("legacy");
        fs::create_dir(&directory).unwrap();
        fs::write(
            directory.join(RECORD),
            serde_json::to_vec(&serialized).unwrap(),
        )
        .unwrap();
        let restored = completed(root.path()).unwrap();
        assert_eq!(restored.len(), 1);
        assert!(restored[0].review_identity.is_none());
        assert!(latest(root.path()).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn completed_inventory_never_follows_a_cache_or_record_symlink() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let jobs = root.path().join("jobs");
        let mut record = fixture("finished");
        record.progress.stage = "complete".into();
        record.save(&jobs.join("finished")).unwrap();
        let alias = root.path().join("alias");
        symlink(&jobs, &alias).unwrap();
        assert!(completed(&alias).is_err());
        let outside = root.path().join("outside.json");
        fs::rename(jobs.join("finished/recovery.json"), &outside).unwrap();
        symlink(&outside, jobs.join("finished/recovery.json")).unwrap();
        assert!(completed(&jobs).unwrap().is_empty());
        assert!(outside.exists());
    }

    #[test]
    fn invalid_completion_identity_is_neither_saved_nor_replayed() {
        let root = tempfile::tempdir().unwrap();
        let mut record = fixture("invalid");
        record.progress.stage = "complete".into();
        record.review_identity = Some(CompletionIdentity {
            session_id: "review-one".into(),
            revision: 0,
        });
        let directory = root.path().join("invalid");
        assert!(record.save(&directory).is_err());
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join(RECORD), serde_json::to_vec(&record).unwrap()).unwrap();
        assert!(completed(root.path()).unwrap().is_empty());
        assert!(directory.join(RECORD).exists());
    }

    #[test]
    fn completed_checkpoint_retries_review_cleanup_after_restart() {
        use crate::reviews::{Preferences, ReviewDraft, Store};
        let root = tempfile::tempdir().unwrap();
        let jobs = root.path().join("jobs");
        let drafts = root.path().join("drafts");
        let mut record = fixture("finished");
        let identity = CompletionIdentity {
            session_id: "review-one".into(),
            revision: 7,
        };
        let draft = ReviewDraft {
            schema_version: 1,
            session_id: identity.session_id.clone(),
            revision: identity.revision,
            url: record.plan.source_url.clone(),
            inspection: Inspection {
                title: record.plan.title.clone(),
                source_url: record.plan.source_url.clone(),
                suggested_mode: record.plan.mode,
                artwork_url: None,
                album_release_year: None,
                tracks: record.plan.tracks.clone(),
                warnings: Vec::new(),
            },
            selected: vec![0],
            preferences: Preferences {
                title: record.plan.title.clone(),
                mode: record.plan.mode,
                album_title: "Album".into(),
                album_artist: String::new(),
                year: None,
                format: record.plan.format,
                artwork_mode: record.plan.artwork_mode,
                shared_artwork: None,
                shared_artwork_asset: None,
                output_kind: audio_engine::OutputKind::Zip,
            },
        };
        Store::default().save(&drafts, draft).unwrap();
        let saved_draft = fs::read(drafts.join("review.json")).unwrap();
        record.review_identity = Some(identity.clone());
        record.progress.stage = "complete".into();
        record.save(&jobs.join("finished")).unwrap();
        // A cleanup failure preserves the durable success checkpoint. It
        // cannot offer the already-published export as an interrupted job.
        fs::write(drafts.join("review.json"), b"temporarily unreadable").unwrap();
        assert!(Store::default()
            .complete_export(&drafts, &identity)
            .is_err());
        assert!(latest(&jobs).is_none());
        assert_eq!(completed(&jobs).unwrap().len(), 1);
        fs::write(drafts.join("review.json"), saved_draft).unwrap();
        let restarted = Store::default();
        for complete in completed(&jobs).unwrap() {
            let identity = complete.review_identity.as_ref().unwrap();
            restarted.complete_export(&drafts, identity).unwrap();
            fs::remove_dir_all(job_directory(&jobs, &complete.job_id).unwrap()).unwrap();
        }
        assert!(restarted.load(&drafts).unwrap().is_none());
        assert!(completed(&jobs).unwrap().is_empty());
        assert!(latest(&jobs).is_none());
    }
}
