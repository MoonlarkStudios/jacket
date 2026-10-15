use audio_engine::{ExportPlan, ProgressEvent};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const RECORD: &str = "recovery.json";
const MAX_RECORD_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recovery {
    schema_version: u32,
    pub job_id: String,
    pub plan: ExportPlan,
    pub output_path: String,
    pub progress: ProgressEvent,
    pub error: Option<String>,
    updated_at: u64,
    #[serde(default)]
    dismissed: bool,
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
            progress: ProgressEvent {
                stage: "queued".into(),
                message: "Starting your download".into(),
                completed: 0,
                total,
            },
            error: None,
            updated_at: 0,
            dismissed: false,
        }
    }

    pub fn save(&mut self, workdir: &Path) -> std::io::Result<()> {
        self.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u64::MAX as u128) as u64;
        private_directory(workdir)?;
        let mut temporary = tempfile::NamedTempFile::new_in(workdir)?;
        temporary.write_all(&serde_json::to_vec(self)?)?;
        temporary.as_file_mut().sync_all()?;
        temporary
            .persist(workdir.join(RECORD))
            .map_err(|error| error.error)?;
        Ok(())
    }
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

fn load(directory: &Path) -> Option<Recovery> {
    if directory.symlink_metadata().ok()?.file_type().is_symlink() {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(directory.join(RECORD))
        .ok()?
        .take(MAX_RECORD_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_RECORD_BYTES {
        return None;
    }
    let record: Recovery = serde_json::from_slice(&bytes).ok()?;
    if record.schema_version != 1
        || !valid_job_id(&record.job_id)
        || directory.file_name()?.to_str()? != record.job_id
        || record.plan.tracks.is_empty()
    {
        return None;
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
        record.save(&directory)?;
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
}
