//! Discover profile metadata only. Cookie access happens only on explicit tests/jobs.
use audio_engine::{AuthOptions, Browser, Cancellation, Toolchain};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub label: String,
    pub detail: String,
    #[serde(skip)]
    path: PathBuf,
    #[serde(skip)]
    container: Option<String>,
}

#[derive(Serialize)]
pub struct Catalog {
    pub profiles: Vec<Profile>,
    pub notice: Option<String>,
}

pub struct Locations {
    home: PathBuf,
    config: PathBuf,
    local: PathBuf,
    roaming: PathBuf,
    platform: &'static str,
}

impl Locations {
    pub fn current(home: PathBuf) -> Self {
        let absolute = |key: &str, fallback: PathBuf| {
            std::env::var_os(key)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .unwrap_or(fallback)
        };
        Self {
            config: absolute("XDG_CONFIG_HOME", home.join(".config")),
            local: absolute("LOCALAPPDATA", home.join("AppData/Local")),
            roaming: absolute("APPDATA", home.join("AppData/Roaming")),
            home,
            platform: std::env::consts::OS,
        }
    }

    fn roots(&self, browser: Browser) -> Vec<PathBuf> {
        match (self.platform, browser) {
            ("macos", Browser::Chrome) => vec![self.home.join("Library/Application Support/Google/Chrome")],
            ("macos", Browser::Edge) => vec![self.home.join("Library/Application Support/Microsoft Edge")],
            ("macos", Browser::Firefox) => vec![self.home.join("Library/Application Support/Firefox")],
            ("macos", Browser::Safari) => vec![self.home.join("Library/Cookies/Cookies.binarycookies"), self.home.join("Library/Containers/com.apple.Safari/Data/Library/Cookies/Cookies.binarycookies")],
            ("windows", Browser::Chrome) => vec![self.local.join("Google/Chrome/User Data")],
            ("windows", Browser::Edge) => vec![self.local.join("Microsoft/Edge/User Data")],
            ("windows", Browser::Firefox) => vec![self.roaming.join("Mozilla/Firefox"), self.local.join("Packages/Mozilla.Firefox_n80bbvh6b1yt2/LocalCache/Roaming/Mozilla/Firefox")],
            (_, Browser::Safari) => vec![],
            (_, Browser::Chrome) => vec![self.config.join("google-chrome"), self.home.join(".var/app/com.google.Chrome/config/google-chrome")],
            (_, Browser::Edge) => vec![self.config.join("microsoft-edge"), self.home.join(".var/app/com.microsoft.Edge/config/microsoft-edge")],
            (_, Browser::Firefox) => vec![self.config.join("mozilla/firefox"), self.home.join(".mozilla/firefox"), self.home.join(".var/app/org.mozilla.firefox/config/mozilla/firefox"), self.home.join(".var/app/org.mozilla.firefox/.mozilla/firefox"), self.home.join("snap/firefox/common/.mozilla/firefox")],
        }
    }
}

fn metadata(path: &Path) -> Option<String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 4 * 1024 * 1024 {
        return None;
    }
    String::from_utf8(bytes).ok()
}

fn json(path: &Path) -> Option<Value> {
    serde_json::from_str(&metadata(path)?).ok()
}
fn label(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(100)
        .collect()
}
fn single_directory(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(['/', '\\', ':'])
        && !name.chars().any(char::is_control)
        && Path::new(name).components().count() == 1
        && matches!(
            Path::new(name).components().next(),
            Some(Component::Normal(_))
        )
}

fn profile(
    browser: Browser,
    path: PathBuf,
    name: &str,
    detail: &str,
    container: Option<String>,
) -> Option<Profile> {
    let path = path.canonicalize().ok()?;
    let text = path.to_str()?;
    if text.contains("::") || text.chars().any(char::is_control) {
        return None;
    }
    let id = format!(
        "{}-{}",
        browser.as_str(),
        hex::encode(Sha256::digest(
            format!(
                "{}\0{text}\0{}",
                browser.as_str(),
                container.as_deref().unwrap_or("")
            )
            .as_bytes()
        ))
    );
    Some(Profile {
        id,
        path,
        label: label(name),
        detail: label(detail),
        container,
    })
}

fn chromium(root: &Path, browser: Browser) -> Vec<Profile> {
    let cache = json(&root.join("Local State"))
        .and_then(|value| value.pointer("/profile/info_cache").cloned());
    let mut names = BTreeMap::new();
    if let Some(cache) = cache.as_ref().and_then(Value::as_object) {
        for (directory, info) in cache {
            if single_directory(directory) {
                names.insert(
                    directory.clone(),
                    info.get("name")
                        .and_then(Value::as_str)
                        .unwrap_or(directory)
                        .to_owned(),
                );
            }
        }
    }
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let directory = entry.file_name().to_string_lossy().into_owned();
            if directory == "Default"
                || directory
                    .strip_prefix("Profile ")
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            {
                names.entry(directory.clone()).or_insert(directory);
            }
        }
    }
    let Ok(canonical_root) = root.canonicalize() else {
        return vec![];
    };
    names
        .into_iter()
        .filter_map(|(directory, name)| {
            let path = root.join(&directory);
            if !path.is_dir() || !path.canonicalize().ok()?.starts_with(&canonical_root) {
                return None;
            }
            profile(browser, path, &name, &directory, None)
        })
        .collect()
}

fn ini_profiles(contents: &str) -> Vec<BTreeMap<String, String>> {
    let mut result = vec![];
    let mut current = false;
    for line in contents.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            current = line.starts_with("[Profile");
            if current {
                result.push(BTreeMap::new());
            }
        } else if current && !line.starts_with([';', '#']) {
            if let Some((key, value)) = line.split_once('=') {
                if let Some(section) = result.last_mut() {
                    section.insert(key.trim().into(), value.trim().into());
                }
            }
        }
    }
    result
}

fn firefox(root: &Path) -> Vec<Profile> {
    let mut directories = BTreeMap::new();
    if let Some(contents) = metadata(&root.join("profiles.ini")) {
        for section in ini_profiles(&contents) {
            if let Some(directory) = section.get("Path") {
                if section.get("IsRelative").map(String::as_str) != Some("0")
                    && Path::new(directory)
                        .components()
                        .any(|part| !matches!(part, Component::Normal(_)))
                {
                    continue;
                }
                let path = if section.get("IsRelative").map(String::as_str) == Some("0") {
                    PathBuf::from(directory)
                } else {
                    root.join(directory)
                };
                if path.is_absolute() {
                    directories.insert(
                        path,
                        section
                            .get("Name")
                            .cloned()
                            .unwrap_or_else(|| "Firefox profile".into()),
                    );
                }
            }
        }
    }
    // Portable/older installations can lack profiles.ini. Only enumerate
    // immediate standard profile directories; never search the user's home.
    for base in [root.to_path_buf(), root.join("Profiles")] {
        if let Ok(entries) = fs::read_dir(&base) {
            for entry in entries.flatten() {
                if entry.path().is_dir() && entry.path().join("prefs.js").is_file() {
                    directories
                        .entry(entry.path())
                        .or_insert_with(|| entry.file_name().to_string_lossy().into_owned());
                }
            }
        }
    }
    let mut result = vec![];
    for (path, name) in directories {
        if !path.is_dir() {
            continue;
        }
        if let Some(standard) = profile(
            Browser::Firefox,
            path.clone(),
            &name,
            "Tabs outside containers",
            Some("none".into()),
        ) {
            result.push(standard);
        }
        if let Some(containers) = json(&path.join("containers.json"))
            .and_then(|value| value.get("identities").and_then(Value::as_array).cloned())
        {
            for container in containers {
                if container.get("public").and_then(Value::as_bool) == Some(false) {
                    continue;
                }
                if let Some(name_or_id) = container
                    .get("name")
                    .and_then(Value::as_str)
                    .or_else(|| container.get("l10nID").and_then(Value::as_str))
                {
                    if name_or_id.is_empty()
                        || name_or_id == "none"
                        || name_or_id.contains("::")
                        || name_or_id.chars().any(char::is_control)
                    {
                        continue;
                    }
                    if let Some(value) = profile(
                        Browser::Firefox,
                        path.clone(),
                        &format!("{name} · {name_or_id}"),
                        "Firefox container",
                        Some(name_or_id.into()),
                    ) {
                        result.push(value);
                    }
                }
            }
        }
    }
    result
}

pub fn discover(locations: &Locations, browser: Browser) -> Catalog {
    let mut profiles = vec![];
    for root in locations.roots(browser) {
        match browser {
            Browser::Chrome | Browser::Edge => profiles.extend(chromium(&root, browser)),
            Browser::Firefox => profiles.extend(firefox(&root)),
            Browser::Safari => {
                if root.is_file() {
                    if let Some(value) = profile(
                        browser,
                        root,
                        "Safari cookie store",
                        "Safari named profiles are not supported by this importer",
                        None,
                    ) {
                        profiles.push(value);
                    }
                }
            }
        }
    }
    profiles.sort_by(|a, b| {
        a.label
            .cmp(&b.label)
            .then(a.detail.cmp(&b.detail))
            .then(a.id.cmp(&b.id))
    });
    let mut seen = std::collections::HashSet::new();
    profiles.retain(|profile| seen.insert(profile.id.clone()));
    let notice = if profiles.is_empty() {
        Some(if browser == Browser::Safari { "Safari’s cookie store is unavailable. macOS may restrict access; you can choose another signed-in browser." }
            else { "No readable profiles found. Open this browser and sign in to YouTube, then refresh the list. If profiles still do not appear, check Jacket’s access to browser data." }.into())
    } else if browser == Browser::Safari {
        Some("Safari named profiles cannot be selected here. Use the available cookie store, or choose Chrome, Firefox, or Edge for explicit profile selection.".into())
    } else {
        None
    };
    Catalog { profiles, notice }
}

pub fn resolve(locations: &Locations, requested: &AuthOptions) -> Result<AuthOptions, String> {
    let mut resolved = requested.clone();
    resolved.browser_profile_path = None;
    resolved.browser_container = None;
    let Some(browser) = requested.browser else {
        if requested.browser_profile.is_some() {
            return Err("Choose a browser for this profile in Session settings.".into());
        }
        return Ok(resolved);
    };
    if requested.managed_session.is_some() {
        return Err("Choose one session source.".into());
    }
    let selected = requested.browser_profile.as_ref().and_then(|id| discover(locations, browser).profiles.into_iter().find(|profile| profile.id == *id))
        .ok_or("The selected browser profile is no longer available. Open Session settings, refresh the list, and choose your signed-in profile. Jacket will not switch accounts automatically.")?;
    resolved.browser_profile_path = Some(selected.path);
    resolved.browser_container = selected.container;
    Ok(resolved)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessCheck {
    pub accessible: bool,
    pub message: String,
    pub guidance: String,
}

pub fn test_access(
    tools: &Toolchain,
    auth: &AuthOptions,
    url: &str,
    cancel: &Cancellation,
) -> Result<AccessCheck, String> {
    let url = audio_engine::source::validate_source_url(url).map_err(|error| error.to_string())?;
    if auth.browser.is_none() {
        return Err("Choose a browser and profile to test.".into());
    }
    let mut command =
        audio_engine::source::downloader(tools, auth).map_err(|error| error.to_string())?;
    command.args([
        "--skip-download",
        "--dump-single-json",
        "--playlist-end",
        "1",
        "--socket-timeout",
        "15",
        "--retries",
        "0",
        "--extractor-retries",
        "0",
        "--",
        &url,
    ]);
    match audio_engine::process::run(&mut command, cancel, Duration::from_secs(60)) {
        Ok(bytes) => {
            let result: Value = serde_json::from_slice(&bytes).map_err(|_| "The downloader returned an unreadable access-test response. Update yt-dlp and try again.")?;
            if result
                .get("id")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
                || result.get("entries").is_some_and(|entries| {
                    entries.as_array().is_none_or(|entries| {
                        entries.is_empty() || entries.iter().all(Value::is_null)
                    })
                })
            {
                return Ok(AccessCheck { accessible: false, message: "No accessible item was returned for this link.".into(), guidance: "Open the link in the selected browser profile and check that it contains an item you can play. Then test again.".into() });
            }
            Ok(AccessCheck { accessible: true, message: "This link is accessible with the selected browser profile.".into(), guidance: "A public link can also work without signing in. To confirm account-only access, test a private playlist or another restricted item your account can play. Jacket reads the selected session again for each operation.".into() })
        }
        Err(audio_engine::EngineError::Cancelled) => Err("Access test cancelled.".into()),
        Err(error) => {
            let detail = error.to_string();
            if let Some(problem) = audio_engine::access::problem(&detail) {
                Ok(AccessCheck {
                    accessible: false,
                    message: problem.message.into(),
                    guidance: problem.guidance.into(),
                })
            } else {
                Ok(AccessCheck { accessible: false, message: "The link could not be checked.".into(), guidance: "Check the link and connection, or update yt-dlp, then test again. A provider error such as HTTP 403 does not by itself mean that you need to sign in again.".into() })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locations(home: &Path, platform: &'static str) -> Locations {
        Locations {
            home: home.into(),
            config: home.join(".config"),
            local: home.join("AppData/Local"),
            roaming: home.join("AppData/Roaming"),
            platform,
        }
    }
    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    fn request(browser: Browser, id: &str) -> AuthOptions {
        AuthOptions {
            browser: Some(browser),
            browser_profile: Some(id.into()),
            ..AuthOptions::default()
        }
    }

    #[test]
    fn chromium_names_and_ids_do_not_depend_on_last_used_profile_and_do_not_expose_paths() {
        let home = tempfile::tempdir().unwrap();
        let locations = locations(home.path(), "macos");
        let root = &locations.roots(Browser::Chrome)[0];
        write(
            &root.join("Local State"),
            r#"{"profile":{"last_used":"Profile 2","info_cache":{"Default":{"name":"Personal","user_name":"do-not-emit@example.test"},"Profile 2":{"name":"Music"},"../escape":{"name":"Invalid"}}}}"#,
        );
        fs::create_dir_all(root.join("Default")).unwrap();
        write(
            &root.join("Profile 2/Cookies"),
            "This is not a cookie database. Discovery never reads it.",
        );
        let catalog = discover(&locations, Browser::Chrome);
        assert_eq!(catalog.profiles.len(), 2);
        let selected = catalog
            .profiles
            .iter()
            .find(|profile| profile.label == "Personal")
            .unwrap();
        let auth = request(Browser::Chrome, &selected.id);
        let original = resolve(&locations, &auth)
            .unwrap()
            .browser_profile_path
            .unwrap();
        write(
            &root.join("Local State"),
            r#"{"profile":{"last_used":"Default","info_cache":{"Default":{"name":"Renamed"},"Profile 2":{"name":"Music"}}}}"#,
        );
        assert_eq!(
            resolve(&locations, &auth)
                .unwrap()
                .browser_profile_path
                .unwrap(),
            original
        );
        let encoded = serde_json::to_string(&catalog).unwrap();
        assert!(!encoded.contains(&home.path().to_string_lossy().to_string()));
        assert!(!encoded.contains("do-not-emit"));
        assert!(resolve(&locations, &request(Browser::Edge, &selected.id)).is_err());
        assert!(resolve(
            &locations,
            &AuthOptions {
                browser: Some(Browser::Chrome),
                ..AuthOptions::default()
            }
        )
        .is_err());
        fs::remove_dir(root.join("Default")).unwrap();
        assert!(resolve(&locations, &auth)
            .unwrap_err()
            .contains("will not switch"));
    }

    #[test]
    fn firefox_ini_absolute_profiles_and_containers_are_selected_explicitly() {
        let home = tempfile::tempdir().unwrap();
        let locations = locations(home.path(), "macos");
        let root = &locations.roots(Browser::Firefox)[0];
        let external = home.path().join("Custom Profile");
        fs::create_dir_all(&external).unwrap();
        write(&root.join("profiles.ini"), &format!("[General]\nStartWithLastProfile=1\n[Profile0]\nName=Music\nIsRelative=1\nPath=Profiles/example.default\n[Profile1]\nName=Custom\nIsRelative=0\nPath={}\n[Profile2]\nName=Invalid\nPath=../escape\n", external.display()));
        write(
            &root.join("Profiles/example.default/containers.json"),
            r#"{"identities":[{"name":"Listening","userContextId":3,"public":true},{"name":"Hidden","public":false},{"name":"bad::container"}]}"#,
        );
        let profiles = discover(&locations, Browser::Firefox).profiles;
        assert_eq!(profiles.len(), 3);
        let standard = profiles
            .iter()
            .find(|profile| profile.label == "Music")
            .unwrap();
        assert_eq!(
            resolve(&locations, &request(Browser::Firefox, &standard.id))
                .unwrap()
                .browser_container
                .as_deref(),
            Some("none")
        );
        let container = profiles
            .iter()
            .find(|profile| profile.label.contains("Listening"))
            .unwrap();
        assert_eq!(
            resolve(&locations, &request(Browser::Firefox, &container.id))
                .unwrap()
                .browser_container
                .as_deref(),
            Some("Listening")
        );
        let custom = profiles
            .iter()
            .find(|profile| profile.label == "Custom")
            .unwrap();
        assert_eq!(custom.path, external.canonicalize().unwrap());
    }

    #[test]
    fn discovery_handles_linux_windows_missing_browsers_and_safari_honestly() {
        let home = tempfile::tempdir().unwrap();
        for platform in ["linux", "windows"] {
            let locations = locations(home.path(), platform);
            let root = locations.roots(Browser::Chrome)[0].clone();
            fs::create_dir_all(root.join("Default")).unwrap();
            assert_eq!(discover(&locations, Browser::Chrome).profiles.len(), 1);
            assert!(discover(&locations, Browser::Safari).profiles.is_empty());
        }
        let locations = locations(home.path(), "macos");
        let empty = discover(&locations, Browser::Edge);
        assert!(empty.profiles.is_empty());
        assert!(empty.notice.unwrap().contains("No readable profiles"));
        let store = locations.roots(Browser::Safari)[0].clone();
        write(&store, "synthetic only, not a cookie database");
        let safari = discover(&locations, Browser::Safari);
        assert_eq!(safari.profiles.len(), 1);
        assert!(safari.notice.unwrap().contains("named profiles"));
        assert_eq!(
            resolve(
                &locations,
                &request(Browser::Safari, &safari.profiles[0].id)
            )
            .unwrap()
            .browser_profile_path
            .unwrap(),
            store.canonicalize().unwrap()
        );
    }

    #[test]
    fn caller_cannot_supply_a_path_or_inject_another_profile() {
        let home = tempfile::tempdir().unwrap();
        let locations = locations(home.path(), "macos");
        let mut supplied: AuthOptions = serde_json::from_value(serde_json::json!({"browser":"chrome","browserProfile":"/unrelated/profile","browserProfilePath":"/caller/path","browserContainer":"injected"})).unwrap();
        assert!(supplied.browser_profile_path.is_none());
        assert!(resolve(&locations, &supplied).is_err());
        supplied.browser = None;
        assert!(resolve(&locations, &supplied).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn chromium_profile_symlink_cannot_escape_the_browser_directory() {
        let home = tempfile::tempdir().unwrap();
        let locations = locations(home.path(), "macos");
        let root = &locations.roots(Browser::Chrome)[0];
        let unrelated = home.path().join("unrelated");
        fs::create_dir_all(root).unwrap();
        fs::create_dir(&unrelated).unwrap();
        std::os::unix::fs::symlink(unrelated, root.join("Default")).unwrap();
        assert!(discover(&locations, Browser::Chrome).profiles.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn access_testing_checks_one_item_without_exporting_cookies_or_media_and_recovers_from_expiry()
    {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().unwrap();
        let locations = locations(home.path(), "macos");
        fs::create_dir_all(locations.roots(Browser::Chrome)[0].join("Default")).unwrap();
        let selected = discover(&locations, Browser::Chrome).profiles.remove(0);
        let auth = resolve(&locations, &request(Browser::Chrome, &selected.id)).unwrap();
        let binary = home.path().join("downloader");
        let args = home.path().join("arguments.txt");
        write(&binary, &format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\necho '{{\"id\":\"fixture\",\"availability\":\"private\"}}'\n", args.display()));
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        let tools = Toolchain {
            yt_dlp: binary.clone(),
            ffmpeg: "unused".into(),
            ffprobe: "unused".into(),
            node: None,
        };
        let url = "https://www.youtube.com/watch?v=fixture";
        assert!(
            test_access(&tools, &auth, url, &Cancellation::default())
                .unwrap()
                .accessible
        );
        let passed = fs::read_to_string(&args).unwrap();
        assert!(passed.contains("--skip-download\n"));
        assert!(passed.contains("--playlist-end\n1\n"));
        assert!(passed.contains(&format!("chrome:{}", selected.path.display())));
        assert!(!passed.contains("--cookies\n"));
        write(&binary, "#!/bin/sh\necho 'WARNING: YouTube account cookies are no longer valid' >&2\necho '{\"id\":\"public-fallback\"}'\n");
        let expired = test_access(&tools, &auth, url, &Cancellation::default()).unwrap();
        assert!(!expired.accessible);
        assert!(expired.guidance.contains("selected browser profile"));
        write(&binary, "#!/bin/sh\necho '{\"id\":\"fixture\"}'\n");
        assert!(
            test_access(&tools, &auth, url, &Cancellation::default())
                .unwrap()
                .accessible
        );
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(test_access(&tools, &auth, url, &cancel)
            .unwrap_err()
            .contains("cancelled"));
    }
}
