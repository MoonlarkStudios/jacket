use audio_engine::{
    AuthOptions, Cancellation, ExportPlan, ExportResult, Inspection, InspectionUpdate,
    ProgressEvent, Toolchain,
};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    process::Command,
    sync::{Arc, Mutex},
};
use tauri::{Emitter, Manager, State};

mod artwork_assets;
mod auth_session;
mod browser_sessions;
mod downloader_updates;
mod export_completion;
#[cfg(target_os = "linux")]
mod linux_graphics;
mod recovery;
mod reviews;
mod shell;

#[derive(Default)]
struct UpdateControl {
    serial: Arc<Mutex<()>>,
    cancel: Arc<Mutex<Option<Cancellation>>>,
}

fn updates_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("downloader"))
}

#[tauri::command]
async fn check_downloader_update(
    app: tauri::AppHandle,
    force: bool,
    channel: Option<downloader_updates::Channel>,
) -> Result<downloader_updates::Status, String> {
    let control = app.state::<UpdateControl>().serial.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = control.lock().map_err(|error| error.to_string())?;
        let tools = base_toolchain(&app).map_err(|error| error.to_string())?;
        downloader_updates::check(&updates_root(&app)?, &tools, force, channel)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn update_downloader(
    app: tauri::AppHandle,
    channel: downloader_updates::Channel,
) -> Result<downloader_updates::Status, String> {
    let control = app.state::<UpdateControl>();
    let cancel = Cancellation::default();
    {
        let mut running = control.cancel.lock().map_err(|error| error.to_string())?;
        if running.is_some() {
            return Err("A yt-dlp update is already running.".into());
        }
        *running = Some(cancel.clone());
    }
    let serial = control.serial.clone();
    let worker_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let _guard = serial.lock().map_err(|error| error.to_string())?;
        let tools = base_toolchain(&worker_app).map_err(|error| error.to_string())?;
        downloader_updates::install(
            &updates_root(&worker_app)?,
            &tools,
            channel,
            &cancel,
            |activate| {
                // Only the atomic switch holds the job lock. Transfers and
                // validation never prevent the user from starting an album.
                let jobs = worker_app.state::<Jobs>();
                let active = jobs.active.lock().map_err(|error| error.to_string())?;
                if !active.is_empty() {
                    return Ok(false);
                }
                activate()?;
                Ok(true)
            },
            |phase| {
                let _ = worker_app.emit("downloader-progress", phase);
            },
        )
    })
    .await;
    if let Ok(mut running) = app.state::<UpdateControl>().cancel.lock() {
        *running = None;
    }
    result.map_err(|error| error.to_string())?
}

#[tauri::command]
fn cancel_downloader_update(app: tauri::AppHandle) -> Result<(), String> {
    let control = app.state::<UpdateControl>();
    if let Some(cancel) = control
        .cancel
        .lock()
        .map_err(|error| error.to_string())?
        .as_ref()
    {
        cancel.cancel();
    }
    Ok(())
}

#[tauri::command]
fn downloader_selection_path(path: String, preferred_path: Option<String>) -> String {
    downloader_updates::selection_path(path, preferred_path)
}

#[tauri::command]
async fn configure_downloader(
    app: tauri::AppHandle,
    preferences: downloader_updates::Preferences,
) -> Result<downloader_updates::Status, String> {
    let control = app.state::<UpdateControl>().serial.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = control.lock().map_err(|error| error.to_string())?;
        let tools = base_toolchain(&app).map_err(|error| error.to_string())?;
        downloader_updates::configure(&updates_root(&app)?, &tools, preferences, |apply| {
            let jobs = app.state::<Jobs>();
            let active = jobs.active.lock().map_err(|error| error.to_string())?;
            if !active.is_empty() {
                return Err(
                    "Finish the current download or inspection before changing yt-dlp settings."
                        .into(),
                );
            }
            apply()
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn restore_downloader(app: tauri::AppHandle) -> Result<downloader_updates::Status, String> {
    let control = app.state::<UpdateControl>().serial.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = control.lock().map_err(|error| error.to_string())?;
        let tools = base_toolchain(&app).map_err(|error| error.to_string())?;
        let root = updates_root(&app)?;
        {
            let jobs = app.state::<Jobs>();
            let active = jobs.active.lock().map_err(|error| error.to_string())?;
            if !active.is_empty() {
                return Err(
                    "Finish the current download or inspection before changing yt-dlp settings."
                        .into(),
                );
            }
            downloader_updates::use_bundled(&root)?;
        }
        downloader_updates::check(&root, &tools, false, None)
    })
    .await
    .map_err(|error| error.to_string())?
}

fn jobs_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_cache_dir()
        .map_err(|error| error.to_string())?
        .join("jobs"))
}

#[tauri::command]
fn pending_export(
    app: tauri::AppHandle,
    jobs: State<Jobs>,
) -> Result<Option<recovery::Recovery>, String> {
    if !jobs
        .active
        .lock()
        .map_err(|error| error.to_string())?
        .is_empty()
    {
        return Ok(None);
    }
    Ok(recovery::latest(&jobs_root(&app)?))
}

#[tauri::command]
fn dismiss_recovery(
    app: tauri::AppHandle,
    job_id: String,
    jobs: State<Jobs>,
) -> Result<(), String> {
    if !jobs
        .active
        .lock()
        .map_err(|error| error.to_string())?
        .is_empty()
    {
        return Err("Wait for the current download to finish.".into());
    }
    recovery::dismiss(&jobs_root(&app)?, &job_id).map_err(|error| error.to_string())
}

#[derive(Default)]
struct Jobs {
    active: Mutex<HashMap<String, Cancellation>>,
    exported: Mutex<HashSet<PathBuf>>,
}

fn review_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("review-library"))
}

fn prune_review_artwork(app: &tauri::AppHandle) -> Result<(), String> {
    let root = review_root(app)?;
    let mut retained =
        recovery::artwork_asset_ids(&jobs_root(app)?).map_err(|error| error.to_string())?;
    if let Some(draft) = app.state::<reviews::Store>().load(&root.join("draft"))? {
        if let Some(id) = draft.preferences.shared_artwork_asset {
            retained.insert(id);
        }
    }
    artwork_assets::prune(&root.join("artwork"), &retained)?;
    Ok(())
}

#[tauri::command]
async fn review_draft_load(app: tauri::AppHandle) -> Result<Option<reviews::ReviewDraft>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        export_completion::reconcile(
            &app.state::<reviews::Store>(),
            &review_root(&app)?.join("draft"),
            &jobs_root(&app)?,
        )?;
        app.state::<reviews::Store>()
            .load(&review_root(&app)?.join("draft"))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn review_draft_save(
    app: tauri::AppHandle,
    draft: reviews::ReviewDraft,
) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<reviews::Store>()
            .save(&review_root(&app)?.join("draft"), draft)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn review_draft_discard(
    app: tauri::AppHandle,
    session_id: String,
    revision: u64,
) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<reviews::Store>().discard(
            &review_root(&app)?.join("draft"),
            &session_id,
            revision,
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn import_artwork(
    app: tauri::AppHandle,
    path: String,
    crop: Option<artwork_assets::Crop>,
) -> Result<artwork_assets::Asset, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let tools = base_toolchain(&app).map_err(|error| error.to_string())?;
        artwork_assets::import(
            &review_root(&app)?.join("artwork"),
            &tools,
            std::path::Path::new(&path),
            crop,
        )
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn crop_artwork(
    app: tauri::AppHandle,
    id: String,
    crop: artwork_assets::Crop,
) -> Result<artwork_assets::Asset, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let tools = base_toolchain(&app).map_err(|error| error.to_string())?;
        artwork_assets::crop(&review_root(&app)?.join("artwork"), &tools, &id, crop)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn artwork_asset(app: tauri::AppHandle, id: String) -> Result<artwork_assets::Asset, String> {
    tauri::async_runtime::spawn_blocking(move || {
        artwork_assets::load(&review_root(&app)?.join("artwork"), &id)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn preview_export(plan: ExportPlan) -> Result<audio_engine::ExportPreview, String> {
    audio_engine::preview_export(&plan).map_err(|error| error.to_string())
}

#[tauri::command]
fn folder_destination(directory: String, title: String) -> Result<String, String> {
    let directory = PathBuf::from(directory);
    if !directory.is_absolute() || !directory.is_dir() {
        return Err("Choose an available music folder.".into());
    }
    Ok(audio_engine::export::available_folder_path(
        &directory.join(audio_engine::export::sanitize_component(&title)),
    )
    .to_string_lossy()
    .into_owned())
}

#[tauri::command]
fn open_export_playlist(output_path: String, jobs: State<Jobs>) -> Result<(), String> {
    let root = PathBuf::from(output_path);
    let exported = jobs.exported.lock().map_err(|error| error.to_string())?;
    if !exported.contains(&root) || !root.is_dir() {
        return Err("This exported folder is no longer available.".into());
    }
    let playlist = root.join("playlist.m3u");
    if !playlist.is_file() {
        return Err("The exported playlist is no longer available.".into());
    }
    drop(exported);
    #[cfg(target_os = "macos")]
    let status = Command::new("open").arg(&playlist).status();
    #[cfg(target_os = "windows")]
    let status = Command::new("explorer").arg(&playlist).status();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let status = Command::new("xdg-open").arg(&playlist).status();
    if !status.map_err(|error| error.to_string())?.success() {
        return Err(
            "Choose an audio player that supports M3U playlists, then open the playlist again."
                .into(),
        );
    }
    Ok(())
}

fn browser_locations(app: &tauri::AppHandle) -> Result<browser_sessions::Locations, String> {
    Ok(browser_sessions::Locations::current(
        app.path()
            .home_dir()
            .map_err(|_| "Your home directory is unavailable.")?,
    ))
}

#[tauri::command]
async fn browser_profiles(
    app: tauri::AppHandle,
    browser: audio_engine::Browser,
) -> Result<browser_sessions::Catalog, String> {
    tauri::async_runtime::spawn_blocking(move || {
        Ok(browser_sessions::discover(
            &browser_locations(&app)?,
            browser,
        ))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn test_browser_access(
    app: tauri::AppHandle,
    auth: AuthOptions,
    url: String,
) -> Result<browser_sessions::AccessCheck, String> {
    if auth.managed_session.is_some() {
        return Err("Choose an existing browser profile to test.".into());
    }
    audio_engine::source::validate_source_url(&url).map_err(|error| error.to_string())?;
    let cancel = Cancellation::default();
    {
        let jobs = app.state::<Jobs>();
        let mut active = jobs.active.lock().map_err(|error| error.to_string())?;
        if !active.is_empty() {
            return Err("Wait for the current operation to finish.".into());
        }
        active.insert("browser-access".into(), cancel.clone());
    }
    let worker_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let resolved = browser_sessions::resolve(&browser_locations(&worker_app)?, &auth)?;
        browser_sessions::test_access(
            &toolchain(&worker_app).map_err(|error| error.to_string())?,
            &resolved,
            &url,
            &cancel,
        )
    })
    .await;
    if let Ok(mut active) = app.state::<Jobs>().active.lock() {
        active.remove("browser-access");
    }
    result.map_err(|error| error.to_string())?
}

fn session_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("auth-prototype"))
}

#[tauri::command]
async fn session_status(app: tauri::AppHandle) -> Result<auth_session::Status, String> {
    tauri::async_runtime::spawn_blocking(move || auth_session::status(&session_root(&app)?))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn session_action(
    app: tauri::AppHandle,
    action: String,
    url: Option<String>,
) -> Result<auth_session::Status, String> {
    auth_session::enabled()?;
    if !["connect", "save", "verify", "disconnect"].contains(&action.as_str()) {
        return Err("Unknown session action.".into());
    }
    let cancel = Cancellation::default();
    {
        let jobs = app.state::<Jobs>();
        let mut active = jobs.active.lock().map_err(|error| error.to_string())?;
        if active.contains_key("session") || (action != "disconnect" && !active.is_empty()) {
            return Err("Wait for the current operation to finish.".into());
        }
        if action == "disconnect" {
            for running in active.values() {
                running.cancel();
            }
        }
        active.insert("session".into(), cancel.clone());
    }
    let worker_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let control = worker_app.state::<auth_session::Control>();
        let _guard = control.serial.lock().map_err(|_| "Session state unavailable")?;
        let root = session_root(&worker_app)?;
        match action.as_str() {
            "connect" => auth_session::begin(&root, &control, &auth_session::Keychain),
            "save" => auth_session::finish(&root, &control, &cancel),
            "verify" => auth_session::verify(&root, &auth_session::Keychain, &toolchain(&worker_app).map_err(|error| error.to_string())?, url.as_deref().unwrap_or(""), &cancel),
            "disconnect" => {
                let result = auth_session::disconnect(&root, &control, &auth_session::Keychain);
                // Workers release their session lease after media/cache writes.
                // All account-backed cache records can now be safely removed.
                recovery::clear_managed(&jobs_root(&worker_app)?).map_err(|_| "Session disabled. Could not remove its download cache; try Disconnect again.".to_owned())?;
                result
            },
            _ => unreachable!(),
        }
    }).await;
    if let Ok(mut active) = app.state::<Jobs>().active.lock() {
        active.remove("session");
    }
    result.map_err(|error| error.to_string())?
}

#[tauri::command]
fn reveal_archive(output_path: String, jobs: State<Jobs>) -> Result<(), String> {
    let path = PathBuf::from(output_path);
    let exported = jobs.exported.lock().map_err(|error| error.to_string())?;
    if !exported.contains(&path) || (!path.is_file() && !path.is_dir()) {
        return Err("This archive is no longer available at the saved location.".into());
    }
    drop(exported);
    #[cfg(target_os = "macos")]
    let status = if path.is_dir() {
        Command::new("open").arg(&path).status()
    } else {
        Command::new("open").arg("-R").arg(&path).status()
    };
    #[cfg(target_os = "windows")]
    let status = if path.is_dir() {
        Command::new("explorer").arg(&path).status()
    } else {
        Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .status()
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let status = Command::new("xdg-open")
        .arg(if path.is_dir() {
            path.as_path()
        } else {
            path.parent().ok_or("Export folder is unavailable")?
        })
        .status();
    let status = status.map_err(|error| error.to_string())?;
    if !status.success() {
        return Err("Could not open the archive folder.".into());
    }
    Ok(())
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct JobEvent {
    job_id: String,
    #[serde(flatten)]
    progress: ProgressEvent,
    result: Option<ExportResult>,
    error: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct InspectionEvent {
    inspection_id: String,
    #[serde(flatten)]
    update: InspectionUpdate,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DependencyStatus {
    name: String,
    available: bool,
    bundled: bool,
}

fn base_toolchain(app: &tauri::AppHandle) -> audio_engine::Result<Toolchain> {
    let resources = app
        .path()
        .resource_dir()
        .map_err(|error| audio_engine::EngineError::Invalid(error.to_string()))?;
    Toolchain::discover_with_resources(&resources)
}

fn toolchain(app: &tauri::AppHandle) -> audio_engine::Result<Toolchain> {
    let mut tools = base_toolchain(app)?;
    tools.yt_dlp = downloader_updates::selected_path(
        &updates_root(app).map_err(audio_engine::EngineError::Invalid)?,
        &tools.yt_dlp,
    )
    .map_err(audio_engine::EngineError::Invalid)?;
    Ok(tools)
}

#[tauri::command]
fn archive_destination(
    app: tauri::AppHandle,
    directory: Option<String>,
    filename: String,
) -> Result<String, String> {
    let name = std::path::Path::new(&filename);
    if name.components().count() != 1
        || name.file_name().and_then(|s| s.to_str()) != Some(&filename)
        || name.extension().and_then(|s| s.to_str()) != Some("zip")
    {
        return Err("Use a ZIP filename without folders.".into());
    }
    let folder = match directory.filter(|value| !value.is_empty()) {
        Some(value) => PathBuf::from(value),
        None => app
            .path()
            .download_dir()
            .map_err(|error| error.to_string())?,
    };
    if !folder.is_absolute() || !folder.is_dir() {
        return Err("Choose an available download folder.".into());
    }
    Ok(
        audio_engine::export::available_archive_path(&folder.join(name))
            .to_string_lossy()
            .into_owned(),
    )
}

#[tauri::command]
fn dependency_status(app: tauri::AppHandle) -> Vec<DependencyStatus> {
    let bundled = app
        .path()
        .resource_dir()
        .ok()
        .map(|directory| directory.join("audio-tools"));
    let bundled = bundled.filter(|directory| directory.is_dir());
    ["yt-dlp", "ffmpeg", "ffprobe"]
        .into_iter()
        .map(|name| DependencyStatus {
            name: name.into(),
            available: bundled.as_ref().map_or_else(
                || audio_engine::process::find_tool(name).is_some(),
                |directory| directory.join("bin").join(name).is_file(),
            ),
            bundled: bundled.is_some(),
        })
        .collect()
}

#[tauri::command]
async fn inspect_source(
    app: tauri::AppHandle,
    url: String,
    auth: AuthOptions,
    inspection_id: String,
) -> Result<Inspection, String> {
    let auth = browser_sessions::resolve(&browser_locations(&app)?, &auth)?;
    if inspection_id.is_empty()
        || !inspection_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err("Invalid inspection identifier".into());
    }
    let cancel = Cancellation::default();
    {
        let jobs = app.state::<Jobs>();
        let mut active = jobs.active.lock().map_err(|error| error.to_string())?;
        if !active.is_empty() {
            return Err("Wait for the current operation to finish.".into());
        }
        active.insert("inspection".into(), cancel.clone());
    }
    let event_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let control = event_app.state::<auth_session::Control>();
        let _guard = control
            .serial
            .lock()
            .map_err(|_| audio_engine::EngineError::Invalid("Session state unavailable".into()))?;
        cancel.check()?;
        let handoff = auth_session::materialize(
            &session_root(&event_app).map_err(audio_engine::EngineError::Invalid)?,
            &auth_session::Keychain,
            &auth,
        )
        .map_err(audio_engine::EngineError::Invalid)?;
        let auth = handoff.as_ref().map_or(&auth, |handoff| &handoff.auth);
        let tools = toolchain(&event_app)?;
        let result = audio_engine::inspect_streaming(&tools, &url, auth, &cancel, |update| {
            let _ = event_app.emit(
                "inspection-progress",
                InspectionEvent {
                    inspection_id: inspection_id.clone(),
                    update,
                },
            );
        });
        if result.is_ok() {
            if let Some(handoff) = &handoff {
                handoff
                    .persist(
                        &session_root(&event_app).map_err(audio_engine::EngineError::Invalid)?,
                        &auth_session::Keychain,
                    )
                    .map_err(audio_engine::EngineError::Invalid)?;
            }
        }
        result
    })
    .await;
    if let Ok(mut active) = app.state::<Jobs>().active.lock() {
        active.remove("inspection");
    }
    result
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn cancel_export(job_id: String, jobs: State<Jobs>) -> Result<(), String> {
    let jobs = jobs.active.lock().map_err(|error| error.to_string())?;
    jobs.get(&job_id)
        .ok_or("This job is no longer running")?
        .cancel();
    Ok(())
}

#[tauri::command]
fn start_export(
    app: tauri::AppHandle,
    plan: ExportPlan,
    output_path: String,
    job_id: String,
    review_identity: Option<reviews::CompletionIdentity>,
    jobs: State<Jobs>,
) -> Result<(), String> {
    if let Some(identity) = &review_identity {
        identity.validate()?;
    }
    let mut plan = plan;
    plan.auth = browser_sessions::resolve(&browser_locations(&app)?, &plan.auth)?;
    let output = PathBuf::from(output_path);
    if !output.is_absolute()
        || (plan.output_kind == audio_engine::OutputKind::Zip
            && output.extension().and_then(|s| s.to_str()) != Some("zip"))
    {
        return Err("Choose an available destination using the save dialog.".into());
    }
    if matches!(plan.artwork_mode, audio_engine::ArtworkMode::Shared) {
        if let Some(id) = &plan.shared_artwork_asset {
            plan.shared_artwork_path = Some(artwork_assets::resolve(
                &review_root(&app)?.join("artwork"),
                id,
            )?);
        }
    }
    if !recovery::valid_job_id(&job_id) {
        return Err("Invalid job identifier".into());
    }
    let tools = toolchain(&app).map_err(|error| error.to_string())?;
    let workdir =
        recovery::job_directory(&jobs_root(&app)?, &job_id).map_err(|error| error.to_string())?;
    let mut record = recovery::Recovery::new(
        job_id.clone(),
        plan.clone(),
        output.to_string_lossy().into_owned(),
    );
    record.review_identity = review_identity;
    let cancel = Cancellation::default();
    {
        let mut active = jobs.active.lock().map_err(|error| error.to_string())?;
        if !active.is_empty() {
            return Err("Wait for the current export to finish.".into());
        }
        record.save(&workdir).map_err(|error| error.to_string())?;
        active.insert(job_id.clone(), cancel.clone());
    }
    tauri::async_runtime::spawn_blocking(move || {
        // Hold through final recovery/cache writes so Disconnect cannot race them.
        let control = app.state::<auth_session::Control>();
        let _guard = control
            .serial
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let outcome = (|| -> audio_engine::Result<ExportResult> {
            cancel.check()?;
            let handoff = auth_session::materialize(
                &session_root(&app).map_err(audio_engine::EngineError::Invalid)?,
                &auth_session::Keychain,
                &plan.auth,
            )
            .map_err(audio_engine::EngineError::Invalid)?;
            let mut runtime_plan = plan.clone();
            if let Some(handoff) = &handoff {
                runtime_plan.auth = handoff.auth.clone();
            }
            std::fs::create_dir_all(&workdir)?;
            let result = audio_engine::export_collection(
                &tools,
                &runtime_plan,
                &output,
                &workdir,
                &cancel,
                |progress| {
                    record.progress = progress.clone();
                    let _ = record.save(&workdir);
                    let _ = app.emit(
                        "export-progress",
                        JobEvent {
                            job_id: job_id.clone(),
                            progress,
                            result: None,
                            error: None,
                        },
                    );
                },
            );
            if result.is_ok() {
                if let Some(handoff) = &handoff {
                    // The archive already exists; do not report an export failure
                    // solely because refreshed cookies could not be retained.
                    let _ = handoff.persist(
                        &session_root(&app).map_err(audio_engine::EngineError::Invalid)?,
                        &auth_session::Keychain,
                    );
                }
            }
            result
        })();
        let (stage, result, error) = match outcome {
            Ok(mut result) => {
                match review_root(&app) {
                    Ok(root) => export_completion::finish(
                        &app.state::<reviews::Store>(),
                        &root.join("draft"),
                        &workdir,
                        &mut record,
                        &mut result,
                    ),
                    Err(_) => {
                        record.progress.stage = "complete".into();
                        record.progress.completed = plan.tracks.len();
                        record.output_path = result.output_path.clone();
                        let _ = record.save(&workdir);
                        result.warnings.push("Your files were saved, but Jacket could not clear the unfinished review. Cleanup will be retried when Jacket opens again.".into());
                    }
                }
                if let Ok(mut exported) = app.state::<Jobs>().exported.lock() {
                    exported.insert(PathBuf::from(&result.output_path));
                }
                ("complete", Some(result), None)
            }
            Err(audio_engine::EngineError::Cancelled) => (
                "cancelled",
                None,
                Some("Cancelled. You can retry using the already downloaded files.".into()),
            ),
            Err(error) => ("failed", None, Some(error.to_string())),
        };
        if stage != "complete" {
            record.progress.stage = stage.into();
            record.error = error.clone();
            let _ = record.save(&workdir);
        }
        // Remove the active job before emitting completion so an immediate
        // retry cannot race with the worker's cleanup.
        if let Ok(mut active) = app.state::<Jobs>().active.lock() {
            active.remove(&job_id);
        }
        let _ = app.emit(
            "export-progress",
            JobEvent {
                job_id,
                progress: ProgressEvent {
                    stage: stage.into(),
                    message: if stage == "complete" {
                        if plan.output_kind == audio_engine::OutputKind::Folder {
                            "Your music folder is ready".into()
                        } else {
                            "Your archive is ready".into()
                        }
                    } else {
                        "Export stopped".into()
                    },
                    completed: record.progress.completed,
                    total: plan.tracks.len(),
                },
                result,
                error,
            },
        );
    });
    Ok(())
}

pub fn run() {
    let builder =
        tauri::Builder::default().plugin(tauri_plugin_single_instance::init(|app, _, _| {
            shell::activate(app)
        }));
    #[cfg(target_os = "linux")]
    let builder = builder.plugin(linux_graphics::init());
    builder
        .plugin(tauri_plugin_dialog::init())
        .manage(Jobs::default())
        .manage(UpdateControl::default())
        .manage(shell::Shell::default())
        .manage(auth_session::Control::default())
        .manage(reviews::Store::default())
        .setup(|app| {
            if let (Ok(root), Ok(jobs)) = (review_root(app.handle()), jobs_root(app.handle())) {
                let _ = export_completion::reconcile(
                    &app.state::<reviews::Store>(),
                    &root.join("draft"),
                    &jobs,
                );
            }
            // Before the frontend can import, retain every draft/job asset.
            // An unreadable inventory leaves all assets intact.
            let _ = prune_review_artwork(app.handle());
            // A failed cleanup is surfaced on session access, never blocks public use.
            if let Ok(root) = session_root(app.handle()) {
                let _ = auth_session::startup(&root);
            }
            Ok(shell::setup(app)?)
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                shell::close_requested(window, api);
            }
        })
        .invoke_handler(tauri::generate_handler![
            shell::take_menu_request,
            shell::workspace_status,
            dependency_status,
            session_status,
            session_action,
            browser_profiles,
            test_browser_access,
            inspect_source,
            start_export,
            cancel_export,
            reveal_archive,
            archive_destination,
            pending_export,
            dismiss_recovery,
            review_draft_load,
            review_draft_save,
            review_draft_discard,
            import_artwork,
            crop_artwork,
            artwork_asset,
            preview_export,
            folder_destination,
            open_export_playlist,
            check_downloader_update,
            update_downloader,
            restore_downloader,
            configure_downloader,
            downloader_selection_path,
            cancel_downloader_update
        ])
        .build(tauri::generate_context!())
        .expect("Unable to start Jacket")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                shell::activate(app);
            }
            if let tauri::RunEvent::Exit = event {
                if let Ok(running) = app.state::<UpdateControl>().cancel.lock() {
                    if let Some(cancel) = running.as_ref() {
                        cancel.cancel();
                    }
                }
                if let Ok(jobs) = app.state::<Jobs>().active.lock() {
                    for cancel in jobs.values() {
                        cancel.cancel();
                    }
                }
            }
        });
}
