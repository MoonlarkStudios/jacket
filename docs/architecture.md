# Architecture

Start with the [user guide](usage.md) for product behavior and [CONTRIBUTING](../CONTRIBUTING.md) for setup and validation. This document records code boundaries and the invariants contributors should preserve.

## Components and data flow

```text
React + Mantine
  Review state, selection, metadata drafts, export preferences
       │ typed commands and progress events
Tauri desktop adapter
  Native dialogs, browser profiles, downloader updates, jobs, recovery
       │ Rust API
audio-engine
  Source inspection → export plan → tagged audio + assets → ZIP + manifest
       │ argument-based subprocesses and HTTPS image requests
yt-dlp / ffmpeg / ffprobe
```

| Boundary | Responsibilities |
| --- | --- |
| [`crates/audio-engine`](../crates/audio-engine/src/lib.rs) | Framework-independent inspection, metadata, codec selection, tagging, cancellable subprocesses, artwork, and archives. `Toolchain`, `Cancellation`, and progress callbacks keep it independent of Tauri. |
| [`src-tauri`](../src-tauri/src/lib.rs) | Native command handlers, save/reveal dialogs, job ownership, private storage, browser-profile resolution, and runtime discovery. |
| [`src`](../src/App.tsx) | Review UI and editing drafts. `domain.ts` assembles transport models/export plans; `desktop.ts` is the native transport boundary; `useWorkspace.ts` owns the workspace and composes browser-session and downloader state. |
| [`shell.rs`](../src-tauri/src/shell.rs), [`useDesktopShell.ts`](../src/useDesktopShell.ts) | Native menus, startup request delivery, window visibility, and single-instance activation. |

The webview does not spawn processes or read browser databases. The browser-only example uses fictional local data and cannot inspect real sources or export audio.

## Metadata and export invariants

- Preserve source order and repeated playlist entries. Provider identity is not a deduplication key.
- Retain original source metadata separately from user edits and the metadata actually embedded. The [manifest contract](manifest.md) defines the exported representation and compatibility rules.
- Unknown artists and release years remain unknown; uploader names and upload dates are not substitutes. Album-header enrichment is bounded and optional, so its failure does not fail inspection.
- Supplied shared album fields override individual album/album-artist/year values at export. Track artists and existing track/disc numbers remain intact. Missing album track numbers are filled before selection, so deselecting tracks leaves gaps.
- Inspection streams partial results without downloading audio. Editing and export wait for completion; failed or cancelled inspections keep a marked partial preview.
- Artwork failures become warnings and allow audio export to continue. Every selected audio item must be prepared before an archive is published.
- Write the ZIP beside its destination through a temporary file. Never overwrite an existing archive, including one created during the job; filename collisions receive a numbered suffix. Cancellation publishes no partial ZIP.

## Jobs and recovery

The native layer owns operation lifetimes. Browser-access tests and album jobs cannot overlap. Menu requests survive frontend startup, are processed serially, and cannot replace active operations or metadata drafts. A second launch activates the existing window and preserves the workspace.

Failed and cancelled exports retain completed source downloads for retry. Transient network failures receive bounded, cancellation-aware retries; explicit account restrictions and unavailable media stop with guidance. A retry reuses source downloads, although audio preparation may run again.

The native layer atomically saves the export plan, destination, progress, and opaque browser selection in a private `recovery.json` beside cached media. On restart it offers the most recently active unfinished job; completed, dismissed, malformed, and incompatible records are skipped. Missing or older browser selections require an explicit profile choice. Successful exports remove their job directory. Dismissing recovery retains cached media; [manual cleanup](usage.md#local-data-and-cleanup) remains available until cache management is implemented.

Keep the `com.moonlark.audio-dl` bundle identifier and preference namespace stable across branding changes. Handle incompatible recovery records explicitly.

## Desktop shell

Jacket uses one resizable window, initially 900 × 720 with a 600 × 600 minimum. Startup/import and rendering failures retain a visible recovery action. Native decoration follows the platform; no tray host is required.

| Behavior | macOS | Windows | Linux |
| --- | --- | --- | --- |
| Close while idle | Hide; reopen through Dock | Close app | Close app |
| Close during inspection, export, or destination selection | Hide; operation continues | Minimize; retain taskbar entry | Minimize; retain dock/overview entry |
| Reopen / second launch | Restore existing workspace | Same | Same |
| Explicit Quit | Cancel operations; retain unfinished-export recovery | Same | Same |
| Save / reveal | Native dialog / Finder | Native dialog / Explorer | Native dialog / default file manager |

This table describes intended behavior. The [release checklist](releasing.md#desktop-acceptance) defines native acceptance; Windows packaging is unverified, and Linux headless CI does not establish full desktop acceptance.

## Audio runtime and downloader updates

Packaging explicitly selects a [macOS runtime overlay](packaging-macos.md) or the [Linux preview overlay](linux-preview.md). Ordinary development and unit tests do not require generated runtime resources. `Toolchain::discover_with_resources` selects bundled tools when present; an incomplete bundle is an explicit reinstall error rather than a mix of bundled and host binaries. Without packaged resources, development discovers installed tools.

Managed yt-dlp updates support macOS (`yt-dlp_macos`) and Linux x86_64 (`yt-dlp_linux`). Automatic checks use the official nightly channel by default and cache release checks for up to a day. Release size, SHA-256, version, and YouTube-extractor validation must pass before staging. Updates live in app data outside the installed bundle; a prepared candidate waits until album jobs are idle before an atomic activation. Existing jobs retain their chosen executable. Failed updates keep the current downloader; missing or corrupt managed copies fall back to the bundled version. Automatic updates do not downgrade.

Local-install mode validates an explicitly selected executable and preserves package-manager symlinks. Jacket neither modifies that installation nor checks GitHub for updates in local mode. A missing local executable is an error instead of an implicit switch. Downloader updates do not update FFmpeg, Node, or Jacket; whole-app updates are not configured.

## Browser sessions and credential boundaries

Public access is the default. Rust discovers browser-profile metadata and exposes opaque IDs and display labels. Each explicit test or job resolves the exact selection again; missing profiles never fall back to another account. Firefox containers are distinct selections; Safari named profiles are unsupported by this importer. New work starts publicly after restart, while explicit recovery retains its browser selection.

Cookie values and resolved credential paths stay outside frontend state, recovery records, archives, and manifests. Access testing inspects the first item without downloading media or exporting cookies. Public-link success does not establish account-only access. Permission, decryption, expiry, locked-database, and missing-profile failures receive specific guidance; a generic 403 does not imply sign-in is needed. Stopping session use clears Jacket's selection without changing the browser.

There is no production in-app sign-in or application server. Normal account access uses an existing browser session rather than an OAuth token for the downloader. The application invokes yt-dlp through argument arrays and ignores user/global downloader configuration. See [SECURITY](../SECURITY.md) for the other trust boundaries.

### Development-only owned-session prototype

The separate owned-session experiment remains excluded from production Settings and normal recovery. On macOS, launch a debug app with `JACKET_AUTH_PROTOTYPE=1` (for example, `JACKET_AUTH_PROTOTYPE=1 mise run desktop`) and choose **Developer session prototype…** from the Jacket menu. **Connect YouTube** opens an isolated Chrome profile; sign-in is human-operated. **I've signed in · save session** captures the YouTube session into a nonsynchronizing Keychain item. **Verify account access** requires an account-only link and checks that public access fails.

The native prototype stores its state under the app data directory's `auth-prototype/`, with credentials in Keychain service `com.moonlark.audio-dl.auth-prototype`, account `youtube-session-v1`. Private plaintext handoffs exist only during downloader use and are cleaned on completion and debug-app restart. **Disconnect** cancels account-backed work, disables the selection, and removes the saved credentials, owned profile, handoffs, and unfinished account-backed caches; retry Disconnect if cleanup reports an error. Completed archives remain. Hiding the feature does not delete saved credentials, and local deletion is not remote account sign-out.

The older standalone probe is separate: `mise run auth:prototype diagnose` only checks executable locations, while `mise run auth:prototype open` launches Chrome with `~/Library/Application Support/com.moonlark.audio-dl.auth-prototype/chrome-profile`. It does not extract cookies or save a Keychain session. Its profile persists; close the separate Chrome instance before manually removing that exact prototype directory.

Synthetic tests cover session storage, cookie filtering, expiry, cancellation, and cleanup. A complete live restricted-content download, restart, expiry/reconnect, and disconnect lifecycle remains unverified. Complete the [authentication acceptance checks](releasing.md#browser-session-acceptance) before enabling this experiment in production.
