# Native port and qualification

The native branch replaces the React/Tauri application with a Rust and iced 0.14
app. It follows Resonant v2's separation of application UI, review/domain logic,
native services and reusable audio engine, while preserving Jacket's GPL license.
The original implementation remains available in `src/` and `src-tauri/` as a
migration reference. It is not a member of the native Cargo workspace.

## Development and candidates

Use the [README](../README.md) and [contribution guide](../CONTRIBUTING.md) for the
current setup and tasks. The default `dev`, `desktop`, `build`, `check`, `lint` and
`test` tasks are native. `legacy:preview` and `legacy:test` are explicit comparison
tools and require npm. `mise run example` provides fictional tracks for visual
review; it must not export example downloads.

The `jacket` binary is in `target/debug` or `target/release`. Local candidates are
created by `mise run package:mac` or `mise run package:linux`. Existing candidate
directories are preserved: move or remove a generated candidate deliberately
before rebuilding it. An unchanged `target/native-dependencies` inventory is reused
after verifying its target, Cargo.lock, supplemental-notice identity and every retained file hash. A
stale or modified inventory is preserved with instructions to move or remove it
before recollecting. A staging failure or runtime manifest mismatch stops the
package operation. Packages do not silently substitute host audio tools.

macOS packaging produces an ad-hoc signed `Jacket.app` for local testing on Apple
Silicon macOS 26 or later. It is not notarized or signed with a distribution identity.
Linux packaging produces a portable `jacket-linux/` directory and a `.tar.gz`,
rather than the old Tauri DEB/RPM installers. Run `bin/jacket` inside a graphical
session. System Python 3, CA certificates and native windowing/display drivers
remain prerequisites. `com.moonlark.audio-dl.desktop` is an installation template; adjust its
Exec/Icon paths or install the files into standard locations before using it.

The source archive beside the candidate snapshots the native build inputs,
including an uncommitted local port. It is not based on `git archive HEAD`.
Exact resolved Cargo dependency sources/notices, the Geist OFL license, Linux
runtime sources and checksums accompany the candidate. `mise run licenses:native`
generates the target-specific Cargo inventory; collection fails on missing or
changed notices. Packaging verifies the inventory against the current Cargo.lock
and target. Before a public release, review the final binary closure and complete
corresponding source for all distributed GPL/LGPL components, including any macOS
Homebrew runtime components. Candidate generation is not publication.

## Migration and local data

The application identifier remains `com.moonlark.audio-dl`. On macOS, app data is
under `~/Library/Application Support/com.moonlark.audio-dl` and cache under
`~/Library/Caches/com.moonlark.audio-dl`. On Linux, XDG data/cache roots are used,
typically `~/.local/share/com.moonlark.audio-dl` and
`~/.cache/com.moonlark.audio-dl`. The native path service is the source of truth
when environment overrides are present.

Versioned Rust review/recovery JSON and opaque artwork IDs retain their format.
The native app does not import the webview's localStorage theme and other local
preferences. A restored unfinished review starts with public access; explicit
interrupted jobs retain the browser selection in their plan. Cookie contents and
resolved browser database paths are not serialized.

Before switching a real working library, retain a backup of local unfinished work
and exercise migration with a disposable data directory. Do not run legacy and
native apps against the same data/cache directories concurrently. The native app
holds an OS advisory lease for its data directory, preventing a second native
instance from editing it. The legacy app does not participate in that lease. `JACKET_DATA_DIR`,
`JACKET_CACHE_DIR` and `JACKET_RESOURCE_DIR` are native test/development overrides.
They should point to separate disposable directories for qualification.

## Native interface qualification

`mise run qualify:ui` captures the fictional review, export preview, settings,
track editor, bulk editor and artwork crop in light/dark themes at 600 × 600 and 1120 × 820.
Six additional captures cover single-song, empty and 10,000-occurrence list
scenarios in both themes. The images are written under `target/native-snapshots`. This requires a graphical
session (or an explicitly configured virtual display on Linux). Inspect the images;
a successfully written PNG alone does not prove layout or accessibility acceptance.
For one capture, use `mise run snapshot`; the binary also accepts
`--screenshot /absolute/path.png`, `--panel preview|settings|editor|bulk|crop`,
`--compact`, and `--dark`. `--song`, `--empty` and `--stress` provide isolated
single-song, welcome, and 10,000-occurrence list scenarios. These modes use fictional data, keep real saved reviews,
and disable downloads.

The native interface supports JPEG/PNG file selection or dropping an image onto
a review, source thumbnail previews, and a draggable square crop with a size
slider and exact numeric bounds. The native app uses the platform menus supplied
by winit. On macOS, Quit is routed through the same save-and-cancel close pipeline as closing the window.
App-specific menu commands remain future work.

Shortcuts are Ctrl/Cmd+L (source), Ctrl/Cmd+S (preview), Ctrl/Cmd+, (settings), and
Escape (close panel or cancel work). Ctrl/Cmd+Q and Ctrl/Cmd+W use the protected
close pipeline. Single-instance ownership is native; verify
second-launch and focus behavior on each target platform. A second native launch
currently reports that Jacket is already running; it does not activate the first
window. Cancelling a downloader update stops publication, although closing may
wait for a bounded HTTP request already in progress (up to 90 seconds).

## Acceptance record

See the [November 3 native verification record](native-verification.md) for the
completed Linux builds, tests and visual checks and their practical limits.

Automated check results belong in the implementation handoff or PR and should
identify the tested commit/environment. This checklist describes required desktop
qualification; unchecked behavior must not be represented as verified.

| Area | Evidence required before native release |
| --- | --- |
| Build and startup | Clean checkout, locked dependencies, ordinary launch, missing-tool guidance, complete bundled runtime. |
| Visual layout | Minimum and normal window sizes, long metadata, large playlists, light/dark theme, display scaling. |
| Keyboard and assistive access | Tab order, visible focus, text editing, dialog cancellation, shortcuts and screen-reader review. |
| Source intent | Song link, mixed song/playlist link, album and playlist, invalid URL, repeated entries and unavailable media. |
| Metadata and artwork | Individual/bulk edits, shared album fields, original metadata preservation, artwork import and export. |
| Output | Single audio file, music folder and ZIP; each format; preview agreement, collision handling and portable playlist. |
| Cancellation | Inspection, download, conversion, destination dialog and app close; no orphan tools or partial output. |
| Recovery | Restart after inspection/edit/export interruption, retry cached media, discard, successful completion and failed cleanup. |
| Account access | Explicit browser/profile, missing profile, locked database, expiry, private-link test and permission guidance. |
| Downloader | Managed/local selection, failed update, cancellation, idle activation and running-job executable stability. |
| Native shell | Platform menus, close/reopen, second instance, focus, file reveal and app identity/icon. |
| Distribution | Signing/notarization, notices, exact corresponding source, installed-runtime smoke and upgrade/uninstall. |

CI compiles and tests device-independent behavior on Linux and macOS. It does not
replace graphical, native-dialog, live-source or account testing. Windows, Intel
Macs and older macOS versions need separate build/runtime qualification. Old Tauri
screenshots and installer tests are historical evidence and do not qualify iced.

The automatic Tauri release workflow is replaced with manual native candidate
builds. The retained published-release verifier is manual and applies only to old
Tauri releases. No workflow on this branch automatically publishes native binaries.
