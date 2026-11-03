use crate::artwork_assets::{self, Asset, Crop};
use crate::auth_session::{self, Control, Keychain};
use crate::browser_sessions::{self, Catalog, Locations};
use crate::downloader_updates::{self, Channel, Preferences};
use crate::export_completion;
use crate::recovery::{self, Recovery};
use crate::reviews::{self, CompletionIdentity, ReviewDraft};
use crate::worker::{Admission, Event, Job, Lease};
use audio_engine::{AuthOptions, Browser, ExportPlan, ExportResult, OutputKind, Toolchain};
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

#[derive(PartialEq, Eq)]
struct PublishedExport {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    created: Option<SystemTime>,
    canonical_path: PathBuf,
    kind: OutputKind,
}

impl PublishedExport {
    fn read(path: &Path, kind: OutputKind) -> Result<Self, String> {
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
        "job-{timestamp}-{}",
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
        Ok(Self(Arc::new(Services {
            _instance: instance,
            data,
            cache,
            resources,
            home,
            reviews: reviews::Store::default(),
            session: Control::default(),
            admission: Arc::new(Mutex::new(None)),
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
    fn jobs_root(&self) -> PathBuf {
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
        if let Some(review) = self.0.reviews.load(&self.review_root())? {
            if let Some(id) = review.preferences.shared_artwork_asset {
                retained.insert(id);
            }
        }
        artwork_assets::prune(&self.artwork_root(), &retained)
    }
    pub fn inspect(&self, url: String, auth: AuthOptions) -> Result<Job, String> {
        let desktop = self.clone();
        Job::spawn(operation_id(), &self.0.admission, move |cancel, events| {
            let _guard = desktop
                .0
                .session
                .serial
                .lock()
                .map_err(|_| "Session state is unavailable.")?;
            cancel.check().map_err(|error| error.to_string())?;
            let auth = desktop.resolve_auth(&auth)?;
            let handoff = auth_session::materialize(&desktop.session_root(), &Keychain, &auth)?;
            let runtime_auth = handoff.as_ref().map_or(&auth, |handoff| &handoff.auth);
            let inspection = audio_engine::inspect_streaming(
                &desktop.tools()?,
                &url,
                runtime_auth,
                cancel,
                |update| {
                    if events.send(Event::Track(Box::new(update))).is_err() {
                        cancel.cancel();
                    }
                },
            )
            .map_err(|error| error.to_string())?;
            if let Some(handoff) = handoff {
                handoff.persist(&desktop.session_root(), &Keychain)?;
            }
            Ok(Event::Inspected(inspection))
        })
    }

    pub fn export(
        &self,
        plan: ExportPlan,
        destination: PathBuf,
        identity: Option<CompletionIdentity>,
    ) -> Result<Job, String> {
        let destination = destination
            .to_str()
            .ok_or("The destination needs a Unicode path.")?
            .to_owned();
        let mut record = Recovery::new(operation_id(), plan, destination);
        record.review_identity = identity;
        self.export_record(record)
    }

    pub fn resume(&self, record: Recovery) -> Result<Job, String> {
        if record.progress.stage == "complete" {
            return Err("This export is already complete. Open its saved files instead.".into());
        }
        if record.plan.auth.managed_session.is_some() {
            return Err("Reconnect your session and start a new export.".into());
        }
        self.export_record(record)
    }

    fn export_record(&self, mut record: Recovery) -> Result<Job, String> {
        if !recovery::valid_job_id(&record.job_id) {
            return Err("Invalid download identifier.".into());
        }
        if let Some(identity) = &record.review_identity {
            identity.validate()?;
        }
        validate_destination(&record.plan, Path::new(&record.output_path))?;
        let desktop = self.clone();
        Job::spawn(
            record.job_id.clone(),
            &self.0.admission,
            move |cancel, events| {
                let _guard = desktop
                    .0
                    .session
                    .serial
                    .lock()
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
                let workdir = recovery::job_directory(&desktop.jobs_root(), &record.job_id)
                    .map_err(|error| error.to_string())?;
                record.save(&workdir).map_err(|error| error.to_string())?;
                let output = PathBuf::from(&record.output_path);
                let outcome = (|| {
                    cancel.check()?;
                    let handoff =
                        auth_session::materialize(&desktop.session_root(), &Keychain, &plan.auth)
                            .map_err(audio_engine::EngineError::Invalid)?;
                    if let Some(handoff) = &handoff {
                        plan.auth = handoff.auth.clone();
                    }
                    let result = audio_engine::export_collection(
                        &tools,
                        &plan,
                        &output,
                        &workdir,
                        cancel,
                        |progress| {
                            record.progress = progress.clone();
                            let _ = record.save(&workdir);
                            // Progress is replaceable; a slow UI must never block file publication.
                            let _ = events.try_send(Event::Progress(progress));
                        },
                    );
                    if result.is_ok() {
                        if let Some(handoff) = handoff {
                            let _ = handoff.persist(&desktop.session_root(), &Keychain);
                        }
                    }
                    result
                })();
                match outcome {
                    Ok(mut result) => {
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
                    Err(error) => {
                        record.progress.stage =
                            if matches!(error, audio_engine::EngineError::Cancelled) {
                                "cancelled"
                            } else {
                                "failed"
                            }
                            .into();
                        let message = error.to_string();
                        record.error = Some(message.clone());
                        let checkpoint = record.save(&workdir);
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
        self.authorize_export(path)?;
        #[cfg(target_os = "macos")]
        let status = if path.is_dir() {
            Command::new("open").arg(path).status()
        } else {
            Command::new("open").arg("-R").arg(path).status()
        };
        #[cfg(target_os = "windows")]
        let status = if path.is_dir() {
            Command::new("explorer").arg(path).status()
        } else {
            Command::new("explorer")
                .arg(format!("/select,{}", path.display()))
                .status()
        };
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let status = open_command(if path.is_dir() {
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
    fn remember_export(&self, result: &ExportResult) -> Result<(), String> {
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
        && output.extension().and_then(|extension| extension.to_str()) != Some("zip")
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
        use std::os::unix::fs::PermissionsExt;
        for (platform, executable, resources) in [
            (
                "macos",
                "Jacket.app/Contents/MacOS/jacket",
                "Jacket.app/Contents/Resources",
            ),
            ("linux", "jacket-linux/bin/jacket", "jacket-linux/resources"),
        ] {
            let root = tempfile::tempdir().unwrap();
            let executable = root.path().join(executable);
            let resources = root.path().join(resources);
            std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
            let binaries = resources.join("audio-tools/bin");
            std::fs::create_dir_all(&binaries).unwrap();
            for name in ["yt-dlp", "ffmpeg", "ffprobe", "node"] {
                let tool = binaries.join(name);
                std::fs::write(&tool, "#!/bin/sh\nexit 0\n").unwrap();
                std::fs::set_permissions(tool, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
            let discovered = super::packaged_resources(&executable, platform).unwrap();
            let tools = audio_engine::Toolchain::discover_with_resources(&discovered).unwrap();
            assert_eq!(
                tools.yt_dlp.canonicalize().unwrap(),
                binaries.join("yt-dlp")
            );
            assert_eq!(
                tools.ffmpeg.canonicalize().unwrap(),
                binaries.join("ffmpeg")
            );
            assert_eq!(
                tools.ffprobe.canonicalize().unwrap(),
                binaries.join("ffprobe")
            );
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
