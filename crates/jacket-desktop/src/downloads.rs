//! Durable download recipes and receipts, independent of the editable draft.
use crate::desktop::PublishedExport;
use crate::recovery::{self, Recovery};
use crate::reviews::CompletionIdentity;
use crate::{Desktop, Job};
use audio_engine::{ExportPlan, ExportResult, OrganizedOutput};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct Download {
    pub recipe: Recovery,
    pub result: Option<ExportResult>,
    pub can_reveal: bool,
}

impl Download {
    pub fn saved(&self) -> bool {
        self.result.is_some() || self.recipe.progress.stage == "complete"
    }

    pub fn waiting(&self) -> bool {
        !self.saved()
            && matches!(
                self.recipe.progress.stage.as_str(),
                "queued" | "retry-queued"
            )
    }

    pub fn retryable(&self) -> bool {
        !self.waiting() && !self.saved() && self.recipe.plan.auth.managed_session.is_none()
    }
}

#[derive(Clone)]
pub enum DownloadDestination {
    Picked(PathBuf),
    Organized(OrganizedOutput),
}

#[derive(Deserialize, Serialize)]
struct Receipt {
    result: ExportResult,
    published: Option<PublishedExport>,
}

pub(crate) struct Claim {
    desktop: Desktop,
}

impl Drop for Claim {
    fn drop(&mut self) {
        if let Ok(mut active) = self.desktop.download_claim().lock() {
            *active = None;
        }
    }
}

const RECEIPT: &str = "receipt.json";

impl Desktop {
    pub(crate) fn downloads_root(&self) -> PathBuf {
        self.data_directory().join("downloads")
    }

    pub(crate) fn download_cache_root(&self) -> PathBuf {
        self.cache_directory().join("downloads")
    }

    pub fn downloads(&self) -> Result<Vec<Download>, String> {
        let root = self.downloads_root();
        check_directory(&root)?;
        let entries = match fs::read_dir(&root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.to_string()),
        };
        let mut items: Vec<_> = entries
            .filter_map(|entry| load(&entry.ok()?.path()))
            .collect();
        // Operation identifiers order first by creation time, then by sequence.
        items.sort_by(|first, second| first.recipe.job_id.cmp(&second.recipe.job_id));
        Ok(items)
    }

    pub fn queue_download(
        &self,
        plan: ExportPlan,
        destination: DownloadDestination,
        identity: CompletionIdentity,
    ) -> Result<Download, String> {
        let _guard = self
            .download_claim()
            .lock()
            .map_err(|_| "Downloads is unavailable.")?;
        if let Some(existing) = self
            .downloads()?
            .into_iter()
            .find(|item| item.recipe.review_identity.as_ref() == Some(&identity))
        {
            let _ = self.retire_queued_review(&identity);
            return Ok(existing);
        }
        let (path, organized) = match &destination {
            DownloadDestination::Picked(path) => (path.as_path(), None),
            DownloadDestination::Organized(output) => (output.path(), Some(output)),
        };
        let mut recipe = Self::export_recipe(plan, path, organized, Some(identity.clone()))?;
        let root = self.downloads_root();
        recovery::private_directory(&root).map_err(|error| error.to_string())?;
        crate::reviews::sync_directory(self.data_directory()).map_err(|error| error.to_string())?;
        let path =
            recovery::job_directory(&root, &recipe.job_id).map_err(|error| error.to_string())?;
        recipe.save(&path).map_err(|error| error.to_string())?;
        crate::reviews::sync_directory(&root).map_err(|error| error.to_string())?;
        // The recipe now owns the work. Startup can retry retiring its exact draft.
        let _ = self.retire_queued_review(&identity);
        Ok(Download {
            recipe,
            result: None,
            can_reveal: false,
        })
    }

    pub fn start_download(&self, id: &str) -> Result<Job, String> {
        let mut active = self
            .download_claim()
            .lock()
            .map_err(|_| "Downloads is unavailable.")?;
        if active.is_some() {
            return Err("Wait for the active download to finish.".into());
        }
        let item = load(&self.download_path(id)?).ok_or("This download is no longer available.")?;
        if !item.waiting() {
            return Err("This download isn’t waiting to start.".into());
        }
        let resume = item.recipe.progress.stage == "retry-queued";
        if resume && item.recipe.plan.auth.managed_session.is_some() {
            return Err("Reconnect your session and queue a new review.".into());
        }
        *active = Some(id.to_owned());
        drop(active);
        self.export_record(
            item.recipe,
            resume,
            Some(Claim {
                desktop: self.clone(),
            }),
        )
    }

    pub fn retry_download(&self, id: &str) -> Result<Download, String> {
        let active = self
            .download_claim()
            .lock()
            .map_err(|_| "Downloads is unavailable.")?;
        if active.as_deref() == Some(id) {
            return Err("Wait for this download to stop first.".into());
        }
        let path = self.download_path(id)?;
        let mut item = load(&path).ok_or("This download is no longer available.")?;
        if item.waiting() {
            return Ok(item);
        }
        if !item.retryable() {
            return Err("This download can’t be retried. For a connected session, reconnect it and queue a new review.".into());
        }
        item.recipe.progress.stage = "retry-queued".into();
        item.recipe.error = None;
        item.recipe
            .save_checkpoint(&path)
            .map_err(|error| error.to_string())?;
        Ok(item)
    }

    pub fn remove_download(&self, id: &str) -> Result<(), String> {
        let active = self
            .download_claim()
            .lock()
            .map_err(|_| "Downloads is unavailable.")?;
        if active.as_deref() == Some(id) {
            return Err("Cancel this active download before removing it.".into());
        }
        let path = self.download_path(id)?;
        if !path.exists() {
            return Ok(());
        }
        if let Some(identity) = load(&path).and_then(|item| item.recipe.review_identity) {
            self.retire_queued_review(&identity)?;
        }
        check_directory(&self.download_cache_root())?;
        let cache = recovery::job_directory(&self.download_cache_root(), id)
            .map_err(|error| error.to_string())?;
        remove_dir(&cache)
            .and_then(|()| remove_dir(&path))
            .and_then(|()| crate::reviews::sync_directory(&self.downloads_root()))
            .map_err(|error| error.to_string())
    }

    pub fn reveal_download(&self, id: &str) -> Result<(), String> {
        let path = self.download_path(id)?;
        let receipt: Receipt = recovery::read_record(&path.join(RECEIPT))
            .ok_or("This download has no saved files to open.")?;
        let output = Path::new(&receipt.result.output_path);
        if !output.is_absolute()
            || receipt.published.as_ref()
                != Some(&PublishedExport::read(output, receipt.result.output_kind)?)
        {
            return Err("The saved files are no longer at that location.".into());
        }
        self.reveal_published(output, receipt.result.output_kind)
    }

    fn download_path(&self, id: &str) -> Result<PathBuf, String> {
        let root = self.downloads_root();
        check_directory(&root)?;
        recovery::job_directory(&root, id).map_err(|error| error.to_string())
    }
}

fn load(directory: &Path) -> Option<Download> {
    let mut recipe = recovery::load(directory)?;
    let receipt = recovery::read_record::<Receipt>(&directory.join(RECEIPT));
    let can_reveal = receipt
        .as_ref()
        .is_some_and(|receipt| receipt.published.is_some());
    let result = receipt.map(|receipt| receipt.result);
    if let Some(result) = &result {
        recipe.progress.stage = "complete".into();
        recipe.progress.completed = recipe.progress.total;
        recipe.error = None;
        recipe.output_path.clone_from(&result.output_path);
    }
    Some(Download {
        recipe,
        result,
        can_reveal,
    })
}

pub(crate) fn finish(
    record_dir: &Path,
    workdir: &Path,
    record: &mut Recovery,
    result: &mut ExportResult,
) -> bool {
    record.progress.stage = "complete".into();
    record.progress.completed = record.progress.total;
    record.output_path.clone_from(&result.output_path);
    record.error = None;
    let checkpoint = record.save_checkpoint(record_dir);
    let published = PublishedExport::read(Path::new(&result.output_path), result.output_kind).ok();
    let can_reveal = published.is_some();
    let receipt = Receipt {
        result: result.clone(),
        published,
    };
    let saved = tempfile::NamedTempFile::new_in(record_dir)
        .and_then(|temporary| recovery::write_record(record_dir, RECEIPT, temporary, &receipt));
    if saved.is_err() && checkpoint.is_err() {
        result.warnings.push("Your files were saved, but Jacket couldn’t save their Downloads receipt. Check the destination before trying again.".into());
    } else if remove_dir(workdir).is_err() {
        result.warnings.push(
            "Your files were saved, but some temporary download files could not be removed.".into(),
        );
    }
    can_reveal && saved.is_ok()
}

fn check_directory(path: &Path) -> Result<(), String> {
    match path.symlink_metadata() {
        Ok(metadata) if !metadata.is_dir() => Err("The Downloads folder is unavailable.".into()),
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error.to_string()),
        _ => Ok(()),
    }
}

fn remove_dir(path: &Path) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use audio_engine::{
        AudioFormat, AuthOptions, CollectionMode, Inspection, Metadata, OutputKind, Track,
    };
    use jacket_core::Review;
    use std::time::{Duration, Instant};

    fn desktop(root: &Path) -> Desktop {
        Desktop::with_directories(
            root.join("data"),
            root.join("cache"),
            root.join("resources"),
            root.join("home"),
        )
        .unwrap()
    }

    fn song(title: &str) -> Review {
        let url = "https://www.youtube.com/watch?v=fixture0001".to_owned();
        let tags = Metadata {
            title: title.into(),
            artist: "Artist".into(),
            ..Metadata::default()
        };
        Review::new(
            Inspection {
                title: title.into(),
                source_url: url.clone(),
                suggested_mode: CollectionMode::Collection,
                artwork_url: None,
                album_release_year: None,
                tracks: vec![Track {
                    id: "fixture0001".into(),
                    source_url: url,
                    uploader: String::new(),
                    duration: None,
                    artwork_url: None,
                    source_metadata: tags.clone(),
                    metadata: tags,
                }],
                warnings: vec![],
            },
            AudioFormat::Mp3,
        )
    }

    fn queue(desktop: &Desktop, review: &Review, path: PathBuf) -> Download {
        desktop
            .queue_download(
                review.plan(AuthOptions::default()).unwrap(),
                DownloadDestination::Picked(path),
                review.identity(),
            )
            .unwrap()
    }

    #[test]
    fn publication_of_a_queue_recipe_retires_only_its_own_autosaved_revision() {
        let root = tempfile::tempdir().unwrap();
        let desktop = desktop(root.path());
        let first = song("First");
        let later = song("Later");
        assert!(desktop.save_review(first.draft()).unwrap());
        let queued = queue(&desktop, &first, root.path().join("First"));
        assert!(desktop.read_review().unwrap().is_none());
        assert!(desktop.save_review(later.draft()).unwrap());
        let duplicate = queue(&desktop, &first, root.path().join("Another location"));
        assert_eq!(duplicate.recipe.job_id, queued.recipe.job_id);
        assert_eq!(duplicate.recipe.output_path, queued.recipe.output_path);
        let _ = desktop.retire_queued_review(&first.identity()).unwrap();
        assert_eq!(
            desktop.read_review().unwrap().unwrap().session_id,
            later.identity().session_id
        );
    }

    #[test]
    fn restored_interrupted_downloads_wait_for_retry_and_removal_fences_stale_starts() {
        let root = tempfile::tempdir().unwrap();
        let service = desktop(root.path());
        let interrupted = queue(&service, &song("First"), root.path().join("First"));
        let next = queue(&service, &song("Second"), root.path().join("Second"));
        let id = interrupted.recipe.job_id.clone();
        let mut recipe = interrupted.recipe;
        recipe.progress.stage = "starting".into();
        recipe
            .save_checkpoint(&service.download_path(&id).unwrap())
            .unwrap();
        drop(service);

        let service = desktop(root.path());
        let restored = service.downloads().unwrap();
        assert_eq!(restored.len(), 2);
        assert_eq!(restored[0].recipe.job_id, id);
        assert!(!restored[0].waiting());
        assert!(restored[0].retryable());
        assert_eq!(restored[1].recipe.job_id, next.recipe.job_id);
        assert!(restored[1].waiting());
        assert!(service.start_download(&id).is_err());
        assert!(service.retry_download(&id).unwrap().waiting());
        service.remove_download(&next.recipe.job_id).unwrap();
        assert!(service.start_download(&next.recipe.job_id).is_err());
        assert_eq!(service.downloads().unwrap().len(), 1);
    }

    #[test]
    fn a_failed_destination_check_is_persisted_and_does_not_block_the_next_item() {
        let root = tempfile::tempdir().unwrap();
        let service = desktop(root.path());
        let removed = root.path().join("removed");
        fs::create_dir(&removed).unwrap();
        let failed = queue(&service, &song("First"), removed.join("First"));
        let next = queue(&service, &song("Second"), root.path().join("Second"));
        fs::remove_dir(removed).unwrap();

        let mut job = service.start_download(&failed.recipe.job_id).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while job.poll().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(job.is_finished());
        let saved = service.downloads().unwrap();
        assert_eq!(saved[0].recipe.progress.stage, "failed");
        assert!(saved[0].recipe.error.is_some());
        assert_eq!(saved[1].recipe.job_id, next.recipe.job_id);
        assert!(saved[1].waiting());
        assert!(service.pending_export().is_none());
    }

    #[test]
    fn saved_downloads_survive_restart_and_removal_preserves_the_output() {
        let root = tempfile::tempdir().unwrap();
        let service = desktop(root.path());
        let path = root.path().join("First.mp3");
        let queued = queue(&service, &song("First"), root.path().join("First"));
        let mut recipe = queued.recipe;
        let id = recipe.job_id.clone();
        let entry = service.download_path(&id).unwrap();
        let work = service.download_cache_root().join(&id);
        fs::create_dir_all(&work).unwrap();
        fs::write(&path, b"output").unwrap();
        let mut output = ExportResult {
            output_path: path.display().to_string(),
            output_kind: OutputKind::File,
            playlist_path: None,
            track_count: 1,
            album_save: None,
            warnings: vec![],
        };
        assert!(finish(&entry, &work, &mut recipe, &mut output));
        assert!(!work.exists());
        assert!(service.retry_download(&id).is_err());
        drop(service);

        let service = desktop(root.path());
        let completed = service.downloads().unwrap().pop().unwrap();
        assert!(completed.saved() && completed.can_reveal);
        assert_eq!(completed.recipe.output_path, path.to_string_lossy());
        fs::remove_file(entry.join(RECEIPT)).unwrap();
        let fallback = service.downloads().unwrap().pop().unwrap();
        assert!(fallback.saved());
        assert!(!fallback.retryable() && !fallback.can_reveal);
        assert!(service.start_download(&id).is_err());
        service.remove_download(&id).unwrap();
        assert!(service.downloads().unwrap().is_empty());
        assert_eq!(fs::read(path).unwrap(), b"output");
    }
}
