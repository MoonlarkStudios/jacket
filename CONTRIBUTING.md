# Contributing to Jacket

Jacket's native application follows the Rust and iced direction of Resonant v2.
Use mise for repository commands and the
[Google Rust Style Guide](https://google.github.io/styleguide/rustguide.html) for
Rust code. Apply its API guidance in the context of application code. Chromium-only
build, threading and flat-directory rules do not replace Cargo conventions.

## Development environment

Install mise, then run `mise trust`, `mise install`, `mise run setup`, and
`mise run dev`. `rust-toolchain.toml` pins the compiler, rustfmt and Clippy;
`mise.toml` pins the other tools and defines the quality gate. Run one-off commands
with `mise exec -- <command>`. Commit `Cargo.lock` with dependency changes and use
`--locked` for ordinary builds and checks.

On macOS, install Xcode Command Line Tools (`xcode-select --install`) and Homebrew.
The macOS runtime staging and local app target require Apple Silicon and macOS 26
or later. Public signing/notarization is not configured.

Ubuntu 24.04 build prerequisites:

```sh
sudo apt-get update
sudo apt-get install build-essential pkg-config libx11-dev libxkbcommon-dev \
  libwayland-dev libgl1-mesa-dev ffmpeg
```

Install yt-dlp using your trusted package manager. mise supplies Node, which yt-dlp
uses for JavaScript challenges. `mise run doctor` checks tool versions, windowing
headers and availability. Native source builds do not need GTK, WebKit or npm.
A graphical X11/Wayland session and suitable display drivers are required to launch
the app; the ordinary tests run without a display or live source account.

| Command | Purpose |
| --- | --- |
| `mise run dev` / `mise run desktop` | Launch the native iced app. |
| `mise run example` | Explore fictional tracks without downloading. |
| `mise run build` | Build the optimized `jacket` executable. |
| `mise run check` | Workflow/script checks, rustfmt, Clippy and all native unit tests. |
| `mise run check:workspace` | Compile-check all native targets. |
| `mise run test:media` | Offline exports with FFmpeg and an independent Mutagen reader. |
| `mise run fmt` | Format native Rust code. |
| `mise run snapshot` | Capture an isolated fictional review and quit; requires a display. |
| `mise run qualify:ui` | Capture 96 album, song, playlist and edge-case views across themes and sizes. |
| `mise run licenses:native` | Collect exact Cargo sources, native notices and an integrity manifest. |
| `mise run package:mac` | Build a local ad-hoc signed `.app` with staged tools. |
| `mise run package:linux` | Build a portable Linux candidate with pinned tools and source companion. |

The first dependency fetch and runtime staging need network access. Native unit
tests do not contact YouTube, access real browser databases or need an account.
Media integration tests synthesize their own recordings. The full native check
never installs the retained frontend dependencies. Comparison-only browser tasks
are explicitly named `legacy:*`; they are outside the native quality gate.

## Module boundaries and Rust conventions

| Path | Responsibility |
| --- | --- |
| `apps/jacket` | iced widgets, input handling, dialogs and native app coordination. |
| `crates/jacket-core` | Source intent, review state, metadata editing and export-plan construction. |
| `crates/jacket-desktop` | Local storage, recovery, browser profiles, artwork, downloader and supervised jobs. |
| `crates/audio-engine` | Inspection, subprocesses, codecs, tagging and transactional export publication. |
| `scripts/native_*` | Native prerequisite checks, packaging and offline script tests. |
| `src`, `src-tauri` | Retained legacy implementation and migration reference. |

Keep reusable behavior outside iced. No native workspace crate may depend on
Tauri or a webview. The dependency-boundary script checks the native manifests.
Keep crate roots as module declarations and explicit re-exports; implementations
belong in leaf modules named for their main responsibility. Use ordinary module
paths and explicit imports, avoiding production glob imports or `#[path]`
overrides. Prefer concrete, readable code over speculative abstraction.

Use `Result` for expected I/O and validation failures. Reserve panics for broken
invariants with useful `expect` messages. Never recover a poisoned mutex's inner
state. Give each admitted operation its own cancellation token; do not reset or
reuse it. Keep slow I/O and subprocess work off the UI thread, bound progress
queues and supervise terminal worker outcomes. Cancel and join owned workers
before teardown. Preserve transactional publication, output collision protection,
original source metadata, duplicate playlist occurrences and exact review identity
when reconciling successful exports.

Keep unit tests beside their modules and integration tests in each crate's
`tests/` directory. Add tests for behavior and meaningful failure modes, including
cancellation, corrupt persistence and stale completion. Do not use live accounts,
cookies or restricted media as fixtures. Avoid including credentials, private
URLs, recordings or export destinations in logs and reports.

## Native presentation and themes

`apps/jacket/src/view/` separates the shell, workspace, export controls, preview,
preferences and shared components. `theme.rs` resolves the versioned semantic
palette; `theme_iced.rs` supplies iced's extended palette, and `style.rs` turns
that resolved snapshot into widget rules. Use semantic roles for surfaces, text,
status and controls rather than hardcoded light/dark color branches. Album
artwork remains literal image content when the surrounding theme changes.

Theme changes are application settings. Preserve review identity, metadata,
selection, recovery and admitted jobs while switching themes. New preferences
default to Jacket Dark; migration retains an explicit old `dark: false` as Jacket
Light and `dark: true` as Jacket Dark. Custom TOML definitions are copied into the
application preference after validation. See [theme authoring](docs/themes.md).

## Validation and pull requests

Run `mise run check`. For audio/tagging/export changes, also run
`mise run test:media`. Native interface changes need a visual review and keyboard
check at 600 × 600 and the 1180 × 800 default size in all three presets. Check focus, resize, scrolling, dialog
cancellation, operation cancellation and closing while work is active. Record
which platform and display path were exercised; headless checks alone do not
establish desktop acceptance. Use the checklist in [native port](docs/native-port.md).

Keep feature tests, workspace membership, manifests, lockfile and mise changes in
the same independently buildable commit. Validate the exact staged tree; do not
rely on untracked files or later commits. Use semantic, single-line commit
subjects such as `feat(native): add cancellable track inspection`, without bodies
or trailers. See [AGENTS.md](AGENTS.md).

PR descriptions should explain the resulting behavior and relevant validation.
Public binary publication remains separate from candidate generation; complete
desktop acceptance, notices, corresponding source and signing before publishing.
The native branch intentionally does not run the automatic Tauri release path.

Contributions are licensed under [GPL-3.0-or-later](LICENSE). Retain third-party
licenses and source provenance. Report security issues through [SECURITY.md](SECURITY.md).
