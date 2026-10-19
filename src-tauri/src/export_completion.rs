//! Successful exports keep their result in the current window, while durable
//! review data exists only for unfinished work. Terminal recovery records are
//! short-lived cleanup checkpoints, never a download history.
use crate::{recovery, reviews};
use audio_engine::ExportResult;
use std::{fs, path::Path};

const REVIEW_CLEANUP_WARNING: &str = "Your files were saved, but Jacket could not clear the unfinished review. Cleanup will be retried when Jacket opens again.";
const CACHE_CLEANUP_WARNING: &str =
    "Your files were saved, but some temporary download files could not be removed.";

fn remove_job(workdir: &Path) -> std::io::Result<()> {
    match fs::remove_dir_all(workdir) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

fn clear_review(
    store: &reviews::Store,
    review_root: &Path,
    record: &recovery::Recovery,
) -> Result<(), String> {
    if let Some(identity) = &record.review_identity {
        store.complete_export(review_root, identity)?;
    }
    Ok(())
}

/// Called only after the engine has published a complete output. Cleanup
/// errors are notes on that successful result, never a failed download.
pub fn finish(
    store: &reviews::Store,
    review_root: &Path,
    workdir: &Path,
    record: &mut recovery::Recovery,
    result: &mut ExportResult,
) {
    record.progress.stage = "complete".into();
    record.progress.completed = record.plan.tracks.len();
    record.output_path = result.output_path.clone();
    record.error = None;
    let checkpoint = record.save(workdir);
    if clear_review(store, review_root, record).is_err() {
        result.warnings.push(REVIEW_CLEANUP_WARNING.into());
        if checkpoint.is_err() {
            result.warnings.push("Jacket could not save the cleanup checkpoint. If the finished review is offered on restart, discard it; your exported files are already saved.".into());
        }
        return;
    }
    if remove_job(workdir).is_err() {
        result.warnings.push(CACHE_CLEANUP_WARNING.into());
        if checkpoint.is_err() {
            result.warnings.push("The completed download could not be marked as finished on disk. If offered again, discard its recovery instead of downloading another copy.".into());
        }
    }
}

/// Retry terminal cleanup before loading drafts. Failure leaves the record
/// and its assets intact and is surfaced by the native draft-load command.
pub fn reconcile(
    store: &reviews::Store,
    review_root: &Path,
    jobs_root: &Path,
) -> Result<(), String> {
    let records = recovery::completed(jobs_root).map_err(|error| error.to_string())?;
    let mut failed = false;
    for record in records {
        if clear_review(store, review_root, &record).is_err() {
            failed = true;
            continue;
        }
        let directory = recovery::job_directory(jobs_root, &record.job_id)
            .map_err(|error| error.to_string())?;
        // A cache-cleanup failure must not block a new review. The terminal
        // checkpoint stays on disk and is retried on the next startup.
        let _ = remove_job(&directory);
    }
    if failed {
        return Err("Your previous export was saved, but its unfinished review could not be cleared. Discard the saved review to start again.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use audio_engine::{AuthOptions, ExportPlan, OutputKind};

    fn draft(session: &str, revision: u64) -> reviews::ReviewDraft {
        serde_json::from_value(serde_json::json!({
            "schemaVersion": 1, "sessionId": session, "revision": revision,
            "url": "https://www.youtube.com/watch?v=fixture",
            "inspection": {
                "title": "Album", "sourceUrl": "https://www.youtube.com/watch?v=fixture",
                "suggestedMode": "album", "artworkUrl": null, "warnings": [],
                "tracks": [{
                    "id": "fixture", "sourceUrl": "https://www.youtube.com/watch?v=fixture",
                    "uploader": "", "duration": 60, "artworkUrl": null,
                    "sourceMetadata": { "title": "Song", "artist": "Artist", "album": "Album", "albumArtist": "Artist", "trackNumber": 1, "discNumber": null, "releaseYear": null },
                    "metadata": { "title": "Song", "artist": "Artist", "album": "Album", "albumArtist": "Artist", "trackNumber": 1, "discNumber": null, "releaseYear": null }
                }]
            },
            "selected": [0],
            "preferences": {
                "title": "Album", "mode": "album", "albumTitle": "Album", "albumArtist": "Artist", "year": null,
                "format": "mp3", "artworkMode": "none", "sharedArtwork": null, "sharedArtworkAsset": null, "outputKind": "zip"
            }
        })).unwrap()
    }

    fn record(identity: Option<reviews::CompletionIdentity>) -> recovery::Recovery {
        let review = draft("exported", 7);
        let mut record = recovery::Recovery::new(
            "job-finished".into(),
            ExportPlan {
                title: review.preferences.title,
                source_url: review.url,
                mode: review.preferences.mode,
                format: review.preferences.format,
                artwork_mode: review.preferences.artwork_mode,
                shared_artwork_url: None,
                shared_artwork_asset: None,
                shared_artwork_path: None,
                output_kind: OutputKind::Zip,
                auth: AuthOptions::default(),
                tracks: review.inspection.tracks,
            },
            "Album.zip".into(),
        );
        record.review_identity = identity;
        record
    }

    fn identity() -> reviews::CompletionIdentity {
        reviews::CompletionIdentity {
            session_id: "exported".into(),
            revision: 7,
        }
    }

    fn result(path: &Path) -> ExportResult {
        fs::write(path, b"completed music").unwrap();
        ExportResult {
            output_path: path.to_string_lossy().into_owned(),
            output_kind: OutputKind::Zip,
            playlist_path: None,
            track_count: 1,
            warnings: vec![],
        }
    }

    #[test]
    fn completed_export_clears_review_and_temporary_files_without_touching_output() {
        let dir = tempfile::tempdir().unwrap();
        let review_root = dir.path().join("review");
        let job = dir.path().join("jobs/job-finished");
        let store = reviews::Store::default();
        store.save(&review_root, draft("exported", 7)).unwrap();
        let mut record = record(Some(identity()));
        record.save(&job).unwrap();
        fs::write(job.join("source-cache"), b"cached audio").unwrap();
        let mut result = result(&dir.path().join("Album.zip"));

        finish(&store, &review_root, &job, &mut record, &mut result);

        assert!(result.warnings.is_empty());
        assert!(store.load(&review_root).unwrap().is_none());
        assert!(!store.save(&review_root, draft("exported", 7)).unwrap());
        assert!(!job.exists());
        assert_eq!(fs::read(&result.output_path).unwrap(), b"completed music");
    }

    #[test]
    fn completion_preserves_unrelated_and_newer_unfinished_reviews() {
        for (session, revision) in [("exported", 8), ("other-review", 1)] {
            let dir = tempfile::tempdir().unwrap();
            let review_root = dir.path().join("review");
            let job = dir.path().join("jobs/job-finished");
            let store = reviews::Store::default();
            store.save(&review_root, draft(session, revision)).unwrap();
            let mut record = record(Some(identity()));
            let mut result = result(&dir.path().join("Album.zip"));
            finish(&store, &review_root, &job, &mut record, &mut result);

            let retained = store.load(&review_root).unwrap().unwrap();
            assert_eq!(retained.session_id, session);
            assert_eq!(retained.revision, revision);
            assert!(!store.save(&review_root, draft("exported", 6)).unwrap());
            assert!(result.warnings.is_empty());
        }
    }

    #[test]
    fn cleanup_failure_keeps_a_successful_output_and_retries_from_terminal_checkpoint() {
        let dir = tempfile::tempdir().unwrap();
        let review_root = dir.path().join("review");
        let jobs = dir.path().join("jobs");
        let job = jobs.join("job-finished");
        let store = reviews::Store::default();
        // Simulate unavailable application data while the export destination
        // and job cache remain writable.
        fs::write(&review_root, b"unavailable directory").unwrap();
        let mut record = record(Some(identity()));
        let mut result = result(&dir.path().join("Album.zip"));
        finish(&store, &review_root, &job, &mut record, &mut result);

        assert_eq!(result.warnings.len(), 1);
        assert!(job.exists());
        assert!(recovery::latest(&jobs).is_none());
        assert_eq!(recovery::completed(&jobs).unwrap().len(), 1);
        assert_eq!(fs::read(&result.output_path).unwrap(), b"completed music");

        fs::remove_file(&review_root).unwrap();
        store.save(&review_root, draft("exported", 7)).unwrap();
        reconcile(&store, &review_root, &jobs).unwrap();
        assert!(store.load(&review_root).unwrap().is_none());
        assert!(!job.exists());
        assert_eq!(fs::read(&result.output_path).unwrap(), b"completed music");
    }

    #[test]
    fn legacy_completion_cleans_cache_without_guessing_review_ownership() {
        let dir = tempfile::tempdir().unwrap();
        let review_root = dir.path().join("review");
        let jobs = dir.path().join("jobs");
        let job = jobs.join("job-finished");
        let store = reviews::Store::default();
        store.save(&review_root, draft("exported", 7)).unwrap();
        let mut record = record(None);
        record.progress.stage = "complete".into();
        record.save(&job).unwrap();
        reconcile(&store, &review_root, &jobs).unwrap();
        assert!(store.load(&review_root).unwrap().is_some());
        assert!(!job.exists());
    }
}
