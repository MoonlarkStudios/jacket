//! Local folder and save dialogs for export destinations.
#[cfg(not(target_os = "linux"))]
use std::path::PathBuf;

pub(super) enum Selection {
    Folder,
    File(String),
}

#[cfg(not(target_os = "linux"))]
pub(super) async fn pick(
    title: &'static str,
    directory: Option<PathBuf>,
    selection: Selection,
) -> Result<Option<PathBuf>, String> {
    let mut dialog = rfd::AsyncFileDialog::new().set_title(title);
    if let Some(directory) = directory {
        dialog = dialog.set_directory(directory);
    }
    let chosen = match selection {
        Selection::Folder => dialog.pick_folder().await,
        Selection::File(name) => dialog.set_file_name(name).save_file().await,
    };
    Ok(chosen.map(|file| file.path().to_path_buf()))
}

#[cfg(target_os = "linux")]
pub(super) use linux::pick;

#[cfg(target_os = "linux")]
mod linux {
    use super::Selection;
    use std::ffi::OsString;
    use std::io;
    use std::os::unix::ffi::OsStringExt;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};

    const CANCEL_EXIT_STATUS: i32 = 10;

    pub(in super::super) async fn pick(
        title: &'static str,
        directory: Option<PathBuf>,
        selection: Selection,
    ) -> Result<Option<PathBuf>, String> {
        // Jacket's Linux packages install a native GTK picker. Opening it
        // directly avoids a portal request waiting forever after a backend
        // disconnects, without imposing a deadline on a person's selection.
        let result = tokio::task::spawn_blocking(move || {
            zenity_picker(title, directory.as_deref(), &selection)
        })
        .await
        .map_err(|_| "The system file chooser stopped unexpectedly.".to_owned())?;
        result.map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                "Couldn’t open the system file chooser. Install Zenity and try again.".into()
            } else {
                "Couldn’t open the system file chooser. Try again.".into()
            }
        })
    }

    fn zenity_picker(
        title: &str,
        directory: Option<&Path>,
        selection: &Selection,
    ) -> io::Result<Option<PathBuf>> {
        let mut command = Command::new("zenity");
        // Use the GTK chooser directly across GTK 3 and 4.
        command
            .env("GTK_USE_PORTAL", "0")
            .env("GDK_DEBUG", "no-portals")
            // GTK can exit with Zenity's default cancellation status before
            // opening a dialog. Give user dismissal its own child exit code.
            .env("ZENITY_OK", "0")
            .env("ZENITY_CANCEL", CANCEL_EXIT_STATUS.to_string())
            .env("ZENITY_ESC", CANCEL_EXIT_STATUS.to_string())
            .env("ZENITY_ERROR", "255")
            .args(["--file-selection", "--title", title]);
        let filename = match selection {
            Selection::Folder => {
                command.arg("--directory");
                directory.map(|folder| folder.join(""))
            }
            Selection::File(name) => {
                command.arg("--save");
                Some(directory.map_or_else(|| PathBuf::from(name), |folder| folder.join(name)))
            }
        };
        if let Some(filename) = filename {
            let mut argument = OsString::from("--filename=");
            argument.push(filename);
            command.arg(argument);
        }
        zenity_response(command.output()?)
    }

    fn zenity_response(output: Output) -> io::Result<Option<PathBuf>> {
        if output.status.code() == Some(CANCEL_EXIT_STATUS) && output.stdout.is_empty() {
            return Ok(None);
        }
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "The system file chooser exited with {}.",
                output.status
            )));
        }
        let mut path = output.stdout;
        if path.last() == Some(&b'\n') {
            path.pop();
        }
        let path = PathBuf::from(OsString::from_vec(path));
        if !path.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "The system file chooser did not return an absolute path.",
            ));
        }
        Ok(Some(path))
    }

    #[cfg(test)]
    mod tests {
        use super::{zenity_response, CANCEL_EXIT_STATUS};
        use std::os::unix::process::ExitStatusExt;
        use std::path::Path;
        use std::process::{ExitStatus, Output};

        fn output(status: i32, stdout: &[u8], stderr: &[u8]) -> Output {
            Output {
                status: ExitStatus::from_raw(status << 8),
                stdout: stdout.to_vec(),
                stderr: stderr.to_vec(),
            }
        }

        #[test]
        fn native_picker_preserves_the_selected_path() {
            assert_eq!(
                zenity_response(output(0, b"/tmp/music \n", b""))
                    .unwrap()
                    .as_deref(),
                Some(Path::new("/tmp/music "))
            );
            assert!(zenity_response(output(0, b"relative-path\n", b"")).is_err());
        }

        #[test]
        fn native_picker_distinguishes_display_failure_and_user_dismissal() {
            let gtk_warning = b"GtkDialog mapped without a transient parent. This is discouraged.";
            assert_eq!(
                zenity_response(output(CANCEL_EXIT_STATUS, b"", gtk_warning)).unwrap(),
                None
            );
            assert!(
                zenity_response(output(1, b"", b"Gtk-WARNING: Failed to open display")).is_err()
            );
            assert!(zenity_response(output(1, b"", b"")).is_err());
            assert!(zenity_response(output(255, b"", b"")).is_err());
            assert!(zenity_response(output(CANCEL_EXIT_STATUS, b"unexpected", b"")).is_err());
        }
    }
}
