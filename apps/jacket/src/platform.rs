//! Small native integrations that must run after the application window exists.

/// Make the standard macOS Quit action use Jacket's normal close workflow.
///
/// The close request lets the application finish persistence and stop its
/// current worker before iced destroys the window. Repeated calls are harmless.
#[cfg(target_os = "macos")]
pub fn window_ready() {
    use objc2::sel;
    use objc2_app_kit::NSApplication;
    use objc2_foundation::MainThreadMarker;

    let Some(marker) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(marker);

    // SAFETY: the marker proves this is AppKit's main thread. Every receiver is
    // a typed, retained object obtained from the live application menu, and the
    // arrays remain retained during iteration. Updating item actions does not
    // change either array. A nil target dispatches the standard performClose:
    // selector through AppKit's responder chain to the current native window.
    unsafe {
        let Some(main_menu) = app.mainMenu() else {
            return;
        };
        let menus = main_menu.itemArray();
        for menu_index in 0..menus.len() {
            let Some(menu_item) = menus.get(menu_index) else {
                continue;
            };
            let Some(menu) = menu_item.submenu() else {
                continue;
            };
            let entries = menu.itemArray();
            for entry_index in 0..entries.len() {
                let Some(entry) = entries.get(entry_index) else {
                    continue;
                };
                if entry.action() == Some(sel!(terminate:)) {
                    entry.setTarget(None);
                    entry.setAction(Some(sel!(performClose:)));
                }
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn window_ready() {}
