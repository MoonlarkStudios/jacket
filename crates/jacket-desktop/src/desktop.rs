use crate::artwork_assets::{self, Asset, Crop};
use crate::auth_session::{self, Control, Keychain};
use crate::browser_sessions::{self, Catalog, Locations};
use crate::catalog::SearchRequest;
use crate::downloader_updates::{self, Channel, Preferences};
use crate::downloads;
use crate::export_completion;
use crate::recovery::{self, Recovery};
use crate::reviews::{self, CompletionIdentity, ReviewDraft};
use crate::worker::{Admission, Event, Job, Lease};
use crate::youtube_music::{self, Runtime as YouTubeMusicRuntime};
use audio_engine::{
    AuthOptions, Browser, Cancellation, ExportPlan, ExportResult, OutputKind, Toolchain,
};
use directories::BaseDirs;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const IDENTIFIER: &str = "com.moonlark.audio-dl";
static NEXT_OPERATION: AtomicU64 = AtomicU64::new(1);

#[derive(serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub(crate) struct PublishedExport {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    created: Option<SystemTime>,
    canonical_path: PathBuf,
    kind: OutputKind,
}

impl PublishedExport {
    pub(crate) fn read(path: &Path, kind: OutputKind) -> Result<Self, String> {
        let unavailable = "This export is no longer available at the saved location.";
        let metadata = path.symlink_metadata().map_err(|_| unavailable)?;
        let expected_type = if kind == OutputKind::Folder {
            metadata.is_dir()
        } else {
            metadata.is_file()
        };
        if metadata.file_type().is_symlink() || !expected_type {
            return Err(unavailable.into());
        }
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Ok(Self {
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
            created: metadata.created().ok(),
            canonical_path: path.canonicalize().map_err(|_| unavailable)?,
            kind,
        })
    }
}

struct InstanceLease {
    file: File,
}

impl Drop for InstanceLease {
    fn drop(&mut self) {
        // A concurrent subprocess spawn can briefly duplicate this descriptor
        // before exec closes it. Unlock the shared open-file description now;
        // waiting for the final inherited descriptor to close delays release.
        let _ = self.file.unlock();
    }
}

struct Services {
    _instance: InstanceLease,
    data: PathBuf,
    cache: PathBuf,
    resources: PathBuf,
    home: PathBuf,
    reviews: reviews::Store,
    session: Control,
    admission: Admission,
    catalog_admission: Admission,
    download_admission: Admission,
    download_claim: Mutex<Option<String>>,
    youtube_music: Option<YouTubeMusicRuntime>,
    exported: Mutex<HashMap<PathBuf, PublishedExport>>,
}

/// Cloneable native service handle. Long operations return supervised jobs;
/// persistence methods are suitable for a UI's blocking I/O executor.
#[derive(Clone)]
pub struct Desktop(Arc<Services>);

impl std::fmt::Debug for Desktop {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Desktop").finish_non_exhaustive()
    }
}

fn operation_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!(
        "job-{timestamp:013}-{:020}",
        NEXT_OPERATION.fetch_add(1, Ordering::Relaxed)
    )
}

fn overridden_directory(key: &str, fallback: PathBuf) -> Result<PathBuf, String> {
    let path = std::env::var_os(key).map(PathBuf::from).unwrap_or(fallback);
    if !path.is_absolute() {
        return Err(format!("{key} must be an absolute directory."));
    }
    Ok(path)
}

pub(crate) fn resource_directory() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let fallback = packaged_resources(&executable, std::env::consts::OS)?;
    overridden_directory("JACKET_RESOURCE_DIR", fallback)
}

fn packaged_resources(executable: &Path, platform: &str) -> Result<PathBuf, String> {
    let directory = executable
        .parent()
        .ok_or("The application directory is unavailable.")?;
    Ok(directory.join(if platform == "macos" {
        "../Resources"
    } else {
        "../resources"
    }))
}

/// The operating system releases this advisory lock after a crash as well as
/// an ordinary shutdown. Keep the open file in Services for every clone.
fn acquire_instance(root: &Path) -> Result<InstanceLease, String> {
    reviews::private_directory(root)
        .map_err(|_| "Jacket's private data directory is unavailable.")?;
    let path = root.join("instance.lock");
    if path
        .symlink_metadata()
        .is_ok_and(|metadata| !metadata.is_file() || metadata.file_type().is_symlink())
    {
        return Err("Jacket's instance lock is unavailable.".into());
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options
        .open(path)
        .map_err(|_| "Jacket's instance lock is unavailable.")?;
    if !file.metadata().is_ok_and(|metadata| metadata.is_file()) {
        return Err("Jacket's instance lock is unavailable.".into());
    }
    file.try_lock().map_err(|error| match error {
        std::fs::TryLockError::WouldBlock => {
            "Jacket is already running. Return to its open window.".to_string()
        }
        std::fs::TryLockError::Error(_) => {
            "Jacket could not protect its saved work. The instance lock is unavailable.".to_string()
        }
    })?;
    Ok(InstanceLease { file })
}

impl Desktop {
    pub fn new() -> Result<Self, String> {
        let base = BaseDirs::new().ok_or("Your application data directory is unavailable.")?;
        Self::with_directories(
            overridden_directory("JACKET_DATA_DIR", base.data_dir().join(IDENTIFIER))?,
            overridden_directory("JACKET_CACHE_DIR", base.cache_dir().join(IDENTIFIER))?,
            resource_directory()?,
            base.home_dir().to_owned(),
        )
    }

    /// Explicit locations support portable qualification without touching a
    /// user's ordinary saved review, downloader settings, or browser profiles.
    pub fn with_directories(
        data: PathBuf,
        cache: PathBuf,
        resources: PathBuf,
        home: PathBuf,
    ) -> Result<Self, String> {
        if [&data, &cache, &resources, &home]
            .iter()
            .any(|path| !path.is_absolute())
        {
            return Err("Desktop data directories must be absolute.".into());
        }
        let instance = acquire_instance(&data)?;
        // A crashed development session can leave a plaintext handoff behind.
        // Cleanup owns the instance lock; a failure must not block public use.
        let _ = auth_session::startup(&data.join("auth-prototype"));
        let youtube_music = YouTubeMusicRuntime::discover(
            &resources,
            std::env::var_os("JACKET_YOUTUBE_MUSIC_PYTHON").map(PathBuf::from),
        );
        Ok(Self(Arc::new(Services {
            _instance: instance,
            data,
            cache,
            resources,
            home,
            reviews: reviews::Store::default(),
            session: Control::default(),
            admission: Arc::new(Mutex::new(None)),
            catalog_admission: Arc::new(Mutex::new(None)),
            download_admission: Arc::new(Mutex::new(None)),
            download_claim: Mutex::new(None),
            youtube_music,
            exported: Mutex::new(HashMap::new()),
        })))
    }

    pub fn data_directory(&self) -> &Path {
        &self.0.data
    }
    pub fn data_root(&self) -> &Path {
        &self.0.data
    }
    pub fn downloader_preferences(&self) -> Result<Preferences, String> {
        downloader_updates::preferences(&self.updates_root())
    }
    pub fn cache_directory(&self) -> &Path {
        &self.0.cache
    }
    pub fn resources_directory(&self) -> &Path {
        &self.0.resources
    }
    fn review_root(&self) -> PathBuf {
        self.0.data.join("review-library/draft")
    }
    fn artwork_root(&self) -> PathBuf {
        self.0.data.join("review-library/artwork")
    }
    pub(crate) fn jobs_root(&self) -> PathBuf {
        self.0.cache.join("jobs")
    }
    fn updates_root(&self) -> PathBuf {
        self.0.data.join("downloader")
    }
    fn session_root(&self) -> PathBuf {
        self.0.data.join("auth-prototype")
    }
    fn browser_locations(&self) -> Locations {
        Locations::current(self.0.home.clone())
    }
    fn base_tools(&self) -> Result<Toolchain, String> {
        Toolchain::discover_with_resources(&self.0.resources).map_err(|error| error.to_string())
    }
    fn tools(&self) -> Result<Toolchain, String> {
        let mut tools = self.base_tools()?;
        tools.yt_dlp = downloader_updates::selected_path(&self.updates_root(), &tools.yt_dlp)?;
        Ok(tools)
    }
    fn resolve_auth(&self, auth: &AuthOptions) -> Result<AuthOptions, String> {
        // Runtime paths are resolved here even for native callers. A saved or
        // imported record must never inject an arbitrary cookie-file path.
        let mut safe = auth.clone();
        safe.cookie_file = None;
        browser_sessions::resolve(&self.browser_locations(), &safe)
    }

    pub fn load_review(&self) -> Result<Option<ReviewDraft>, String> {
        let _lease = Lease::acquire(&self.0.admission)?;
        export_completion::reconcile(&self.0.reviews, &self.review_root(), &self.jobs_root())?;
        self.0.reviews.load(&self.review_root())
    }
    pub fn read_review(&self) -> Result<Option<ReviewDraft>, String> {
        self.0.reviews.load(&self.review_root())
    }

    pub fn retire_queued_review(&self, identity: &CompletionIdentity) -> Result<bool, String> {
        self.0
            .reviews
            .complete_export(&self.review_root(), identity)
    }

    pub(crate) fn download_claim(&self) -> &Mutex<Option<String>> {
        &self.0.download_claim
    }
    pub fn save_review(&self, draft: ReviewDraft) -> Result<bool, String> {
        self.0.reviews.save(&self.review_root(), draft)
    }
    pub fn discard_review(&self, session_id: &str, revision: u64) -> Result<bool, String> {
        self.0
            .reviews
            .discard(&self.review_root(), session_id, revision)
    }
    pub fn pending_export(&self) -> Option<Recovery> {
        recovery::latest(&self.jobs_root())
    }
    pub fn dismiss_recovery(&self, id: &str) -> Result<(), String> {
        let _lease = Lease::acquire(&self.0.admission)?;
        recovery::dismiss(&self.jobs_root(), id).map_err(|error| error.to_string())
    }
    pub fn prune_artwork(&self) -> Result<usize, String> {
        let _lease = Lease::acquire(&self.0.admission)?;
        let mut retained =
            recovery::artwork_asset_ids(&self.jobs_root()).map_err(|error| error.to_string())?;
        retained.extend(
            recovery::artwork_asset_ids(&self.downloads_root())
                .map_err(|error| error.to_string())?,
        );
        if let Some(review) = self.0.reviews.load(&self.review_root())? {
            if let Some(id) = review.preferences.shared_artwork_asset {
                retained.insert(id);
            }
        }
        artwork_assets::prune(&self.artwork_root(), &retained)
    }
    pub fn search_catalog(&self, request: SearchRequest) -> Result<Job, String> {
        self.youtube_music_job(move |runtime, cancel| {
            youtube_music::search(runtime, &request.query, request.filter, cancel).map(|hits| {
                crate::CatalogEvent::Results {
                    filter: request.filter,
                    hits,
                }
            })
        })
    }

    pub fn open_youtube_music_artist(&self, id: String) -> Result<Job, String> {
        self.youtube_music_job(move |runtime, cancel| {
            youtube_music::artist(runtime, &id, false, cancel).map(crate::CatalogEvent::Artist)
        })
    }

    pub fn youtube_music_artist_releases(&self, id: String) -> Result<Job, String> {
        self.youtube_music_job(move |runtime, cancel| {
            youtube_music::artist(runtime, &id, true, cancel)
                .map(|page| crate::CatalogEvent::ArtistReleases(page.hits))
        })
    }

    pub fn open_youtube_music_album(&self, id: String) -> Result<Job, String> {
        self.youtube_music_job(move |runtime, cancel| {
            youtube_music::album(runtime, &id, cancel)
                .map(|album| crate::CatalogEvent::Album(Box::new(album)))
        })
    }

    pub fn youtube_music_editions(&self, id: String) -> Result<Job, String> {
        self.youtube_music_job(move |runtime, cancel| {
            youtube_music::editions(runtime, &id, cancel).map(crate::CatalogEvent::Editions)
        })
    }

    pub fn open_youtube_music_song_album(&self, song: youtube_music::Song) -> Result<Job, String> {
        self.youtube_music_job(move |runtime, cancel| {
            youtube_music::song_album(runtime, song, cancel)
                .map(|album| crate::CatalogEvent::Album(Box::new(album)))
        })
    }

    pub fn open_youtube_music_song(&self, song: youtube_music::Song) -> Result<Job, String> {
        self.youtube_music_job(move |runtime, cancel| {
            youtube_music::song(runtime, song, cancel)
                .map(|song| crate::CatalogEvent::Song(Box::new(song)))
        })
    }

    pub fn open_youtube_music_search(&self, query: &str) -> Result<(), String> {
        let mut url = reqwest::Url::parse("https://music.youtube.com/search")
            .expect("constant YouTube Music search URL");
        url.query_pairs_mut().append_pair("q", query);
        match open_command(Path::new(url.as_str())) {
            Ok(status) if status.success() => Ok(()),
            _ => Err("Could not open YouTube Music in your browser.".into()),
        }
    }

    fn youtube_music_job(
        &self,
        run: impl FnOnce(&YouTubeMusicRuntime, &Cancellation) -> Result<crate::CatalogEvent, String>
            + Send
            + 'static,
    ) -> Result<Job, String> {
        let runtime = self
            .0
            .youtube_music
            .clone()
            .ok_or("YouTube Music is unavailable in this build of Jacket.")?;
        self.catalog_job(move |cancel| run(&runtime, cancel))
    }

    fn catalog_job(
        &self,
        run: impl FnOnce(&Cancellation) -> Result<crate::CatalogEvent, String> + Send + 'static,
    ) -> Result<Job, String> {
        Job::spawn(
            operation_id(),
            &self.0.catalog_admission,
            move |cancel, _| run(cancel).map(Event::Catalog),
        )
    }

    pub fn inspect(&self, url: String, auth: AuthOptions) -> Result<Job, String> {
        self.inspection_job(auth, move |tools, auth, cancel, events| {
            audio_engine::inspect_streaming(tools, &url, auth, cancel, |update| {
                if events.send(Event::Track(Box::new(update))).is_err() {
                    cancel.cancel();
                }
            })
            .map(Event::Inspected)
            .map_err(|error| error.to_string())
        })
    }

    fn inspection_job(
        &self,
        auth: AuthOptions,
        inspect: impl FnOnce(
                &Toolchain,
                &AuthOptions,
                &Cancellation,
                &std::sync::mpsc::SyncSender<Event>,
            ) -> Result<Event, String>
            + Send
            + 'static,
    ) -> Result<Job, String> {
        let desktop = self.clone();
        Job::spawn(operation_id(), &self.0.admission, move |cancel, events| {
            let _guard = auth
                .managed_session
                .is_some()
                .then(|| desktop.0.session.serial.lock())
                .transpose()
                .map_err(|_| "Session state is unavailable.")?;
            cancel.check().map_err(|error| error.to_string())?;
            let auth = desktop.resolve_auth(&auth)?;
            let handoff = auth_session::materialize(&desktop.session_root(), &Keychain, &auth)?;
            let runtime_auth = handoff.as_ref().map_or(&auth, |handoff| &handoff.auth);
            let result = inspect(&desktop.tools()?, runtime_auth, cancel, events)?;
            if let Some(handoff) = handoff {
                handoff.persist(&desktop.session_root(), &Keychain)?;
            }
            Ok(result)
        })
    }

    pub fn export(
        &self,
        plan: ExportPlan,
        destination: PathBuf,
        identity: Option<CompletionIdentity>,
    ) -> Result<Job, String> {
        self.start_export(plan, &destination, None, identity)
    }

    pub fn export_organized(
        &self,
        plan: ExportPlan,
        destination: &audio_engine::OrganizedOutput,
        identity: Option<CompletionIdentity>,
    ) -> Result<Job, String> {
        self.start_export(plan, destination.path(), Some(destination), identity)
    }

    fn start_export(
        &self,
        plan: ExportPlan,
        destination: &Path,
        organized: Option<&audio_engine::OrganizedOutput>,
        identity: Option<CompletionIdentity>,
    ) -> Result<Job, String> {
        let record = Self::export_recipe(plan, destination, organized, identity)?;
        self.export_record(record, false, None)
    }

    pub fn resume(&self, record: Recovery) -> Result<Job, String> {
        if record.progress.stage == "complete" {
            return Err("This export is already complete. Open its saved files instead.".into());
        }
        if record.plan.auth.managed_session.is_some() {
            return Err("Reconnect your session and start a new export.".into());
        }
        self.export_record(record, true, None)
    }

    pub(crate) fn export_recipe(
        plan: ExportPlan,
        destination: &Path,
        organized: Option<&audio_engine::OrganizedOutput>,
        identity: Option<CompletionIdentity>,
    ) -> Result<Recovery, String> {
        if plan.tracks.is_empty() {
            return Err("Select at least one track.".into());
        }
        let destination = destination
            .to_str()
            .ok_or("The destination needs a Unicode path.")?;
        let mut record = Recovery::new(operation_id(), plan, destination.to_owned());
        if let Some(organized) = organized {
            record.music_root = Some(organized.root().to_owned());
            record.music_album_stems = organized.album_stems().map(<[_]>::to_vec);
        }
        record.review_identity = identity;
        Self::export_paths(&record)?;
        Ok(record)
    }

    fn export_paths(
        record: &Recovery,
    ) -> Result<(PathBuf, Option<audio_engine::OrganizedOutput>), String> {
        if !recovery::valid_job_id(&record.job_id) {
            return Err("Invalid download identifier.".into());
        }
        if let Some(identity) = &record.review_identity {
            identity.validate()?;
        }
        let output = PathBuf::from(&record.output_path);
        let organized = if let Some(root) = &record.music_root {
            let destination = match (&record.music_album_stems, record.plan.output_kind) {
                (Some(stems), OutputKind::Folder)
                    if record.plan.mode == audio_engine::CollectionMode::Album
                        && record.plan.tracks.len() == stems.len() =>
                {
                    audio_engine::OrganizedOutput::album(
                        root.clone(),
                        output.clone(),
                        stems.clone(),
                    )
                }
                (None, OutputKind::File) if record.plan.tracks.len() == 1 => {
                    audio_engine::OrganizedOutput::new(root.clone(), output.clone())
                }
                _ => return Err("The saved music folder does not match this export.".into()),
            };
            Some(destination.map_err(|error| error.to_string())?)
        } else {
            validate_destination(&record.plan, &output)?;
            None
        };
        Ok((output, organized))
    }

    pub(crate) fn export_record(
        &self,
        mut record: Recovery,
        resume: bool,
        download: Option<downloads::Claim>,
    ) -> Result<Job, String> {
        let queued = download.is_some();
        // Recheck paths immediately before a worker starts, even for a queued recipe.
        let paths = if queued {
            Self::export_paths(&record)
        } else {
            Ok(Self::export_paths(&record)?)
        };
        let desktop = self.clone();
        Job::spawn(
            record.job_id.clone(),
            if queued {
                &self.0.download_admission
            } else {
                &self.0.admission
            },
            move |cancel, events| {
                let _claim = download;
                let work_root = if queued {
                    desktop.download_cache_root()
                } else {
                    desktop.jobs_root()
                };
                let workdir = if queued {
                    recovery::private_directory(&work_root)
                        .and_then(|()| recovery::job_directory(&work_root, &record.job_id))
                } else {
                    recovery::job_directory(&work_root, &record.job_id)
                };
                let record_dir = if queued {
                    recovery::job_directory(&desktop.downloads_root(), &record.job_id)
                } else {
                    workdir
                        .as_ref()
                        .map(Clone::clone)
                        .map_err(|error| std::io::Error::other(error.to_string()))
                }
                .map_err(|error| error.to_string())?;
                if queued {
                    record.progress.stage = "starting".into();
                    record.error = None;
                    record
                        .save(&record_dir)
                        .map_err(|error| error.to_string())?;
                }
                let outcome = (|| -> Result<ExportResult, String> {
                    let (output, organized) = paths?;
                    let workdir = workdir.as_ref().map_err(|error| error.to_string())?;
                    let _guard = (!queued || record.plan.auth.managed_session.is_some())
                        .then(|| desktop.0.session.serial.lock())
                        .transpose()
                        .map_err(|_| "Session state is unavailable.")?;
                    let mut plan = record.plan.clone();
                    plan.auth = desktop.resolve_auth(&plan.auth)?;
                    plan.shared_artwork_path = None;
                    if matches!(plan.artwork_mode, audio_engine::ArtworkMode::Shared) {
                        if let Some(id) = &plan.shared_artwork_asset {
                            plan.shared_artwork_path =
                                Some(artwork_assets::resolve(&desktop.artwork_root(), id)?);
                        }
                    }
                    let tools = desktop.tools()?;
                    if !queued {
                        record
                            .save(&record_dir)
                            .map_err(|error| error.to_string())?;
                    } else {
                        recovery::private_directory(workdir).map_err(|error| error.to_string())?;
                    }
                    let result = (|| {
                        cancel.check()?;
                        let handoff = auth_session::materialize(
                            &desktop.session_root(),
                            &Keychain,
                            &plan.auth,
                        )
                        .map_err(audio_engine::EngineError::Invalid)?;
                        if let Some(handoff) = &handoff {
                            plan.auth = handoff.auth.clone();
                        }
                        let emit = |progress: audio_engine::ProgressEvent| {
                            record.progress = progress.clone();
                            let _ = record.save_checkpoint(&record_dir);
                            // Progress is replaceable; a slow UI must never block file publication.
                            let _ = events.try_send(Event::Progress(progress));
                        };
                        let result = match &organized {
                            Some(organized) if organized.album_stems().is_some() => {
                                let save_album = if resume {
                                    audio_engine::resume_organized_album
                                } else {
                                    audio_engine::export_organized_album
                                };
                                save_album(&tools, &plan, organized, workdir, cancel, emit)
                            }
                            Some(organized) => audio_engine::export_organized_file(
                                &tools, &plan, organized, workdir, cancel, emit,
                            ),
                            None => audio_engine::export_collection(
                                &tools, &plan, &output, workdir, cancel, emit,
                            ),
                        };
                        if result.is_ok() {
                            if let Some(handoff) = handoff {
                                let _ = handoff.persist(&desktop.session_root(), &Keychain);
                            }
                        }
                        result
                    })()
                    .map_err(|error: audio_engine::EngineError| error.to_string());
                    result
                })();
                match outcome {
                    Ok(mut result) => {
                        let workdir = workdir.expect("publication requires its work directory");
                        if queued {
                            let can_reveal =
                                downloads::finish(&record_dir, &workdir, &mut record, &mut result);
                            return Ok(Event::DownloadExported { result, can_reveal });
                        }
                        export_completion::finish(
                            &desktop.0.reviews,
                            &desktop.review_root(),
                            &workdir,
                            &mut record,
                            &mut result,
                        );
                        if desktop.remember_export(&result).is_err() {
                            result.warnings.push("Your files were saved, but Jacket could not verify their location for opening. Open the destination from your file manager.".into());
                        }
                        Ok(Event::Exported(result))
                    }
                    Err(message) => {
                        record.progress.stage = if cancel.check().is_err() {
                            "cancelled"
                        } else {
                            "failed"
                        }
                        .into();
                        record.error = Some(message.clone());
                        let checkpoint = if queued || record_dir.is_dir() {
                            record.save_checkpoint(&record_dir)
                        } else {
                            Ok(())
                        };
                        Err(if checkpoint.is_err() {
                            format!("{message} The recovery checkpoint could not be saved; downloaded media remains in the local cache.")
                        } else {
                            message
                        })
                    }
                }
            },
        )
    }

    pub fn browser_profiles(&self, browser: Browser) -> Catalog {
        browser_sessions::discover(&self.browser_locations(), browser)
    }
    pub fn test_access(&self, url: String, auth: AuthOptions) -> Result<Job, String> {
        let desktop = self.clone();
        Job::spawn(operation_id(), &self.0.admission, move |cancel, _| {
            let auth = desktop.resolve_auth(&auth)?;
            browser_sessions::test_access(&desktop.tools()?, &auth, &url, cancel)
                .map(Event::AccessChecked)
        })
    }

    pub fn artwork_preview(&self, url: &str) -> Result<Vec<u8>, String> {
        crate::artwork_preview::fetch(url)
    }

    pub fn artwork_thumbnail(&self, url: &str) -> Result<Vec<u8>, String> {
        crate::artwork_preview::thumbnail(url)
    }

    /// Own a bounded public source cover before allowing a crop to modify it.
    /// Call this on the desktop blocking executor, like file artwork imports.
    pub fn import_remote_artwork(&self, url: &str) -> Result<Asset, String> {
        let _lease = Lease::acquire(&self.0.admission)?;
        let bytes = crate::artwork_preview::fetch(url)?;
        artwork_assets::import_bytes(&self.artwork_root(), &self.base_tools()?, &bytes)
    }

    pub fn import_artwork(&self, path: &Path, crop: Option<Crop>) -> Result<Asset, String> {
        let _lease = Lease::acquire(&self.0.admission)?;
        artwork_assets::import(&self.artwork_root(), &self.base_tools()?, path, crop)
    }
    pub fn crop_artwork(&self, id: &str, crop: Crop) -> Result<Asset, String> {
        let _lease = Lease::acquire(&self.0.admission)?;
        artwork_assets::crop(&self.artwork_root(), &self.base_tools()?, id, crop)
    }
    pub fn artwork_asset(&self, id: &str) -> Result<Asset, String> {
        artwork_assets::load(&self.artwork_root(), id)
    }
    pub fn artwork_path(&self, id: &str) -> Result<PathBuf, String> {
        artwork_assets::resolve(&self.artwork_root(), id)
    }

    pub fn check_downloader(&self, force: bool, channel: Option<Channel>) -> Result<Job, String> {
        let desktop = self.clone();
        Job::spawn(operation_id(), &self.0.admission, move |cancel, _| {
            cancel.check().map_err(|error| error.to_string())?;
            let status = downloader_updates::check(
                &desktop.updates_root(),
                &desktop.base_tools()?,
                force,
                channel,
            )?;
            cancel.check().map_err(|error| error.to_string())?;
            Ok(Event::Downloader(status))
        })
    }
    pub fn configure_downloader(&self, preferences: Preferences) -> Result<Job, String> {
        let desktop = self.clone();
        Job::spawn(operation_id(), &self.0.admission, move |cancel, _| {
            cancel.check().map_err(|error| error.to_string())?;
            downloader_updates::configure(
                &desktop.updates_root(),
                &desktop.base_tools()?,
                preferences,
                |apply| apply(),
            )
            .map(Event::Downloader)
        })
    }
    pub fn update_downloader(&self, channel: Channel) -> Result<Job, String> {
        let desktop = self.clone();
        Job::spawn(operation_id(), &self.0.admission, move |cancel, events| {
            downloader_updates::install(
                &desktop.updates_root(),
                &desktop.base_tools()?,
                channel,
                cancel,
                |activate| {
                    activate()?;
                    Ok(true)
                },
                |phase| {
                    let _ = events.try_send(Event::Progress(audio_engine::ProgressEvent {
                        stage: phase.into(),
                        message: format!("Audio downloader: {phase}"),
                        completed: 0,
                        total: 0,
                    }));
                },
            )
            .map(Event::Downloader)
        })
    }
    pub fn restore_downloader(&self) -> Result<Job, String> {
        let desktop = self.clone();
        Job::spawn(operation_id(), &self.0.admission, move |cancel, _| {
            cancel.check().map_err(|error| error.to_string())?;
            downloader_updates::use_bundled(&desktop.updates_root())?;
            downloader_updates::check(&desktop.updates_root(), &desktop.base_tools()?, false, None)
                .map(Event::Downloader)
        })
    }

    pub fn session_status(&self) -> Result<auth_session::Status, String> {
        auth_session::status(&self.session_root())
    }
    pub fn session_action(&self, action: SessionAction, url: String) -> Result<Job, String> {
        let desktop = self.clone();
        Job::spawn(operation_id(), &self.0.admission, move |cancel, _| {
            let _guard = desktop
                .0
                .session
                .serial
                .lock()
                .map_err(|_| "Session state is unavailable.")?;
            let root = desktop.session_root();
            let status = match action {
                SessionAction::Connect => auth_session::begin(&root, &desktop.0.session, &Keychain),
                SessionAction::Save => auth_session::finish(&root, &desktop.0.session, cancel),
                SessionAction::Verify => {
                    auth_session::verify(&root, &Keychain, &desktop.tools()?, &url, cancel)
                }
                SessionAction::Disconnect => {
                    let status = auth_session::disconnect(&root, &desktop.0.session, &Keychain);
                    recovery::clear_managed(&desktop.jobs_root())
                        .map_err(|error| error.to_string())?;
                    status
                }
            }?;
            Ok(Event::Session(status))
        })
    }

    /// Only paths actually published by this process may be opened as exports.
    pub fn reveal_export(&self, result: &ExportResult) -> Result<(), String> {
        let path = Path::new(&result.output_path);
        let kind = self.authorize_export(path)?;
        self.reveal_published(path, kind)
    }

    pub(crate) fn reveal_published(&self, path: &Path, kind: OutputKind) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        let status = if kind == OutputKind::Folder {
            Command::new("open").arg(path).status()
        } else {
            Command::new("open").arg("-R").arg(path).status()
        };
        #[cfg(target_os = "windows")]
        let status = if kind == OutputKind::Folder {
            Command::new("explorer").arg(path).status()
        } else {
            Command::new("explorer")
                .arg(format!("/select,{}", path.display()))
                .status()
        };
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let status = open_command(if kind == OutputKind::Folder {
            path
        } else {
            path.parent().ok_or("The export folder is unavailable.")?
        });
        if !status.map_err(|error| error.to_string())?.success() {
            return Err("Could not open the export folder.".into());
        }
        Ok(())
    }
    pub fn open_playlist(&self, result: &ExportResult) -> Result<(), String> {
        let root = Path::new(&result.output_path);
        let kind = self.authorize_export(root)?;
        if kind != OutputKind::Folder {
            return Err("Extract the ZIP before opening its playlist.".into());
        }
        let playlist = root.join("playlist.m3u");
        if !playlist
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
        {
            return Err("The exported playlist is no longer available.".into());
        }
        if !open_command(&playlist)
            .map_err(|error| error.to_string())?
            .success()
        {
            return Err("Choose an audio player that supports M3U playlists.".into());
        }
        Ok(())
    }
    pub(crate) fn remember_export(&self, result: &ExportResult) -> Result<(), String> {
        let path = Path::new(&result.output_path);
        let published = PublishedExport::read(path, result.output_kind)?;
        self.0
            .exported
            .lock()
            .map_err(|_| "The export list is unavailable.")?
            .insert(path.to_owned(), published);
        Ok(())
    }

    fn authorize_export(&self, path: &Path) -> Result<OutputKind, String> {
        let exported = self
            .0
            .exported
            .lock()
            .map_err(|_| "The export list is unavailable.")?;
        let published = exported
            .get(path)
            .ok_or("This export is no longer available at the saved location.")?;
        let current = PublishedExport::read(path, published.kind)?;
        if current != *published {
            return Err("This export is no longer available at the saved location.".into());
        }
        Ok(published.kind)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum SessionAction {
    Connect,
    Save,
    Verify,
    Disconnect,
}

fn open_command(path: &Path) -> std::io::Result<std::process::ExitStatus> {
    #[cfg(target_os = "macos")]
    let executable = "open";
    #[cfg(target_os = "windows")]
    let executable = "explorer";
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let executable = "xdg-open";
    Command::new(executable)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
}

fn validate_destination(plan: &ExportPlan, output: &Path) -> Result<(), String> {
    if !output.is_absolute()
        || output.file_name().is_none()
        || !output.parent().is_some_and(Path::is_dir)
    {
        return Err("Choose an available destination using the save dialog.".into());
    }
    if plan.output_kind == OutputKind::Zip
        && !output
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
    {
        return Err("Choose a ZIP filename for the archive.".into());
    }
    if plan.output_kind == OutputKind::File && plan.tracks.len() != 1 {
        return Err("Choose exactly one track to save an audio file.".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Desktop;
    use crate::worker::Event;
    use audio_engine::{AuthOptions, ExportPlan, ExportResult, OutputKind};
    use std::path::{Path, PathBuf};
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

    #[test]
    fn catalog_admission_is_available_while_a_download_owns_its_worker() {
        let root = tempfile::tempdir().unwrap();
        let desktop = desktop(root.path());
        let _download = super::Lease::acquire(&desktop.0.download_admission).unwrap();
        let mut job = desktop
            .catalog_job(|_| {
                Ok(crate::CatalogEvent::Results {
                    filter: None,
                    hits: Vec::new(),
                })
            })
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if let Some(event) = job.poll() {
                assert!(matches!(
                    event,
                    Event::Catalog(crate::CatalogEvent::Results { .. })
                ));
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
    }

    #[test]
    fn startup_does_not_require_media_tools_or_create_review_data() {
        let root = tempfile::tempdir().unwrap();
        let desktop = desktop(root.path());
        assert!(desktop.load_review().unwrap().is_none());
        assert!(desktop.pending_export().is_none());
        assert!(root.path().join("data/instance.lock").is_file());
        assert!(!root.path().join("data/review-library").exists());
        assert!(!root.path().join("cache").exists());
    }

    #[test]
    fn imported_results_cannot_open_arbitrary_local_files() {
        let root = tempfile::tempdir().unwrap();
        let desktop = desktop(root.path());
        let path = root.path().join("unrelated");
        std::fs::write(&path, "fixture").unwrap();
        let result = ExportResult {
            output_path: path.to_string_lossy().into_owned(),
            output_kind: OutputKind::File,
            playlist_path: None,
            track_count: 1,
            album_save: None,
            warnings: Vec::new(),
        };
        assert!(desktop.reveal_export(&result).is_err());
        assert!(desktop.open_playlist(&result).is_err());
    }

    #[test]
    fn stale_browser_profiles_fail_without_falling_back_to_another_account() {
        let root = tempfile::tempdir().unwrap();
        let desktop = desktop(root.path());
        let mut job = desktop
            .inspect(
                "https://www.youtube.com/watch?v=fixture".into(),
                AuthOptions {
                    browser: Some(audio_engine::Browser::Firefox),
                    browser_profile: Some("missing-profile".into()),
                    ..AuthOptions::default()
                },
            )
            .unwrap();
        let start = Instant::now();
        loop {
            if let Some(event) = job.poll() {
                match event {
                    Event::Failed(error) => assert!(error.contains("no longer available")),
                    _ => panic!("a stale profile must not start inspection"),
                }
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(5));
            std::thread::yield_now();
        }
    }

    #[test]
    fn export_admission_rejects_invalid_destinations_before_work_starts() {
        let root = tempfile::tempdir().unwrap();
        let desktop = desktop(root.path());
        let plan: ExportPlan = serde_json::from_value(serde_json::json!({
            "title": "Album", "sourceUrl": "https://www.youtube.com/watch?v=fixture",
            "mode": "album", "format": "mp3", "artworkMode": "none",
            "sharedArtworkUrl": null, "auth": {}, "tracks": []
        }))
        .unwrap();
        assert!(desktop
            .export(plan.clone(), PathBuf::from("relative.zip"), None)
            .is_err());
        assert!(desktop
            .export(plan, root.path().join("wrong.mp3"), None)
            .is_err());
        assert!(desktop.pending_export().is_none());
    }

    #[test]
    fn archive_destination_accepts_the_pickers_case_preserving_zip_suffix() {
        let root = tempfile::tempdir().expect("destination fixture");
        let plan: ExportPlan = serde_json::from_value(serde_json::json!({
            "title": "Album", "sourceUrl": "https://www.youtube.com/watch?v=fixture",
            "mode": "album", "format": "mp3", "artworkMode": "none",
            "sharedArtworkUrl": null, "auth": {}, "tracks": []
        }))
        .expect("archive plan");
        for extension in ["zip", "ZIP", "ZiP"] {
            let destination = root.path().join(format!("Album.{extension}"));
            assert!(super::validate_destination(&plan, &destination).is_ok());
            assert!(!destination.exists());
        }
        for name in ["Album", "Album.zipx", "Album.mp3"] {
            assert!(super::validate_destination(&plan, &root.path().join(name)).is_err());
        }
    }

    #[test]
    fn data_roots_must_be_absolute() {
        assert!(Desktop::with_directories(
            "relative".into(),
            "/cache".into(),
            "/resources".into(),
            "/home".into()
        )
        .is_err());
    }
    #[test]
    fn only_one_instance_can_mutate_a_data_directory_and_drop_releases_it() {
        let root = tempfile::tempdir().unwrap();
        let first = desktop(root.path());
        let clone = first.clone();
        let create = || {
            Desktop::with_directories(
                root.path().join("data"),
                root.path().join("cache"),
                root.path().join("resources"),
                root.path().join("home"),
            )
        };
        assert!(create().unwrap_err().contains("already running"));
        drop(first);
        assert!(
            create().is_err(),
            "a remaining desktop clone must retain the lock"
        );
        drop(clone);
        assert!(
            create().is_ok(),
            "dropping the final clone must release the OS lock"
        );
    }

    #[cfg(unix)]
    #[test]
    fn instance_locks_never_follow_symlinks() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("data")).unwrap();
        let unrelated = root.path().join("unrelated");
        std::fs::write(&unrelated, "keep").unwrap();
        symlink(&unrelated, root.path().join("data/instance.lock")).unwrap();
        assert!(Desktop::with_directories(
            root.path().join("data"),
            root.path().join("cache"),
            root.path().join("resources"),
            root.path().join("home")
        )
        .is_err());
        assert_eq!(std::fs::read_to_string(unrelated).unwrap(), "keep");
    }
    #[test]
    fn subprocess_instance_holder() {
        let Some(root) = std::env::var_os("JACKET_INSTANCE_TEST_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let _desktop = desktop(&root);
        std::fs::write(root.join("ready"), "ready").unwrap();
        let mut buffer = [0];
        let _ = std::io::Read::read(&mut std::io::stdin(), &mut buffer);
    }

    #[test]
    fn a_second_process_is_excluded_and_a_crash_releases_the_lock() {
        use std::process::{Command, Stdio};
        let root = tempfile::tempdir().unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "desktop::tests::subprocess_instance_holder",
                "--nocapture",
            ])
            .env("JACKET_INSTANCE_TEST_ROOT", root.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let start = Instant::now();
        while !root.path().join("ready").exists() {
            assert!(
                child.try_wait().unwrap().is_none(),
                "lock holder exited before acquiring its lock"
            );
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "lock holder did not start"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        let create = || {
            Desktop::with_directories(
                root.path().join("data"),
                root.path().join("cache"),
                root.path().join("resources"),
                root.path().join("home"),
            )
        };
        assert!(create().unwrap_err().contains("already running"));
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(
            create().is_ok(),
            "the OS must release an interrupted process's lock"
        );
    }
    fn export_result(path: &Path, kind: OutputKind) -> ExportResult {
        ExportResult {
            output_path: path.to_string_lossy().into_owned(),
            output_kind: kind,
            playlist_path: None,
            track_count: 1,
            album_save: None,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn synchronous_asset_and_recovery_mutations_hold_job_admission() {
        let root = tempfile::tempdir().unwrap();
        let desktop = desktop(root.path());
        let lease = crate::worker::Lease::acquire(&desktop.0.admission).unwrap();
        let busy = "Wait for the current operation to finish.";
        assert_eq!(
            desktop
                .import_artwork(&root.path().join("missing.png"), None)
                .unwrap_err(),
            busy
        );
        assert_eq!(
            desktop
                .crop_artwork(
                    "missing",
                    crate::artwork_assets::Crop {
                        x: 0,
                        y: 0,
                        size: 1
                    }
                )
                .unwrap_err(),
            busy
        );
        assert_eq!(
            desktop
                .import_remote_artwork("https://i.ytimg.com/cover.jpg")
                .unwrap_err(),
            busy
        );
        assert_eq!(desktop.prune_artwork().unwrap_err(), busy);
        assert_eq!(desktop.dismiss_recovery("missing").unwrap_err(), busy);
        assert_eq!(desktop.load_review().unwrap_err(), busy);
        assert_eq!(
            desktop
                .inspect(
                    "https://www.youtube.com/watch?v=fixture".into(),
                    AuthOptions::default()
                )
                .unwrap_err(),
            busy
        );
        drop(lease);
        assert!(desktop.prune_artwork().is_ok());
        assert!(desktop.load_review().unwrap().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn resource_resolution_loads_bundled_tools_from_both_native_package_layouts() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        for (platform, executable, resources) in [
            (
                "macos",
                "Jacket.app/Contents/MacOS/jacket",
                "Jacket.app/Contents/Resources",
            ),
            ("linux", "jacket-linux/bin/jacket", "jacket-linux/resources"),
        ] {
            let root = tempfile::tempdir().unwrap();
            let package = root.path().join("package");
            let binaries = package.join(resources).join("audio-tools/bin");
            std::fs::create_dir_all(package.join(executable).parent().unwrap()).unwrap();
            std::fs::create_dir_all(&binaries).unwrap();
            for name in ["yt-dlp", "ffmpeg", "ffprobe", "node"] {
                let tool = binaries.join(name);
                std::fs::write(&tool, "#!/bin/sh\nexit 0\n").unwrap();
                std::fs::set_permissions(tool, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
            // macOS temp directories can spell /private/var as /var. Exercise
            // an equivalent parent alias on every Unix host as well.
            let alias = root.path().join("package-alias");
            symlink(&package, &alias).unwrap();
            let expected = binaries.canonicalize().unwrap();
            for package_path in [&package, &alias] {
                let discovered =
                    super::packaged_resources(&package_path.join(executable), platform).unwrap();
                let tools = audio_engine::Toolchain::discover_with_resources(&discovered).unwrap();
                for (tool, name) in [
                    (&tools.yt_dlp, "yt-dlp"),
                    (&tools.ffmpeg, "ffmpeg"),
                    (&tools.ffprobe, "ffprobe"),
                    (tools.node.as_ref().expect("bundled Node"), "node"),
                ] {
                    assert_eq!(tool.canonicalize().unwrap(), expected.join(name));
                }
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn saved_output_paths_cannot_be_redirected_through_symlinks() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let desktop = desktop(root.path());
        let first = root.path().join("first");
        let second = root.path().join("second");
        std::fs::create_dir_all(first.join("Album")).unwrap();
        std::fs::create_dir_all(second.join("Album")).unwrap();
        let alias = root.path().join("Music");
        symlink(&first, &alias).unwrap();
        let path = alias.join("Album");
        desktop
            .remember_export(&export_result(&path, OutputKind::Folder))
            .unwrap();
        assert_eq!(desktop.authorize_export(&path).unwrap(), OutputKind::Folder);
        std::fs::remove_file(&alias).unwrap();
        symlink(&second, &alias).unwrap();
        assert!(desktop.authorize_export(&path).is_err());
        std::fs::remove_file(&alias).unwrap();
        symlink(&first, &alias).unwrap();
        std::fs::remove_dir(first.join("Album")).unwrap();
        symlink(second.join("Album"), first.join("Album")).unwrap();
        assert!(desktop.authorize_export(&path).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn opening_a_playlist_never_follows_a_replaced_playlist_symlink() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let desktop = desktop(root.path());
        let album = root.path().join("Album");
        std::fs::create_dir(&album).unwrap();
        let unrelated = root.path().join("unrelated");
        std::fs::write(&unrelated, "private").unwrap();
        let result = export_result(&album, OutputKind::Folder);
        desktop.remember_export(&result).unwrap();
        symlink(&unrelated, album.join("playlist.m3u")).unwrap();
        assert!(desktop.open_playlist(&result).is_err());
    }
    #[test]
    fn opening_a_saved_output_rejects_a_different_item_reusing_its_name() {
        let root = tempfile::tempdir().unwrap();
        let desktop = desktop(root.path());
        let original = root.path().join("Album");
        let retained = root.path().join("Moved Album");
        std::fs::create_dir(&original).unwrap();
        desktop
            .remember_export(&export_result(&original, OutputKind::Folder))
            .unwrap();
        std::fs::rename(&original, retained).unwrap();
        std::fs::create_dir(&original).unwrap();
        assert!(desktop.authorize_export(&original).is_err());
    }
    #[test]
    fn dropping_an_instance_unlocks_even_while_an_inherited_descriptor_is_open() {
        let root = tempfile::tempdir().unwrap();
        let first = desktop(root.path());
        // try_clone reproduces the shared open-file description temporarily
        // inherited by a concurrent subprocess between fork and exec.
        let inherited = first.0._instance.file.try_clone().unwrap();
        drop(first);
        let reopened = Desktop::with_directories(
            root.path().join("data"),
            root.path().join("cache"),
            root.path().join("resources"),
            root.path().join("home"),
        );
        assert!(
            reopened.is_ok(),
            "instance teardown must explicitly unlock before closing its descriptor"
        );
        drop(inherited);
    }
}
