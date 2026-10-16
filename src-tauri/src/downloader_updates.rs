use audio_engine::{process::run, Cancellation, Toolchain};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const MAX_BINARY: u64 = 128 * 1024 * 1024;
const DAY: u64 = 24 * 60 * 60;
const ASSET: &str = "yt-dlp_macos";

#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Stable,
    #[default]
    Nightly,
}
impl Channel {
    fn repository(self) -> &'static str {
        match self {
            Self::Stable => "yt-dlp/yt-dlp",
            Self::Nightly => "yt-dlp/yt-dlp-nightly-builds",
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Nightly => "nightly",
        }
    }
}

#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Managed,
    Local,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    pub mode: Mode,
    pub auto_update: bool,
    pub local_path: Option<String>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            mode: Mode::Managed,
            auto_update: true,
            local_path: None,
        }
    }
}

pub fn preferences(root: &Path) -> Result<Preferences, String> {
    let path = root.join("preferences.json");
    if !path.exists() {
        return Ok(Preferences::default());
    }
    load(&path).ok_or_else(|| "Couldn’t read yt-dlp settings. Choose your downloader again.".into())
}

// macOS file pickers can resolve a package-manager symlink into a pinned
// Cellar version. Keep the suggested link when both denote the selected file.
pub fn selection_path(path: String, preferred: Option<String>) -> String {
    if let Some(preferred) = preferred {
        let candidate = Path::new(&path);
        let alias = Path::new(&preferred);
        if candidate.is_absolute()
            && alias.is_absolute()
            && candidate.is_file()
            && alias.is_file()
            && candidate.canonicalize().ok() == alias.canonicalize().ok()
            && candidate.canonicalize().is_ok()
        {
            return preferred;
        }
    }
    path
}

fn local_binary(path: Option<&str>) -> Result<PathBuf, String> {
    let path = PathBuf::from(path.ok_or("Choose a local yt-dlp executable.")?);
    if !path.is_absolute() || !path.is_file() {
        return Err("Your local yt-dlp is unavailable. Choose another executable or switch to Automatic in yt-dlp settings.".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if path
            .metadata()
            .map_err(|error| error.to_string())?
            .permissions()
            .mode()
            & 0o111
            == 0
        {
            return Err("Choose an executable yt-dlp file.".into());
        }
    }
    Ok(path)
}

pub fn selected_path(root: &Path, base: &Path) -> Result<PathBuf, String> {
    let prefs = preferences(root)?;
    match prefs.mode {
        Mode::Managed => Ok(override_path(root).unwrap_or_else(|| base.to_owned())),
        Mode::Local => local_binary(prefs.local_path.as_deref()),
    }
}

fn youtube_support(binary: &Path, cancel: &Cancellation) -> Result<(), String> {
    let extractors = run(
        Command::new(binary).args(["--ignore-config", "--list-extractors"]),
        cancel,
        Duration::from_secs(20),
    )
    .map_err(|error| error.to_string())?;
    if !String::from_utf8_lossy(&extractors)
        .lines()
        .any(|line| line.trim().eq_ignore_ascii_case("youtube"))
    {
        return Err(
            "This executable is missing YouTube support. Your current downloader is unchanged."
                .into(),
        );
    }
    Ok(())
}

pub fn configure(
    root: &Path,
    base: &Toolchain,
    prefs: Preferences,
    apply_if_idle: impl FnOnce(&mut dyn FnMut() -> Result<(), String>) -> Result<(), String>,
) -> Result<Status, String> {
    if prefs.mode == Mode::Local {
        let path = local_binary(prefs.local_path.as_deref())?;
        version(&path, &Cancellation::default()).map_err(|_| {
            "This file couldn’t run as yt-dlp. Your current downloader is unchanged."
        })?;
        youtube_support(&path, &Cancellation::default())?;
        // Local executables may be Homebrew symlinks. Keep the selected path so
        // package-manager upgrades remain visible; never copy or mutate them.
    }
    apply_if_idle(&mut || save(&root.join("preferences.json"), &prefs))?;
    if prefs.mode == Mode::Managed && prefs.auto_update {
        check(root, base, false, None)
    } else {
        status(
            root,
            base,
            &load(&root.join("check.json")).unwrap_or_default(),
        )
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Release {
    channel: Channel,
    version: String,
    sha256: String,
    size: u64,
}
impl Release {
    fn url(&self, asset: &str) -> String {
        format!(
            "https://github.com/{}/releases/download/{}/{asset}",
            self.channel.repository(),
            self.version
        )
    }
    fn directory(&self) -> String {
        format!(
            "{}-{}-{}",
            self.channel.name(),
            self.version,
            &self.sha256[..12]
        )
    }
    fn valid(&self) -> bool {
        version_parts(&self.version).is_some()
            && valid_hash(&self.sha256)
            && self.size > 0
            && self.size <= MAX_BINARY
    }
}

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Check {
    channel: Channel,
    checked_at: u64,
    release: Option<Release>,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub installed_version: String,
    pub mode: Mode,
    pub auto_update: bool,
    pub local_path: Option<String>,
    pub detected_local_path: Option<String>,
    pub pending_activation: bool,
    pub latest_version: Option<String>,
    pub channel: Channel,
    pub checked_at: Option<u64>,
    pub update_available: bool,
    pub downgrade: bool,
    pub using_updated_copy: bool,
    pub error: Option<String>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn valid_hash(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn version_parts(value: &str) -> Option<Vec<u32>> {
    if value.len() > 32 {
        return None;
    }
    let parts = value
        .split('.')
        .map(|part| {
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            part.parse().ok()
        })
        .collect::<Option<Vec<u32>>>()?;
    if !(3..=4).contains(&parts.len())
        || !(2000..=2999).contains(&parts[0])
        || !(1..=12).contains(&parts[1])
        || !(1..=31).contains(&parts[2])
    {
        return None;
    }
    Some(parts)
}

fn load<T: for<'a> Deserialize<'a>>(path: &Path) -> Option<T> {
    if path.symlink_metadata().ok()?.file_type().is_symlink() {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() <= 64 * 1024)
        .then(|| serde_json::from_slice(&bytes).ok())
        .flatten()
}

fn private_directory(path: &Path) -> Result<(), String> {
    if path
        .symlink_metadata()
        .is_ok_and(|value| value.file_type().is_symlink())
    {
        return Err("The downloader update folder is unavailable.".into());
    }
    fs::create_dir_all(path).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn save<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path.parent().ok_or("The update folder is unavailable.")?;
    private_directory(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    file.write_all(&serde_json::to_vec(value).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    file.as_file_mut()
        .sync_all()
        .map_err(|error| error.to_string())?;
    file.persist(path).map_err(|error| error.to_string())?;
    Ok(())
}

fn hash(path: &Path) -> Option<String> {
    if path.symlink_metadata().ok()?.file_type().is_symlink() {
        return None;
    }
    let mut file = fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer).ok()?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Some(hex::encode(hasher.finalize()))
}

pub fn override_path(root: &Path) -> Option<PathBuf> {
    if [root.to_owned(), root.join("versions")].iter().any(|path| {
        path.symlink_metadata()
            .is_ok_and(|value| value.file_type().is_symlink())
    }) {
        return None;
    }
    let release: Release = load(&root.join("active.json"))?;
    if !release.valid() {
        return None;
    }
    let directory = root.join("versions").join(release.directory());
    if directory.symlink_metadata().ok()?.file_type().is_symlink() {
        return None;
    }
    let binary = directory.join(ASSET);
    if binary.metadata().ok()?.len() != release.size
        || hash(&binary)? != release.sha256.to_lowercase()
    {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if binary.metadata().ok()?.permissions().mode() & 0o111 == 0 {
            return None;
        }
    }
    Some(binary)
}

fn version(binary: &Path, cancel: &Cancellation) -> Result<String, String> {
    let bytes = run(
        Command::new(binary).args(["--ignore-config", "--version"]),
        cancel,
        Duration::from_secs(20),
    )
    .map_err(|error| error.to_string())?;
    let value = String::from_utf8_lossy(&bytes).trim().to_owned();
    if version_parts(&value).is_none() {
        return Err("The downloader returned an unsupported version.".into());
    }
    Ok(value)
}

fn allowed_url(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && matches!(
            url.host_str(),
            Some(
                "api.github.com"
                    | "github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
}

fn client() -> Result<Client, String> {
    Client::builder()
        .user_agent("Audio-DL-downloader-updater")
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(90))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || !allowed_url(attempt.url()) {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|error| error.to_string())
}

fn get_bytes(client: &Client, url: &str, limit: u64) -> Result<Vec<u8>, String> {
    let response = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(15))
        .send()
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    response
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("The update response was unexpectedly large.".into());
    }
    Ok(bytes)
}

fn release_from_json(value: &Value, channel: Channel) -> Result<Release, String> {
    let version = value
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or("GitHub did not return a release version.")?
        .to_owned();
    if version_parts(&version).is_none()
        || value.get("draft").and_then(Value::as_bool) == Some(true)
    {
        return Err("GitHub returned an invalid release.".into());
    }
    let asset = value
        .get("assets")
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .find(|item| item.get("name").and_then(Value::as_str) == Some(ASSET))
        })
        .ok_or("This release has no macOS downloader.")?;
    let mut release = Release {
        channel,
        version,
        sha256: String::new(),
        size: asset.get("size").and_then(Value::as_u64).unwrap_or(0),
    };
    if asset.get("browser_download_url").and_then(Value::as_str)
        != Some(release.url(ASSET).as_str())
        || release.size == 0
        || release.size > MAX_BINARY
    {
        return Err("GitHub returned an unexpected download location or size.".into());
    }
    if let Some(digest) = asset.get("digest").and_then(Value::as_str) {
        let digest = digest
            .strip_prefix("sha256:")
            .ok_or("The release checksum is unsupported.")?;
        if !valid_hash(digest) {
            return Err("The release checksum is invalid.".into());
        }
        release.sha256 = digest.to_lowercase();
    }
    Ok(release)
}

fn checksum_from_manifest(bytes: &[u8]) -> Option<String> {
    String::from_utf8_lossy(bytes).lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let digest = parts.next()?;
        let name = parts.next()?.trim_start_matches('*');
        (name == ASSET && valid_hash(digest) && parts.next().is_none())
            .then(|| digest.to_lowercase())
    })
}

fn fetch_release(channel: Channel) -> Result<Release, String> {
    let client = client()?;
    let bytes = get_bytes(
        &client,
        &format!(
            "https://api.github.com/repos/{}/releases/latest",
            channel.repository()
        ),
        2 * 1024 * 1024,
    )?;
    let mut release = release_from_json(
        &serde_json::from_slice(&bytes).map_err(|error| error.to_string())?,
        channel,
    )?;
    if release.sha256.is_empty() {
        release.sha256 = checksum_from_manifest(&get_bytes(
            &client,
            &release.url("SHA2-256SUMS"),
            1024 * 1024,
        )?)
        .ok_or("No verified checksum was found for the downloader.")?;
    }
    Ok(release)
}

fn status(root: &Path, base: &Toolchain, check: &Check) -> Result<Status, String> {
    let prefs = preferences(root)?;
    let active = override_path(root);
    let selected = selected_path(root, &base.yt_dlp);
    let installed = selected.and_then(|path| version(&path, &Cancellation::default()));
    let (installed_version, selected_error) = match installed {
        Ok(value) => (value, None),
        Err(error) => ("Unavailable".into(), Some(error)),
    };
    let latest = check
        .release
        .as_ref()
        .filter(|release| {
            prefs.mode == Mode::Managed && release.valid() && release.channel == check.channel
        })
        .map(|release| release.version.clone());
    let update_available = latest
        .as_ref()
        .is_some_and(|value| version_parts(value) != version_parts(&installed_version));
    let downgrade = latest
        .as_ref()
        .is_some_and(|value| version_parts(value) < version_parts(&installed_version));
    Ok(Status {
        installed_version,
        mode: prefs.mode,
        auto_update: prefs.auto_update,
        local_path: prefs.local_path,
        detected_local_path: audio_engine::process::find_tool("yt-dlp")
            .map(|path| path.to_string_lossy().into_owned()),
        pending_activation: prefs.mode == Mode::Managed
            && pending_release(root).is_some_and(|pending| {
                pending.channel == check.channel && latest.as_deref() == Some(&pending.version)
            }),
        latest_version: latest,
        channel: check.channel,
        checked_at: (check.checked_at > 0).then_some(check.checked_at),
        update_available,
        downgrade,
        using_updated_copy: prefs.mode == Mode::Managed && active.is_some(),
        error: selected_error
            .or_else(|| {
                if prefs.mode == Mode::Managed {
                    check.error.clone()
                } else {
                    None
                }
            })
            .or_else(|| {
                (prefs.mode == Mode::Managed
                    && root.join("active.json").exists()
                    && active.is_none())
                .then(|| {
                    "The updated copy could not be verified. Using the included downloader instead."
                        .into()
                })
            }),
    })
}

pub fn check(
    root: &Path,
    base: &Toolchain,
    force: bool,
    channel: Option<Channel>,
) -> Result<Status, String> {
    check_with_fetch(root, base, force, channel, fetch_release)
}

fn check_with_fetch(
    root: &Path,
    base: &Toolchain,
    force: bool,
    channel: Option<Channel>,
    fetch: impl FnOnce(Channel) -> Result<Release, String>,
) -> Result<Status, String> {
    let mut check: Check = load(&root.join("check.json")).unwrap_or_default();
    if preferences(root)?.mode == Mode::Local {
        return status(root, base, &check);
    }

    let requested = channel.unwrap_or(check.channel);
    if requested != check.channel {
        check = Check {
            channel: requested,
            ..Check::default()
        };
    }
    if force
        || check.error.is_some()
        || check.checked_at == 0
        || check.checked_at > now()
        || now().saturating_sub(check.checked_at) >= DAY
    {
        check.checked_at = now();
        match fetch(check.channel) {
            Ok(release) => {
                check.release = Some(release);
                check.error = None;
            }
            Err(_) => {
                check.error = Some(
                    "Couldn’t check GitHub. Downloads still work; try checking again later.".into(),
                );
            }
        }
        save(&root.join("check.json"), &check)?;
    }
    status(root, base, &check)
}

fn stage(
    root: &Path,
    release: &Release,
    file: tempfile::NamedTempFile,
    cancel: &Cancellation,
) -> Result<(), String> {
    if !release.valid()
        || file
            .as_file()
            .metadata()
            .map_err(|error| error.to_string())?
            .len()
            != release.size
        || hash(file.path()).as_deref() != Some(&release.sha256)
    {
        return Err(
            "The downloaded yt-dlp checksum did not match. Your current version is unchanged."
                .into(),
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
    }
    let actual = version(file.path(), cancel)?;
    if version_parts(&actual) != version_parts(&release.version) {
        return Err(
            "The download reported the wrong version. Your current version is unchanged.".into(),
        );
    }
    youtube_support(file.path(), cancel)?;
    cancel.check().map_err(|error| error.to_string())?;
    private_directory(&root.join("versions"))?;
    let directory = root.join("versions").join(release.directory());
    private_directory(&directory)?;
    let destination = directory.join(ASSET);
    match file.persist_noclobber(&destination) {
        Ok(_) => (),
        Err(error)
            if error.error.kind() == std::io::ErrorKind::AlreadyExists
                && hash(&destination).as_deref() == Some(&release.sha256) => {}
        Err(error) => return Err(error.to_string()),
    }
    cancel.check().map_err(|error| error.to_string())?;
    Ok(())
}

fn pending_release(root: &Path) -> Option<Release> {
    let release: Release = load(&root.join("pending.json"))?;
    (release.valid() && staged_binary(root, &release).is_some()).then_some(release)
}

fn staged_binary(root: &Path, release: &Release) -> Option<PathBuf> {
    if !release.valid() {
        return None;
    }
    let directory = root.join("versions").join(release.directory());
    for path in [root.to_owned(), root.join("versions"), directory.clone()] {
        if path.symlink_metadata().ok()?.file_type().is_symlink() {
            return None;
        }
    }
    let binary = directory.join(ASSET);
    let metadata = binary.metadata().ok()?;
    if metadata.len() != release.size || hash(&binary)? != release.sha256 {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return None;
        }
    }
    Some(binary)
}

fn activate(root: &Path, release: &Release, cancel: &Cancellation) -> Result<(), String> {
    cancel.check().map_err(|error| error.to_string())?;
    if staged_binary(root, release).is_none() {
        return Err(
            "The prepared update could not be verified. Your current version is unchanged.".into(),
        );
    }
    save(&root.join("active.json"), release)?;
    let _ = fs::remove_file(root.join("pending.json"));
    Ok(())
}

#[cfg(test)]
fn commit(
    root: &Path,
    release: &Release,
    file: tempfile::NamedTempFile,
    cancel: &Cancellation,
) -> Result<(), String> {
    stage(root, release, file, cancel)?;
    activate(root, release, cancel)
}

pub fn install(
    root: &Path,
    base: &Toolchain,
    channel: Channel,
    cancel: &Cancellation,
    activate_if_idle: impl FnOnce(&mut dyn FnMut() -> Result<(), String>) -> Result<bool, String>,
    progress: impl Fn(&str),
) -> Result<Status, String> {
    if !cfg!(target_os = "macos") {
        return Err("Downloader updates currently support the macOS app.".into());
    }
    cancel.check().map_err(|error| error.to_string())?;
    if preferences(root)?.mode != Mode::Managed {
        return Err("Your local yt-dlp is managed outside Jacket.".into());
    }
    progress("checking");
    let cached: Check = load(&root.join("check.json")).unwrap_or_default();
    let release = match cached.release.filter(|release| {
        release.valid()
            && release.channel == channel
            && cached.checked_at <= now()
            && now().saturating_sub(cached.checked_at) < DAY
    }) {
        Some(release) => release,
        None => fetch_release(channel)?,
    };
    cancel.check().map_err(|error| error.to_string())?;
    private_directory(root)?;
    // A verified candidate survives a busy album job or application restart.
    // Reuse it instead of downloading the same release again.
    if staged_binary(root, &release).is_none() {
        progress("downloading");
        let mut file = tempfile::NamedTempFile::new_in(root).map_err(|error| error.to_string())?;
        let mut response = client()?
            .get(release.url(ASSET))
            .send()
            .map_err(|error| error.to_string())?
            .error_for_status()
            .map_err(|error| error.to_string())?;
        let mut buffer = [0; 65536];
        let mut count = 0_u64;
        loop {
            cancel.check().map_err(|error| error.to_string())?;
            let read = response
                .read(&mut buffer)
                .map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            count += read as u64;
            if count > MAX_BINARY || count > release.size {
                return Err("The downloader exceeded its expected download size.".into());
            }
            file.write_all(&buffer[..read])
                .map_err(|error| error.to_string())?;
        }
        file.as_file_mut()
            .sync_all()
            .map_err(|error| error.to_string())?;
        progress("verifying");
        stage(root, &release, file, cancel)?;
    }
    cancel.check().map_err(|error| error.to_string())?;
    save(&root.join("pending.json"), &release)?;
    activate_if_idle(&mut || activate(root, &release, cancel))?;
    let check = Check {
        channel,
        checked_at: now(),
        release: Some(release),
        error: None,
    };
    save(&root.join("check.json"), &check)?;
    status(root, base, &check)
}

pub fn use_bundled(root: &Path) -> Result<(), String> {
    let mut prefs = preferences(root)?;
    prefs.mode = Mode::Managed;
    prefs.auto_update = false;
    save(&root.join("preferences.json"), &prefs)?;
    let _ = fs::remove_file(root.join("pending.json"));
    let path = root.join("active.json");
    match fs::remove_file(path) {
        Ok(_) => (),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.to_string()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture(root: &Path, version: &str) -> (tempfile::NamedTempFile, Release) {
        let mut file = tempfile::NamedTempFile::new_in(root).unwrap();
        let payload = format!(
            "#!/bin/sh\ncase \"$*\" in *--version*) echo {version};; *) echo youtube;; esac\n"
        );
        file.write_all(payload.as_bytes()).unwrap();
        let release = Release {
            channel: Channel::Nightly,
            version: version.into(),
            sha256: hex::encode(Sha256::digest(payload.as_bytes())),
            size: payload.len() as u64,
        };
        (file, release)
    }

    fn fixture_tools(binary: PathBuf) -> Toolchain {
        Toolchain {
            yt_dlp: binary.clone(),
            ffmpeg: binary.clone(),
            ffprobe: binary,
            node: None,
        }
    }

    #[test]
    #[cfg(unix)]
    fn file_picker_resolution_retains_the_upgradable_symlink_only_for_the_same_file() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("yt-dlp-versioned");
        let other = root.path().join("other");
        let alias = root.path().join("yt-dlp");
        fs::write(&target, b"tool").unwrap();
        fs::write(&other, b"other").unwrap();
        symlink(&target, &alias).unwrap();
        let alias = alias.to_string_lossy().into_owned();
        let target = target.to_string_lossy().into_owned();
        let other = other.to_string_lossy().into_owned();
        assert_eq!(selection_path(target.clone(), Some(alias.clone())), alias);
        assert_eq!(selection_path(other.clone(), Some(alias)), other);
        assert_eq!(
            selection_path(target.clone(), Some("relative/path".into())),
            target
        );
    }

    #[test]
    #[cfg(unix)]
    fn local_install_preserves_package_manager_symlinks_and_never_checks_github() {
        use std::os::unix::{fs::symlink, fs::PermissionsExt};
        let root = tempfile::tempdir().unwrap();
        let (file, release) = fixture(root.path(), "2026.08.19");
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(0o700))
            .unwrap();
        let local = root.path().join("homebrew-yt-dlp");
        symlink(file.path(), &local).unwrap();
        let tools = fixture_tools(file.path().to_owned());
        let prefs = Preferences {
            mode: Mode::Local,
            auto_update: true,
            local_path: Some(local.to_string_lossy().into_owned()),
        };
        let value = configure(root.path(), &tools, prefs, |apply| apply()).unwrap();
        assert!(value.mode == Mode::Local);
        assert!(!value.update_available);
        assert_eq!(selected_path(root.path(), &tools.yt_dlp).unwrap(), local);
        let checked = check_with_fetch(root.path(), &tools, true, None, |_| {
            panic!("Local mode must not fetch releases")
        })
        .unwrap();
        assert!(checked.latest_version.is_none());
        assert_eq!(hash(file.path()).unwrap(), release.sha256);
        // An external package-manager update is observed through the same link.
        fs::write(file.path(), "#!/bin/sh\ncase \"$*\" in *--version*) echo 2026.10.10.233141;; *) echo youtube;; esac\n").unwrap();
        assert_eq!(
            check_with_fetch(root.path(), &tools, false, None, |_| panic!())
                .unwrap()
                .installed_version,
            "2026.10.10.233141"
        );
        fs::remove_file(local).unwrap();
        assert!(selected_path(root.path(), &tools.yt_dlp).is_err());
        assert!(status(root.path(), &tools, &Check::default())
            .unwrap()
            .error
            .is_some());
    }

    #[test]
    #[cfg(unix)]
    fn invalid_local_selection_and_busy_configuration_leave_current_settings_unchanged() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let (file, _) = fixture(root.path(), "2026.08.19");
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(0o700))
            .unwrap();
        let tools = fixture_tools(file.path().to_owned());
        let prefs = Preferences {
            mode: Mode::Local,
            auto_update: true,
            local_path: Some("relative/yt-dlp".into()),
        };
        assert!(configure(root.path(), &tools, prefs, |apply| apply()).is_err());
        let prefs = Preferences {
            mode: Mode::Local,
            auto_update: true,
            local_path: Some(file.path().to_string_lossy().into_owned()),
        };
        assert!(configure(root.path(), &tools, prefs, |_| Err(
            "An album is running".into()
        ))
        .is_err());
        assert!(preferences(root.path()).unwrap().mode == Mode::Managed);
        assert!(!root.path().join("preferences.json").exists());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn prepared_updates_wait_for_idle_and_reuse_the_verified_candidate_after_restart() {
        let root = tempfile::tempdir().unwrap();
        let (file, previous) = fixture(root.path(), "2026.08.19");
        commit(root.path(), &previous, file, &Cancellation::default()).unwrap();
        let previous_path = override_path(root.path()).unwrap();
        let tools = fixture_tools(previous_path.clone());
        let (file, release) = fixture(root.path(), "2026.10.10.233141");
        stage(root.path(), &release, file, &Cancellation::default()).unwrap();
        save(
            &root.path().join("check.json"),
            &Check {
                channel: Channel::Nightly,
                checked_at: now(),
                release: Some(release),
                error: None,
            },
        )
        .unwrap();
        let waiting = install(
            root.path(),
            &tools,
            Channel::Nightly,
            &Cancellation::default(),
            |_| Ok(false),
            |phase| assert_eq!(phase, "checking"),
        )
        .unwrap();
        assert!(waiting.pending_activation);
        assert_eq!(waiting.installed_version, previous.version);
        assert_eq!(override_path(root.path()).unwrap(), previous_path);
        assert!(pending_release(root.path()).is_some());
        let switched = install(
            root.path(),
            &tools,
            Channel::Nightly,
            &Cancellation::default(),
            |apply| {
                apply()?;
                Ok(true)
            },
            |phase| assert_eq!(phase, "checking"),
        )
        .unwrap();
        assert!(!switched.pending_activation);
        assert_eq!(switched.installed_version, "2026.10.10.233141");
        // Jobs that captured the previous executable still have a working file.
        assert_eq!(
            version(&previous_path, &Cancellation::default()).unwrap(),
            previous.version
        );
    }

    #[test]
    #[cfg(unix)]
    fn explicit_fallback_pauses_automatic_updates_and_returns_to_included_tools() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let (included, _) = fixture(root.path(), "2026.08.19");
        included
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o700))
            .unwrap();
        let tools = fixture_tools(included.path().to_owned());
        let (file, release) = fixture(root.path(), "2026.10.10.233141");
        commit(root.path(), &release, file, &Cancellation::default()).unwrap();
        use_bundled(root.path()).unwrap();
        let value = status(root.path(), &tools, &Check::default()).unwrap();
        assert!(!value.auto_update);
        assert!(!value.using_updated_copy);
        assert_eq!(value.installed_version, "2026.08.19");
        assert_eq!(
            selected_path(root.path(), &tools.yt_dlp).unwrap(),
            tools.yt_dlp
        );
    }

    #[test]
    #[cfg(unix)]
    fn daily_checks_are_cached_and_offline_checks_do_not_replace_a_known_release() {
        let root = tempfile::tempdir().unwrap();
        let (file, release) = fixture(root.path(), "2026.10.10.233141");
        commit(root.path(), &release, file, &Cancellation::default()).unwrap();
        let binary = override_path(root.path()).unwrap();
        let tools = Toolchain {
            yt_dlp: binary.clone(),
            ffmpeg: binary.clone(),
            ffprobe: binary,
            node: None,
        };
        save(
            &root.path().join("check.json"),
            &Check {
                channel: Channel::Nightly,
                checked_at: now(),
                release: Some(release.clone()),
                error: None,
            },
        )
        .unwrap();
        let cached = check_with_fetch(root.path(), &tools, false, None, |_| {
            panic!("Daily cache must prevent a network request")
        })
        .unwrap();
        assert!(!cached.update_available);
        let failed =
            check_with_fetch(root.path(), &tools, true, None, |_| Err("Offline".into())).unwrap();
        assert_eq!(failed.latest_version, Some(release.version));
        assert!(failed.error.is_some());
        assert!(failed.using_updated_copy);
        assert!(override_path(root.path()).is_some());
    }

    #[test]
    fn official_release_and_checksum_are_bound_to_the_channel_asset_and_version() {
        let value = json!({"tag_name":"2026.10.10.233141", "draft":false, "assets":[{"name":ASSET,"size":42,"digest":format!("sha256:{}", "a".repeat(64)),"browser_download_url":"https://github.com/yt-dlp/yt-dlp-nightly-builds/releases/download/2026.10.10.233141/yt-dlp_macos"}]});
        assert!(release_from_json(&value, Channel::Nightly).unwrap().valid());
        assert!(release_from_json(&value, Channel::Stable).is_err());
        let mut bad = value.clone();
        bad["tag_name"] = json!("../../escape");
        assert!(release_from_json(&bad, Channel::Nightly).is_err());
        let mut bad = value;
        bad["assets"][0]["browser_download_url"] = json!("https://example.com/tool");
        assert!(release_from_json(&bad, Channel::Nightly).is_err());
        assert!(!allowed_url(
            &reqwest::Url::parse("https://github.com.evil.test/file").unwrap()
        ));
        assert_eq!(
            checksum_from_manifest(format!("{} *yt-dlp_macos\n", "a".repeat(64)).as_bytes()),
            Some("a".repeat(64))
        );
        assert!(
            checksum_from_manifest(format!("{} yt-dlp.exe\n", "a".repeat(64)).as_bytes()).is_none()
        );
    }

    #[test]
    fn nightly_versions_compare_without_downgrading_due_to_string_sorting() {
        assert!(version_parts("2026.10.10.233141") > version_parts("2026.08.19"));
        assert_eq!(version_parts("2026.8.19"), version_parts("2026.08.19"));
        assert!(version_parts("2026.13.01").is_none());
    }

    #[test]
    #[cfg(unix)]
    fn failed_validation_leaves_the_current_copy_active_and_activation_is_atomic() {
        let root = tempfile::tempdir().unwrap();
        let (previous_file, previous_release) = fixture(root.path(), "2026.08.19");
        commit(
            root.path(),
            &previous_release,
            previous_file,
            &Cancellation::default(),
        )
        .unwrap();
        let previous = load::<Value>(&root.path().join("active.json")).unwrap();
        let previous_binary = override_path(root.path()).unwrap();
        let mut file = tempfile::NamedTempFile::new_in(root.path()).unwrap();
        let payload = b"#!/bin/sh\ncase \"$*\" in *--version*) echo 2026.10.10.233141;; *) echo youtube;; esac\n";
        file.write_all(payload).unwrap();
        let mut release = Release {
            channel: Channel::Nightly,
            version: "2026.10.10.233141".into(),
            sha256: "0".repeat(64),
            size: payload.len() as u64,
        };
        assert!(commit(root.path(), &release, file, &Cancellation::default()).is_err());
        assert_eq!(
            load::<Value>(&root.path().join("active.json")).unwrap(),
            previous
        );
        assert_eq!(override_path(root.path()).unwrap(), previous_binary);
        assert_eq!(
            version(&previous_binary, &Cancellation::default()).unwrap(),
            "2026.08.19"
        );
        let mut file = tempfile::NamedTempFile::new_in(root.path()).unwrap();
        file.write_all(payload).unwrap();
        release.sha256 = hex::encode(Sha256::digest(payload));
        commit(root.path(), &release, file, &Cancellation::default()).unwrap();
        let selected = override_path(root.path()).unwrap();
        assert_eq!(
            version(&selected, &Cancellation::default()).unwrap(),
            release.version
        );
        fs::write(selected, b"tampered").unwrap();
        assert!(override_path(root.path()).is_none());
    }

    #[test]
    #[cfg(unix)]
    fn cancellation_and_wrong_reported_versions_never_activate_a_candidate() {
        let root = tempfile::tempdir().unwrap();
        let (file, mut release) = fixture(root.path(), "2026.08.19");
        release.version = "2026.10.10.233141".into();
        assert!(commit(root.path(), &release, file, &Cancellation::default()).is_err());
        assert!(!root.path().join("active.json").exists());
        let (file, release) = fixture(root.path(), "2026.10.10.233141");
        let cancelled = Cancellation::default();
        cancelled.cancel();
        assert!(commit(root.path(), &release, file, &cancelled).is_err());
        assert!(!root.path().join("active.json").exists());
    }
}
