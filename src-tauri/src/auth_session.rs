//! Development-only, local YouTube session. No secrets cross the webview boundary.
use audio_engine::{AuthOptions, Cancellation, Toolchain};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Child, Command, Stdio},
    sync::Mutex,
    time::Duration,
};

type Result<T> = std::result::Result<T, String>;
#[cfg(target_os = "macos")]
const SERVICE: &str = "com.moonlark.audio-dl.auth-prototype";
#[cfg(target_os = "macos")]
const ACCOUNT: &str = "youtube-session-v1";
const CHROME: &str = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

#[derive(Default)]
pub struct Control {
    pub serial: Mutex<()>,
    browser: Mutex<Option<Child>>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub supported: bool,
    pub session_id: Option<String>,
    pub verified: bool,
    pub signing_in: bool,
    pub message: String,
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    session_id: Option<String>,
    verified: bool,
    signing_in: bool,
}

#[derive(Deserialize, Serialize)]
struct Secret {
    id: String,
    cookies: String,
}

pub trait Store {
    fn get(&self) -> Result<Option<Vec<u8>>>;
    fn set(&self, bytes: &[u8]) -> Result<()>;
    fn delete(&self) -> Result<()>;
}

pub struct Keychain;

#[cfg(target_os = "macos")]
fn password_options() -> security_framework::passwords::PasswordOptions {
    let mut options =
        security_framework::passwords::PasswordOptions::new_generic_password(SERVICE, ACCOUNT);
    options.set_access_synchronized(Some(false));
    options
}

impl Store for Keychain {
    fn get(&self) -> Result<Option<Vec<u8>>> {
        #[cfg(target_os = "macos")]
        {
            match security_framework::passwords::generic_password(password_options()) {
                Ok(bytes) => Ok(Some(bytes)),
                Err(error) if error.code() == -25300 => Ok(None),
                Err(_) => Err(
                    "Unlock or allow access to Jacket's local session in Keychain, then try again."
                        .into(),
                ),
            }
        }
        #[cfg(not(target_os = "macos"))]
        Err("The session prototype requires macOS.".into())
    }
    fn set(&self, bytes: &[u8]) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            security_framework::passwords::set_generic_password_options(bytes, password_options())
                .map_err(|_| "Could not save Jacket's local session in Keychain.".into())
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = bytes;
            Err("The session prototype requires macOS.".into())
        }
    }
    fn delete(&self) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            match security_framework::passwords::delete_generic_password_options(password_options()) {
                Ok(()) => Ok(()),
                Err(error) if error.code() == -25300 => Ok(()),
                Err(_) => Err("Session disabled locally. Allow Keychain access and press Disconnect again to finish removing its stored credentials.".into()),
            }
        }
        #[cfg(not(target_os = "macos"))]
        Err("The session prototype requires macOS.".into())
    }
}

fn io_error(_: std::io::Error) -> String {
    "Could not access Jacket's private session files.".into()
}

fn no_symlink_chain(path: &Path) -> Result<()> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(info) if info.file_type().is_symlink() => {
                return Err("Session path contains a symlink; refusing access.".into())
            }
            Ok(_) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(io_error(error)),
        }
    }
    Ok(())
}

fn private_directory(path: &Path) -> Result<()> {
    no_symlink_chain(path)?;
    if !path.exists() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(path)
                .map_err(io_error)?;
        }
        #[cfg(not(unix))]
        fs::create_dir_all(path).map_err(io_error)?;
    }
    let info = fs::symlink_metadata(path).map_err(io_error)?;
    if !info.is_dir() {
        return Err("Invalid private session directory.".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if info.permissions().mode() & 0o077 != 0 || info.uid() != unsafe { libc::getuid() } {
            return Err("Session directory must belong to you with owner-only permissions.".into());
        }
    }
    Ok(())
}

fn record(root: &Path) -> Result<Record> {
    private_directory(root)?;
    let path = root.join("state.json");
    no_symlink_chain(&path)?;
    match fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|_| "Invalid session record. Disconnect to reset it.".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Record::default()),
        Err(error) => Err(io_error(error)),
    }
}

fn save_record(root: &Path, value: &Record) -> Result<()> {
    private_directory(root)?;
    no_symlink_chain(&root.join("state.json"))?;
    let mut file = tempfile::NamedTempFile::new_in(root).map_err(io_error)?;
    serde_json::to_writer(&mut file, value)
        .map_err(|_| "Could not write session state.".to_owned())?;
    file.as_file().sync_all().map_err(io_error)?;
    file.persist(root.join("state.json"))
        .map_err(|_| "Could not save session state.".to_owned())?;
    Ok(())
}

pub fn status(root: &Path) -> Result<Status> {
    if !cfg!(all(debug_assertions, target_os = "macos")) {
        return Ok(Status::default());
    }
    let value = record(root)?;
    Ok(Status {
        supported: true,
        message: if value.signing_in {
            "Sign in in the separate Chrome window, then close it and save the session."
        } else if value.verified {
            "Account-only access verified. The session stays on this Mac."
        } else if value.session_id.is_some() {
            "Session saved. Verify it with an account-only link."
        } else {
            "Optional development prototype. Public downloads need no connection."
        }
        .into(),
        session_id: value.session_id,
        verified: value.verified,
        signing_in: value.signing_in,
    })
}

pub fn enabled() -> Result<()> {
    if !cfg!(all(debug_assertions, target_os = "macos")) {
        return Err(
            "The owned-session prototype is available in macOS development builds only.".into(),
        );
    }
    Ok(())
}

fn clean_directory(root: &Path, name: &str) -> Result<()> {
    let path = root.join(name);
    no_symlink_chain(&path)?;
    if path.exists() {
        fs::remove_dir_all(path).map_err(io_error)?;
    }
    Ok(())
}

/// On startup no worker can own a jar. Clean only our plaintext handoff area.
pub fn startup(root: &Path) -> Result<()> {
    if !cfg!(all(debug_assertions, target_os = "macos")) {
        return Ok(());
    }
    private_directory(root)?;
    clean_directory(root, "handoffs")
}

// A PID is never sufficient authority to signal a process after a restart.
// Match its entire expected launch command before closing this exact browser.
fn browser_command(root: &Path) -> String {
    format!("{CHROME} --user-data-dir={} --no-first-run --no-default-browser-check https://www.youtube.com/", root.join("chrome-profile").display())
}

fn browser_pid(root: &Path) -> Result<Option<u32>> {
    let path = root.join("browser.pid");
    no_symlink_chain(&path)?;
    let Ok(value) = fs::read_to_string(&path) else {
        return Ok(None);
    };
    let pid: u32 = value
        .trim()
        .parse()
        .map_err(|_| "Invalid owned-browser record.".to_owned())?;
    let output = Command::new("/bin/ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()
        .map_err(io_error)?;
    if !output.status.success() {
        return Ok(None);
    }
    if String::from_utf8_lossy(&output.stdout).trim() != browser_command(root) {
        return Err("The owned browser process could not be identified safely. Close its separate window before retrying; personal browser windows will not be closed.".into());
    }
    Ok(Some(pid))
}

fn close_browser(root: &Path, control: &Control) -> Result<()> {
    if let Some(child) = control
        .browser
        .lock()
        .map_err(|_| "Browser state unavailable")?
        .as_mut()
    {
        let _ = child.try_wait();
    }
    if let Some(pid) = browser_pid(root)? {
        #[cfg(unix)]
        if unsafe { libc::kill(pid as i32, libc::SIGTERM) } != 0 {
            return Err("Close Jacket's separate Chrome window, then try again.".into());
        }
        for _ in 0..100 {
            if let Some(child) = control
                .browser
                .lock()
                .map_err(|_| "Browser state unavailable")?
                .as_mut()
            {
                let _ = child.try_wait();
            }
            if browser_pid(root)?.is_none() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        if browser_pid(root)?.is_some() {
            return Err("Close Jacket's separate Chrome window, then try again.".into());
        }
    }
    Ok(())
}

pub fn begin(root: &Path, control: &Control, store: &impl Store) -> Result<Status> {
    enabled()?;
    private_directory(root)?;
    close_browser(root, control)?;
    // Invalidate the old generation before deleting secrets or opening a browser.
    save_record(root, &Record::default())?;
    store.delete()?;
    clean_directory(root, "chrome-profile")?;
    clean_directory(root, "handoffs")?;
    if !Path::new(CHROME).is_file() {
        return Err("Install Google Chrome to try the owned-session prototype.".into());
    }
    private_directory(&root.join("chrome-profile"))?;
    let child = Command::new(CHROME)
        .arg(format!(
            "--user-data-dir={}",
            root.join("chrome-profile").display()
        ))
        .args([
            "--no-first-run",
            "--no-default-browser-check",
            "https://www.youtube.com/",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(io_error)?;
    no_symlink_chain(&root.join("browser.pid"))?;
    fs::write(root.join("browser.pid"), child.id().to_string()).map_err(io_error)?;
    *control
        .browser
        .lock()
        .map_err(|_| "Browser state unavailable")? = Some(child);
    save_record(
        root,
        &Record {
            signing_in: true,
            ..Record::default()
        },
    )?;
    status(root)
}

fn valid_jar(jar: &str) -> bool {
    if jar.len() > 1024 * 1024 || !jar.starts_with("# Netscape HTTP Cookie File\n") {
        return false;
    }
    let mut count = 0;
    for line in jar.lines() {
        let line = line.strip_prefix("#HttpOnly_").unwrap_or(line);
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 7
            || line
                .chars()
                .any(|c| (c.is_control() && c != '\t') || c == '\u{7f}')
        {
            return false;
        }
        let domain = fields[0]
            .strip_prefix('.')
            .unwrap_or(fields[0])
            .to_ascii_lowercase();
        if !(domain == "youtube.com" || domain.ends_with(".youtube.com"))
            || domain.split('.').any(|part| {
                part.is_empty()
                    || part.starts_with('-')
                    || part.ends_with('-')
                    || !part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            })
            || !["TRUE", "FALSE"].contains(&fields[1])
            || !["TRUE", "FALSE"].contains(&fields[3])
            || !fields[2].starts_with('/')
            || fields[4].parse::<u64>().is_err()
            || fields[5].is_empty()
        {
            return false;
        }
        count += 1;
    }
    count > 0
}

pub fn accept(root: &Path, store: &impl Store, jar: String) -> Result<Status> {
    if !valid_jar(&jar) {
        return Err(
            "The isolated session has invalid or unrelated cookies. Reconnect YouTube.".into(),
        );
    }
    let nonce = tempfile::Builder::new()
        .prefix("session-")
        .tempdir_in(root)
        .map_err(io_error)?;
    let id = nonce
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let bytes = serde_json::to_vec(&Secret {
        id: id.clone(),
        cookies: jar,
    })
    .map_err(|_| "Could not prepare session")?;
    store.set(&bytes)?;
    // Do not report success while the full sign-in profile remains on disk.
    let result = (|| {
        clean_directory(root, "chrome-profile")?;
        save_record(
            root,
            &Record {
                session_id: Some(id),
                ..Record::default()
            },
        )
    })();
    if let Err(error) = result {
        let _ = store.delete();
        return Err(error);
    }
    status(root)
}

pub fn finish(root: &Path, control: &Control, cancel: &Cancellation) -> Result<Status> {
    enabled()?;
    if !record(root)?.signing_in {
        return Err("Choose Connect YouTube first.".into());
    }
    close_browser(root, control)?;
    let profile = root.join("chrome-profile").join("Default");
    no_symlink_chain(&profile)?;
    if !profile.is_dir() {
        return Err("Sign in in Jacket's separate Chrome window first.".into());
    }
    // Deliberately use the bundled Python/yt-dlp pair for this internal API.
    // Updated standalone yt-dlp executables need not expose Python modules.
    let runtime = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/audio-tools");
    let python_home = runtime.join("python");
    let library = python_home.join("lib/python3.14/site-packages");
    if !library.join("yt_dlp").is_dir() {
        return Err("Stage the bundled audio runtime before trying this prototype.".into());
    }
    let handoffs = root.join("handoffs");
    private_directory(&handoffs)?;
    let temporary = tempfile::tempdir_in(&handoffs).map_err(io_error)?;
    let mut command = Command::new(runtime.join("bin/python"));
    command
        .args(["-B", "-S", "-c", include_str!("session_capture.py")])
        .arg(library)
        .arg(profile)
        .env("PYTHONHOME", python_home)
        .env("PYTHONNOUSERSITE", "1")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("TMPDIR", temporary.path())
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONSTARTUP");
    let bytes = audio_engine::process::run(&mut command, cancel, Duration::from_secs(120))
        .map_err(|_| "Could not read the isolated YouTube session. Sign in again, or allow the Chrome Keychain request.".to_owned())?;
    let jar = String::from_utf8(bytes).map_err(|_| "Invalid isolated session")?;
    accept(root, &Keychain, jar)
}

/// The directory and 0600 file outlive yt-dlp and are dropped on every exit path.
pub struct Handoff {
    _directory: tempfile::TempDir,
    pub auth: AuthOptions,
}

impl Handoff {
    pub fn persist(&self, root: &Path, store: &impl Store) -> Result<()> {
        let id = self
            .auth
            .managed_session
            .as_ref()
            .ok_or("Session unavailable")?;
        if record(root)?.session_id.as_ref() != Some(id) {
            return Err("Session changed. Reconnect YouTube.".into());
        }
        let jar = fs::read_to_string(
            self.auth
                .cookie_file
                .as_ref()
                .ok_or("Session unavailable")?,
        )
        .map_err(io_error)?;
        if !valid_jar(&jar) {
            return Err("The downloader returned invalid session data. Reconnect YouTube.".into());
        }
        store.set(
            &serde_json::to_vec(&Secret {
                id: id.clone(),
                cookies: jar,
            })
            .map_err(|_| "Could not save session")?,
        )
    }
}

pub fn materialize(
    root: &Path,
    store: &impl Store,
    requested: &AuthOptions,
) -> Result<Option<Handoff>> {
    let Some(id) = &requested.managed_session else {
        return Ok(None);
    };
    enabled()?;
    if requested.browser.is_some() {
        return Err("Choose one session source.".into());
    }
    if record(root)?.session_id.as_ref() != Some(id) {
        return Err("Reconnect the Jacket YouTube session before continuing.".into());
    }
    let bytes = store
        .get()?
        .ok_or("Session missing from Keychain. Reconnect YouTube.")?;
    let secret: Secret =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid stored session. Reconnect YouTube.")?;
    if secret.id != *id || !valid_jar(&secret.cookies) {
        return Err("Session changed. Reconnect YouTube.".into());
    }
    let directory = root.join("handoffs");
    private_directory(&directory)?;
    let directory = tempfile::tempdir_in(directory).map_err(io_error)?;
    let mut file = tempfile::NamedTempFile::new_in(directory.path()).map_err(io_error)?;
    file.write_all(secret.cookies.as_bytes())
        .map_err(io_error)?;
    let file = file
        .into_temp_path()
        .keep()
        .map_err(|_| "Could not prepare private cookie file")?;
    let mut auth = requested.clone();
    auth.cookie_file = Some(file);
    Ok(Some(Handoff {
        _directory: directory,
        auth,
    }))
}

pub fn disconnect(root: &Path, control: &Control, store: &impl Store) -> Result<Status> {
    enabled()?;
    // Fail closed even if browser shutdown, file cleanup, or Keychain deletion fails.
    save_record(root, &Record::default())?;
    let browser = close_browser(root, control);
    let credentials = store.delete();
    let handoffs = clean_directory(root, "handoffs");
    browser?;
    let profile = clean_directory(root, "chrome-profile");
    credentials?;
    handoffs?;
    profile?;
    let _ = fs::remove_file(root.join("browser.pid"));
    status(root)
}

fn probe(
    tools: &Toolchain,
    url: &str,
    auth: &AuthOptions,
    cancel: &Cancellation,
) -> Result<Vec<u8>> {
    let mut command =
        audio_engine::source::downloader(tools, auth).map_err(|error| error.to_string())?;
    command.args([
        "--skip-download",
        "--dump-single-json",
        "--flat-playlist",
        "--playlist-end",
        "1",
        "--",
        url,
    ]);
    audio_engine::process::run(&mut command, cancel, Duration::from_secs(90))
        .map_err(|error| error.to_string())
}

pub fn verify(
    root: &Path,
    store: &impl Store,
    tools: &Toolchain,
    url: &str,
    cancel: &Cancellation,
) -> Result<Status> {
    enabled()?;
    let url = audio_engine::source::validate_source_url(url).map_err(|error| error.to_string())?;
    let mut value = record(root)?;
    value.verified = false;
    save_record(root, &value)?;
    let auth = AuthOptions {
        managed_session: value.session_id.clone(),
        ..AuthOptions::default()
    };
    if auth.managed_session.is_none() {
        return Err("Connect YouTube first.".into());
    }
    let handoff = materialize(root, store, &auth)?.ok_or("Session unavailable")?;
    // Prove this is account-only access, not merely a successful public request.
    match probe(tools, &url, &AuthOptions::default(), cancel) {
        Ok(_) => return Err("This link works without signing in. Choose a private playlist or another account-only item to verify the session.".into()),
        Err(error) => {
            let detail = error.to_ascii_lowercase();
            if !["private", "sign in", "sign-in", "login required", "age-restricted", "members-only"].iter().any(|pattern| detail.contains(pattern)) {
                return Err("Public access failed for an unrelated reason. Try verification again when the connection is working.".into());
            }
        }
    }
    let bytes = probe(tools, &url, &handoff.auth, cancel).map_err(|_| "Account-only access failed. Reconnect YouTube, or check that this account can open the link.".to_owned())?;
    let result: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| "Could not verify the response")?;
    if result.get("entries").is_some_and(|entries| {
        entries.as_array().is_none_or(|entries| {
            entries.is_empty() || entries.iter().all(serde_json::Value::is_null)
        })
    }) {
        return Err("Choose an account-only link containing an available item.".into());
    }
    value.verified = true;
    handoff.persist(root, store)?;
    save_record(root, &value)?;
    status(root)
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::{
        cell::RefCell,
        os::unix::fs::{symlink, PermissionsExt},
    };

    const JAR: &str = "# Netscape HTTP Cookie File\n#HttpOnly_.youtube.com\tTRUE\t/\tTRUE\t0\tFIXTURE_SESSION\tsynthetic-only\n";

    #[derive(Default)]
    struct MemoryStore {
        bytes: RefCell<Option<Vec<u8>>>,
        fail_delete: RefCell<bool>,
        fail_get: RefCell<bool>,
    }
    impl Store for MemoryStore {
        fn get(&self) -> Result<Option<Vec<u8>>> {
            if *self.fail_get.borrow() {
                Err("Keychain locked".into())
            } else {
                Ok(self.bytes.borrow().clone())
            }
        }
        fn set(&self, bytes: &[u8]) -> Result<()> {
            *self.bytes.borrow_mut() = Some(bytes.to_vec());
            Ok(())
        }
        fn delete(&self) -> Result<()> {
            if *self.fail_delete.borrow() {
                Err("Keychain denied".into())
            } else {
                self.bytes.borrow_mut().take();
                Ok(())
            }
        }
    }
    fn fixture() -> (tempfile::TempDir, std::path::PathBuf, MemoryStore) {
        let parent = tempfile::tempdir_in("/private/tmp").unwrap();
        let root = parent.path().join("owned");
        private_directory(&root).unwrap();
        (parent, root, MemoryStore::default())
    }
    fn request(id: Option<String>) -> AuthOptions {
        AuthOptions {
            managed_session: id,
            ..AuthOptions::default()
        }
    }
    fn handoffs_empty(root: &Path) -> bool {
        !root.join("handoffs").exists()
            || fs::read_dir(root.join("handoffs"))
                .unwrap()
                .next()
                .is_none()
    }

    #[test]
    fn session_survives_reconstruction_and_disconnect_removes_secrets_and_profile() {
        let (_parent, root, store) = fixture();
        private_directory(&root.join("chrome-profile")).unwrap();
        fs::write(
            root.join("chrome-profile/browser-state"),
            "synthetic-browser-state",
        )
        .unwrap();
        let saved = accept(&root, &store, JAR.into()).unwrap();
        let auth = request(saved.session_id.clone());
        assert!(!root.join("chrome-profile").exists());
        let metadata = fs::read_to_string(root.join("state.json")).unwrap();
        assert!(!metadata.contains("synthetic-only"));
        // A new control and freshly read status represent app process restart.
        let restarted = Control::default();
        assert_eq!(status(&root).unwrap().session_id, saved.session_id);
        let cookie_path;
        {
            let handoff = materialize(&root, &store, &auth).unwrap().unwrap();
            cookie_path = handoff.auth.cookie_file.clone().unwrap();
            assert_eq!(
                fs::metadata(&cookie_path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(fs::read_to_string(&cookie_path).unwrap(), JAR);
            let refreshed = JAR.replace("synthetic-only", "synthetic-refreshed");
            fs::write(&cookie_path, &refreshed).unwrap();
            handoff.persist(&root, &store).unwrap();
        }
        assert!(!cookie_path.exists());
        let again = materialize(&root, &store, &auth).unwrap().unwrap();
        assert!(fs::read_to_string(again.auth.cookie_file.as_ref().unwrap())
            .unwrap()
            .contains("synthetic-refreshed"));
        drop(again);
        disconnect(&root, &restarted, &store).unwrap();
        assert!(store.get().unwrap().is_none());
        assert!(status(&root).unwrap().session_id.is_none());
        assert!(handoffs_empty(&root));
        assert!(materialize(&root, &store, &auth).is_err());
        disconnect(&root, &restarted, &store).unwrap();
    }

    #[test]
    fn reconnect_invalidates_prior_generation_and_keychain_failure_fails_closed() {
        let (_parent, root, store) = fixture();
        let old = request(accept(&root, &store, JAR.into()).unwrap().session_id);
        let new = request(accept(&root, &store, JAR.into()).unwrap().session_id);
        assert_ne!(old.managed_session, new.managed_session);
        assert!(materialize(&root, &store, &old).is_err());
        *store.fail_get.borrow_mut() = true;
        assert!(materialize(&root, &store, &new).is_err());
        *store.fail_get.borrow_mut() = false;
        *store.fail_delete.borrow_mut() = true;
        assert!(disconnect(&root, &Control::default(), &store).is_err());
        assert!(store.bytes.borrow().is_some());
        assert!(materialize(&root, &store, &new).is_err());
        *store.fail_delete.borrow_mut() = false;
        disconnect(&root, &Control::default(), &store).unwrap();
    }

    #[test]
    fn startup_removes_orphaned_plaintext_without_erasing_the_saved_session() {
        let (_parent, root, store) = fixture();
        let auth = request(accept(&root, &store, JAR.into()).unwrap().session_id);
        let stale = materialize(&root, &store, &auth).unwrap().unwrap();
        let path = stale.auth.cookie_file.clone().unwrap();
        std::mem::forget(stale); // Simulated abrupt process exit bypassing Drop.
        startup(&root).unwrap();
        assert!(!path.exists());
        assert!(materialize(&root, &store, &auth).unwrap().is_some());
    }

    #[test]
    fn unsafe_paths_and_unrelated_cookies_are_rejected_without_touching_the_target() {
        let (parent, root, store) = fixture();
        assert!(!valid_jar(
            &JAR.replace(".youtube.com", ".youtube.com.attacker.test")
        ));
        assert!(!valid_jar(
            &JAR.replace("synthetic-only", "line\ninjection")
        ));
        assert!(!valid_jar(&JAR.replace(".youtube.com", "..youtube.com")));
        let other = parent.path().join("unrelated");
        fs::create_dir(&other).unwrap();
        fs::write(other.join("keep"), "untouched").unwrap();
        symlink(&other, root.join("handoffs")).unwrap();
        assert!(startup(&root).is_err());
        assert!(accept(&root, &store, JAR.replace(".youtube.com", ".google.com")).is_err());
        assert_eq!(fs::read_to_string(other.join("keep")).unwrap(), "untouched");
        fs::remove_file(root.join("handoffs")).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(status(&root).is_err());
    }

    fn tools(root: &Path, public: bool, expired: bool) -> Toolchain {
        let binary = root.join("fixture-downloader");
        let script = if public {
            "#!/bin/sh\nprintf '%s' '{\"id\":\"fixture\"}'\n".to_owned()
        } else {
            format!("#!/bin/sh\ncase \" $* \" in\n*' --cookies '*) {};;\n*) echo 'ERROR: Private video. Sign in' >&2; exit 1;;\nesac\n", if expired { "echo 'WARNING: YouTube account cookies are no longer valid' >&2; echo '{\"id\":\"fixture\"}'" } else { "echo '{\"id\":\"fixture\"}'" })
        };
        fs::write(&binary, script).unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        Toolchain {
            yt_dlp: binary,
            ffmpeg: "unused".into(),
            ffprobe: "unused".into(),
            node: None,
        }
    }

    #[test]
    fn verification_requires_account_only_access_and_cleans_up_on_failure_and_cancellation() {
        let (_parent, root, store) = fixture();
        accept(&root, &store, JAR.into()).unwrap();
        let url = "https://www.youtube.com/watch?v=fixture";
        let success = verify(
            &root,
            &store,
            &tools(&root, false, false),
            url,
            &Cancellation::default(),
        )
        .unwrap();
        assert!(success.verified);
        assert!(handoffs_empty(&root));
        assert!(verify(
            &root,
            &store,
            &tools(&root, true, false),
            url,
            &Cancellation::default()
        )
        .unwrap_err()
        .contains("without signing in"));
        assert!(!status(&root).unwrap().verified);
        assert!(verify(
            &root,
            &store,
            &tools(&root, false, true),
            url,
            &Cancellation::default()
        )
        .is_err());
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(verify(&root, &store, &tools(&root, false, false), url, &cancel).is_err());
        assert!(handoffs_empty(&root));
    }

    #[test]
    #[ignore = "Explicit local Keychain integration; writes and deletes only a synthetic test item"]
    fn real_keychain_roundtrip_uses_a_separate_synthetic_item() {
        use security_framework::passwords::*;
        let options = || {
            let mut value =
                PasswordOptions::new_generic_password(SERVICE, "synthetic-integration-test");
            value.set_access_synchronized(Some(false));
            value
        };
        let value = b"Jacket synthetic fixture; contains no browser cookies";
        set_generic_password_options(value, options()).unwrap();
        let read = generic_password(options());
        let deleted = delete_generic_password_options(options());
        assert_eq!(read.unwrap(), value);
        deleted.unwrap();
        assert_eq!(generic_password(options()).unwrap_err().code(), -25300);
    }
}
