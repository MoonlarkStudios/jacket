use serde::Serialize;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    Emitter, Manager,
};

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Request {
    New,
    Settings,
    Example,
    AuthPrototype,
}

#[derive(Default)]
pub struct Shell {
    pending: Mutex<VecDeque<Request>>,
    busy: AtomicBool,
}

fn show(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("Jacket window is unavailable")?;
    window.unminimize().map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

pub fn activate(app: &tauri::AppHandle) {
    let _ = show(app);
}

fn request(app: &tauri::AppHandle, request: Request) {
    let shell = app.state::<Shell>();
    if let Ok(mut pending) = shell.pending.lock() {
        pending.push_back(request);
    }
    activate(app);
    // Retain requests made before the webview attaches its listener.
    let _ = app.emit_to("main", "jacket-menu-request", ());
}

#[tauri::command]
pub fn take_menu_request(shell: tauri::State<Shell>) -> Result<Option<Request>, String> {
    Ok(shell.pending.lock().map_err(|e| e.to_string())?.pop_front())
}

#[tauri::command]
pub fn workspace_status(app: tauri::AppHandle, busy: bool) {
    app.state::<Shell>().busy.store(busy, Ordering::Relaxed);
}

#[derive(Debug, PartialEq)]
enum CloseAction {
    Hide,
    Minimize,
    Close,
}

fn close_action(macos: bool, busy: bool) -> CloseAction {
    if macos {
        CloseAction::Hide
    } else if busy {
        CloseAction::Minimize
    } else {
        CloseAction::Close
    }
}

pub fn close_requested(window: &tauri::Window, api: &tauri::CloseRequestApi) {
    let app = window.app_handle();
    // Check native jobs as well as the UI state: a job can start just before
    // the frontend publishes its busy state.
    let native_busy = app
        .state::<crate::Jobs>()
        .active
        .lock()
        .map_or(true, |jobs| !jobs.is_empty());
    let busy = native_busy || app.state::<Shell>().busy.load(Ordering::Relaxed);
    match close_action(cfg!(target_os = "macos"), busy) {
        CloseAction::Hide => {
            api.prevent_close();
            let _ = window.hide();
        }
        CloseAction::Minimize => {
            api.prevent_close();
            let _ = window.minimize();
        }
        CloseAction::Close => {}
    }
}

fn handle_menu(app: &tauri::AppHandle, id: &str) {
    match id {
        "new" => request(app, Request::New),
        "open" => activate(app),
        "settings" => request(app, Request::Settings),
        "example" => request(app, Request::Example),
        "auth-prototype"
            if cfg!(debug_assertions)
                && std::env::var("JACKET_AUTH_PROTOTYPE").as_deref() == Ok("1") =>
        {
            request(app, Request::AuthPrototype)
        }
        "quit" => app.exit(0),
        _ => {}
    }
}

pub fn setup(app: &mut tauri::App) -> tauri::Result<()> {
    let new = MenuItem::with_id(app, "new", "New download", true, Some("CmdOrCtrl+N"))?;
    let open = MenuItem::with_id(app, "open", "Open Jacket", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, Some("CmdOrCtrl+,"))?;
    let quit = MenuItem::with_id(app, "quit", "Quit Jacket", true, Some("CmdOrCtrl+Q"))?;
    let actions = Submenu::with_items(
        app,
        "Jacket",
        true,
        &[
            &new,
            &open,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    if cfg!(debug_assertions) {
        actions.append(&MenuItem::with_id(
            app,
            "example",
            "Preview example",
            true,
            None::<&str>,
        )?)?;
        if std::env::var("JACKET_AUTH_PROTOTYPE").as_deref() == Ok("1") {
            actions.append(&MenuItem::with_id(
                app,
                "auth-prototype",
                "Developer session prototype…",
                true,
                None::<&str>,
            )?)?;
        }
    }
    // Preserve native text-editing shortcuts in the webview.
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    app.set_menu(Menu::with_items(app, &[&actions, &edit])?)?;
    app.on_menu_event(|app, event| handle_menu(app, event.id().as_ref()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_close_preserves_the_workspace_for_dock_reopening() {
        assert_eq!(close_action(true, false), CloseAction::Hide);
        assert_eq!(close_action(true, true), CloseAction::Hide);
    }

    #[test]
    fn windows_and_linux_close_normally_unless_an_operation_is_running() {
        assert_eq!(close_action(false, false), CloseAction::Close);
        assert_eq!(close_action(false, true), CloseAction::Minimize);
    }
}
