use crate::{EngineError, Result};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{ChildStdout, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default)]
pub struct Cancellation(Arc<CancellationState>);

#[derive(Debug, Default)]
struct CancellationState {
    cancelled: AtomicBool,
    process: Mutex<Option<u32>>,
}

fn stop_process_tree(pid: u32) {
    #[cfg(unix)]
    // Each tool starts in its own process group. Kill the whole group so
    // yt-dlp's ffmpeg children cannot outlive cancellation or keep pipes open.
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

struct RunningProcess<'a>(&'a Cancellation);

impl Drop for RunningProcess<'_> {
    fn drop(&mut self) {
        if let Ok(mut process) = self.0 .0.process.lock() {
            *process = None;
        }
    }
}

impl Cancellation {
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Relaxed);
        if let Ok(process) = self.0.process.lock() {
            if let Some(pid) = *process {
                stop_process_tree(pid);
            }
        }
    }
    pub fn check(&self) -> Result<()> {
        if self.0.cancelled.load(Ordering::Relaxed) {
            Err(EngineError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug)]
pub struct Toolchain {
    pub yt_dlp: PathBuf,
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub node: Option<PathBuf>,
}

pub fn find_tool(name: &str) -> Option<PathBuf> {
    let filename = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    let mut directories: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|value| std::env::split_paths(&value).collect())
        .unwrap_or_default();
    directories.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ]);
    if let Some(home) = std::env::var_os("HOME") {
        directories.push(PathBuf::from(home).join(".local/bin"));
    }
    directories
        .into_iter()
        .map(|dir| dir.join(&filename))
        .find(|path| path.is_file())
}

impl Toolchain {
    /// Prefer an app's complete staged runtime. A partial bundle is an error;
    /// mixing bundled and host tools hides packaging failures and breaks clean installs.
    pub fn discover_with_resources(resource_dir: &Path) -> Result<Self> {
        let bundled = resource_dir.join("audio-tools");
        if !bundled.exists() {
            return Self::discover();
        }
        Self::from_bundled_directory(&bundled)
    }

    fn from_bundled_directory(directory: &Path) -> Result<Self> {
        let required = |name: &str| -> Result<PathBuf> {
            let path = directory.join("bin").join(name);
            if !path.is_file() {
                return Err(EngineError::Invalid(format!(
                    "The app's bundled {name} is missing. Reinstall Jacket."
                )));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if path.metadata()?.permissions().mode() & 0o111 == 0 {
                    return Err(EngineError::Invalid(format!(
                        "The app's bundled {name} is not executable. Reinstall Jacket."
                    )));
                }
            }
            Ok(path)
        };
        Ok(Self {
            yt_dlp: required("yt-dlp")?,
            ffmpeg: required("ffmpeg")?,
            ffprobe: required("ffprobe")?,
            node: Some(required("node")?),
        })
    }

    pub fn discover() -> Result<Self> {
        let required = |name| {
            find_tool(name).ok_or_else(|| EngineError::Invalid(format!("{name} is not installed. This development build uses locally installed audio tools.")))
        };
        Ok(Self {
            yt_dlp: required("yt-dlp")?,
            ffmpeg: required("ffmpeg")?,
            ffprobe: required("ffprobe")?,
            node: find_tool("node"),
        })
    }
}

// Drain both pipes concurrently so a verbose child cannot deadlock the worker.
// Bound captured output: a malformed or unexpectedly large playlist must not
// exhaust the desktop application's memory.
fn capture(mut pipe: impl Read) -> std::io::Result<Vec<u8>> {
    let mut kept = Vec::new();
    let mut chunk = [0; 8192];
    let mut overflow = false;
    loop {
        let count = pipe.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        if kept.len() + count <= 32 * 1024 * 1024 {
            kept.write_all(&chunk[..count])?;
        } else {
            overflow = true;
        }
    }
    if overflow {
        Err(std::io::Error::other(
            "Tool output exceeded 32 MiB; use a smaller playlist",
        ))
    } else {
        Ok(kept)
    }
}

pub fn run(command: &mut Command, cancel: &Cancellation, timeout: Duration) -> Result<Vec<u8>> {
    run_with_stdout(command, cancel, timeout, |stdout| Ok(capture(stdout)?))
}

/// Deliver complete output lines while the child is still running. Keep the
/// same capture bound, stderr draining, timeout and process-tree cancellation
/// as ordinary commands. On parser failure, drain stdout to avoid deadlock.
pub fn run_lines(
    command: &mut Command,
    cancel: &Cancellation,
    timeout: Duration,
    mut on_line: impl FnMut(&[u8]) -> Result<()> + Send,
) -> Result<()> {
    run_with_stdout(command, cancel, timeout, move |mut stdout| {
        let outcome = (|| {
            let mut reader = BufReader::new((&mut stdout).take(32 * 1024 * 1024 + 1));
            let mut consumed = 0;
            let mut line = Vec::new();
            loop {
                line.clear();
                let count = reader.read_until(b'\n', &mut line)?;
                if count == 0 {
                    break;
                }
                consumed += count;
                if consumed > 32 * 1024 * 1024 {
                    return Err(EngineError::Invalid(
                        "Tool output exceeded 32 MiB; use a smaller playlist".into(),
                    ));
                }
                if !line.iter().all(u8::is_ascii_whitespace) {
                    on_line(&line)?;
                }
            }
            Ok(())
        })();
        if outcome.is_err() {
            std::io::copy(&mut stdout, &mut std::io::sink())?;
        }
        outcome
    })
}

fn run_with_stdout<T: Send>(
    command: &mut Command,
    cancel: &Cancellation,
    timeout: Duration,
    read_stdout: impl FnOnce(ChildStdout) -> Result<T> + Send,
) -> Result<T> {
    cancel.check()?;
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "linux")]
    let mut busy_retries = 0;
    let mut child = loop {
        cancel.check()?;
        match command.spawn() {
            Ok(child) => break child,
            Err(error) => {
                // Another thread's concurrent fork can briefly inherit a just-
                // closed executable writer until its own exec closes CLOEXEC
                // descriptors. No child has started when exec returns ETXTBSY.
                #[cfg(target_os = "linux")]
                if error.raw_os_error() == Some(libc::ETXTBSY) && busy_retries < 20 {
                    busy_retries += 1;
                    thread::sleep(Duration::from_millis(25));
                    continue;
                }
                return Err(error.into());
            }
        }
    };
    let _registration = RunningProcess(cancel);
    if let Ok(mut process) = cancel.0.process.lock() {
        *process = Some(child.id());
        if cancel.check().is_err() {
            stop_process_tree(child.id());
        }
    }
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    thread::scope(|scope| {
        let out_reader = scope.spawn(move || read_stdout(stdout));
        let err_reader = scope.spawn(move || capture(stderr));
        let started = Instant::now();
        let mut interrupted = None;
        let status = loop {
            if let Err(error) = cancel.check() {
                interrupted = Some(error);
            }
            if started.elapsed() > timeout {
                interrupted = Some(EngineError::Invalid(
                    "Audio tool timed out; retry this item.".into(),
                ));
            }
            if interrupted.is_some() {
                stop_process_tree(child.id());
                let _ = child.kill();
                break child.wait()?;
            }
            if let Some(status) = child.try_wait()? {
                break status;
            }
            thread::sleep(Duration::from_millis(100));
        };
        let stdout = out_reader
            .join()
            .map_err(|_| EngineError::Invalid("Output reader stopped".into()))?;
        let stderr = err_reader
            .join()
            .map_err(|_| EngineError::Invalid("Error reader stopped".into()))??;
        if let Some(error) = interrupted {
            return Err(error);
        }
        cancel.check()?;
        // yt-dlp can fall back to guest access after warning about a rotated
        // session. An explicitly authenticated operation must fail closed.
        let authenticated = command
            .get_args()
            .any(|arg| arg == "--cookies" || arg == "--cookies-from-browser");
        if authenticated {
            if let Some(problem) = crate::access::problem(&String::from_utf8_lossy(&stderr)) {
                // Decryption and expiry can be warnings on an otherwise
                // successful guest response. Do not claim this session worked.
                if !status.success()
                    || matches!(
                        problem.code,
                        "expired" | "decryption" | "permission" | "locked"
                    )
                {
                    return Err(EngineError::Invalid(format!(
                        "{} {}",
                        problem.message, problem.guidance
                    )));
                }
            }
        }
        if !status.success() {
            let detail = String::from_utf8_lossy(&stderr);
            let last_lines = detail
                .lines()
                .rev()
                .take(8)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n");
            return Err(EngineError::Tool(last_lines));
        }
        stdout
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "linux")]
    fn a_busy_executable_can_start_after_its_writer_closes() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let executable = root.path().join("tool");
        std::fs::write(&executable, "#!/bin/sh\nprintf ready").unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        let writer = std::fs::OpenOptions::new()
            .write(true)
            .open(&executable)
            .unwrap();
        let closer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(100));
            drop(writer);
        });
        let result = run(
            &mut Command::new(&executable),
            &Cancellation::default(),
            Duration::from_secs(2),
        );
        closer.join().unwrap();
        assert_eq!(result.unwrap(), b"ready");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn cancellation_interrupts_busy_executable_retries() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let executable = root.path().join("tool");
        std::fs::write(&executable, "#!/bin/sh\nexit 0").unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _writer = std::fs::OpenOptions::new()
            .write(true)
            .open(&executable)
            .unwrap();
        let cancel = Cancellation::default();
        let trigger = cancel.clone();
        let canceller = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            trigger.cancel();
        });
        let result = run(
            &mut Command::new(&executable),
            &cancel,
            Duration::from_secs(2),
        );
        canceller.join().unwrap();
        assert!(matches!(result, Err(EngineError::Cancelled)));
    }

    #[test]
    fn expired_cookie_warning_cannot_succeed_as_an_anonymous_download() {
        let outcome = run(Command::new("sh").args(["-c", "echo 'WARNING: YouTube account cookies are no longer valid' >&2; echo public-result", "fixture", "--cookies", "fixture.txt"]), &Cancellation::default(), Duration::from_secs(2));
        assert!(
            matches!(outcome, Err(EngineError::Invalid(message)) if message.contains("expired"))
        );
    }

    #[test]
    fn bundled_discovery_uses_only_the_complete_app_runtime() {
        use std::os::unix::fs::PermissionsExt;
        let resources = tempfile::tempdir().unwrap();
        let bin = resources.path().join("audio-tools/bin");
        std::fs::create_dir_all(&bin).unwrap();
        for name in ["yt-dlp", "ffmpeg", "ffprobe", "node"] {
            let path = bin.join(name);
            std::fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let tools = Toolchain::discover_with_resources(resources.path()).unwrap();
        assert_eq!(tools.yt_dlp, bin.join("yt-dlp"));
        assert_eq!(tools.ffmpeg, bin.join("ffmpeg"));
        assert_eq!(tools.ffprobe, bin.join("ffprobe"));
        assert_eq!(tools.node, Some(bin.join("node")));
        std::fs::remove_file(bin.join("ffprobe")).unwrap();
        assert!(
            matches!(Toolchain::discover_with_resources(resources.path()), Err(EngineError::Invalid(message)) if message.contains("bundled ffprobe"))
        );
    }

    #[test]
    fn bundled_discovery_rejects_a_nonexecutable_tool() {
        let resources = tempfile::tempdir().unwrap();
        let bin = resources.path().join("audio-tools/bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("yt-dlp"), "not executable").unwrap();
        assert!(
            matches!(Toolchain::discover_with_resources(resources.path()), Err(EngineError::Invalid(message)) if message.contains("not executable"))
        );
    }

    #[test]
    fn streaming_delivers_a_line_before_exit_and_can_cancel_from_callback() {
        let cancel = Cancellation::default();
        let started = Instant::now();
        let mut lines = Vec::new();
        let outcome = run_lines(
            Command::new("sh").args(["-c", "printf 'first\\n'; sleep 30 & wait"]),
            &cancel,
            Duration::from_secs(3),
            |line| {
                lines.push(line.to_vec());
                cancel.cancel();
                Ok(())
            },
        );
        assert_eq!(lines, [b"first\n".to_vec()]);
        assert!(matches!(outcome, Err(EngineError::Cancelled)));
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn streaming_parser_failure_drains_remaining_output() {
        let outcome = run_lines(
            Command::new("sh").args([
                "-c",
                "printf 'invalid\\n'; dd if=/dev/zero bs=1024 count=1024 2>/dev/null",
            ]),
            &Cancellation::default(),
            Duration::from_secs(3),
            |_| Err(EngineError::Invalid("Malformed metadata".into())),
        );
        assert!(
            matches!(outcome, Err(EngineError::Invalid(message)) if message == "Malformed metadata")
        );
    }

    #[test]
    fn streaming_bounds_an_unterminated_line() {
        let outcome = run_lines(
            Command::new("sh").args(["-c", "dd if=/dev/zero bs=1048576 count=34 2>/dev/null"]),
            &Cancellation::default(),
            Duration::from_secs(5),
            |_| panic!("Oversized line must not reach the parser"),
        );
        assert!(
            matches!(outcome, Err(EngineError::Invalid(message)) if message.contains("32 MiB"))
        );
    }

    #[test]
    fn cancellation_stops_descendants_that_hold_output_pipes() {
        let cancel = Cancellation::default();
        let worker_cancel = cancel.clone();
        let started = Instant::now();
        let worker = thread::spawn(move || {
            run(
                Command::new("sh").args(["-c", "sleep 30 & wait"]),
                &worker_cancel,
                Duration::from_secs(60),
            )
        });
        thread::sleep(Duration::from_millis(200));
        cancel.cancel();
        assert!(matches!(
            worker.join().unwrap(),
            Err(EngineError::Cancelled)
        ));
        assert!(started.elapsed() < Duration::from_secs(3));
    }
}
