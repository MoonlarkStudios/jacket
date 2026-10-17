//! GTK3/WebKitGTK first-frame compatibility on NVIDIA Wayland desktops.

use gtk::{gdk::prelude::*, prelude::*};
use std::{ffi::OsString, fs, path::Path};
use tauri::plugin::{Builder, TauriPlugin};

fn rendering_overridden(mut get: impl FnMut(&str) -> Option<OsString>) -> bool {
    get("JACKET_NVIDIA_WAYLAND_WORKAROUND").as_deref() == Some("0".as_ref())
        || [
            "GDK_GL",
            "GDK_DEBUG",
            "__NV_DISABLE_EXPLICIT_SYNC",
            "WEBKIT_DISABLE_DMABUF_RENDERER",
            "WEBKIT_DISABLE_COMPOSITING_MODE",
            "WEBKIT_DMABUF_RENDERER_FORCE_SHM",
            "LIBGL_ALWAYS_SOFTWARE",
        ]
        .into_iter()
        .any(|name| get(name).is_some())
}

fn has_nvidia_driver(drm: &Path) -> bool {
    let Ok(entries) = fs::read_dir(drm) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let name = entry.file_name();
        let Some(index) = name.to_str().and_then(|name| name.strip_prefix("card")) else {
            return false;
        };
        if index.is_empty() || !index.bytes().all(|byte| byte.is_ascii_digit()) {
            return false;
        }
        let device = entry.path().join("device");
        fs::read_to_string(device.join("vendor")).is_ok_and(|vendor| vendor.trim() == "0x10de")
            && fs::read_link(device.join("driver"))
                .is_ok_and(|driver| driver.file_name().is_some_and(|name| name == "nvidia"))
    })
}

fn prepare_window(window: &gtk::Window) {
    let Some(native) = window.window() else {
        return;
    };
    // This public API also creates and realizes GDK's window-owned paint
    // context. Do it at realize time, before GTK chooses the first frame's
    // rendering path. Creating it during WebKit's first draw would enable
    // NVIDIA explicit sync midway through a shared-memory frame and cause a
    // fatal Wayland protocol error. The temporary application context can be
    // dropped; GDK retains the paint context for the window's lifetime.
    // Upstream fix: https://bugs.webkit.org/show_bug.cgi?id=324551
    match native.create_gl_context() {
        Ok(_) => eprintln!("Jacket: prepared GTK OpenGL painting for NVIDIA Wayland"),
        Err(error) => eprintln!(
            "Jacket: could not prepare GTK OpenGL painting: {error}. \
             Try launching with __NV_DISABLE_EXPLICIT_SYNC=1."
        ),
    }
}

pub fn init() -> TauriPlugin<tauri::Wry> {
    Builder::new("linux-graphics")
        .setup(|_, _| {
            // Inspect the actual GDK backend: a Wayland desktop can launch
            // Jacket through XWayland when GDK_BACKEND=x11 is requested.
            if rendering_overridden(|name| std::env::var_os(name))
                || !gtk::gdk::Display::default()
                    .is_some_and(|display| display.backend().is_wayland())
                || !has_nvidia_driver(Path::new("/sys/class/drm"))
            {
                return Ok(());
            }
            let application = gtk::gio::Application::default()
                .and_then(|application| application.downcast::<gtk::Application>().ok())
                .ok_or("Jacket's GTK application is unavailable")?;
            // Plugin setup precedes Tauri's configured windows. GTK announces
            // each window synchronously, so even later-created windows get a
            // realize handler before any frame can be painted.
            application.connect_window_added(|_, window| {
                window.connect_realize(prepare_window);
                if window.is_realized() {
                    prepare_window(window);
                }
            });
            Ok(())
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn only_an_nvidia_card_with_the_nvidia_driver_is_affected() {
        let root = tempfile::tempdir().unwrap();
        assert!(!has_nvidia_driver(&root.path().join("missing")));
        assert!(!has_nvidia_driver(root.path()));
        for (card, vendor, driver) in [
            ("card0", "0x1002", "amdgpu"),
            ("card1", "0x10de", "nouveau"),
            ("card2-DP-1", "0x10de", "nvidia"),
            ("renderD128", "0x10de", "nvidia"),
        ] {
            let device = root.path().join(card).join("device");
            fs::create_dir_all(&device).unwrap();
            fs::write(device.join("vendor"), format!("{vendor}\n")).unwrap();
            symlink(format!("../../../drivers/{driver}"), device.join("driver")).unwrap();
        }
        assert!(!has_nvidia_driver(root.path()));
        let device = root.path().join("card1/device");
        fs::remove_file(device.join("driver")).unwrap();
        symlink("../../../drivers/nvidia", device.join("driver")).unwrap();
        assert!(has_nvidia_driver(root.path()));
    }

    #[test]
    fn explicit_rendering_choices_and_the_opt_out_are_respected() {
        assert!(!rendering_overridden(|_| None));
        for name in [
            "GDK_GL",
            "GDK_DEBUG",
            "__NV_DISABLE_EXPLICIT_SYNC",
            "WEBKIT_DISABLE_DMABUF_RENDERER",
            "WEBKIT_DISABLE_COMPOSITING_MODE",
            "WEBKIT_DMABUF_RENDERER_FORCE_SHM",
            "LIBGL_ALWAYS_SOFTWARE",
        ] {
            assert!(rendering_overridden(|key| (key == name).then(|| "1".into())));
        }
        assert!(rendering_overridden(|key| {
            (key == "JACKET_NVIDIA_WAYLAND_WORKAROUND").then(|| "0".into())
        }));
        assert!(!rendering_overridden(|key| {
            (key == "JACKET_NVIDIA_WAYLAND_WORKAROUND").then(|| "1".into())
        }));
    }
}
