# Behavior and implementation

Detailed notes for contributors. Start with the [README](../README.md) for setup and the [user guide](usage.md) for everyday use. Paths below are relative to the repository root.

## Current workflow

- Inspect supported HTTPS YouTube and YouTube Music links with yt-dlp.
- Select tracks, including repeated entries in a playlist.
- Use collection mode to keep each track's own music metadata, or album mode to supply shared album details.
- Edit title, artist, album, album artist, track number, disc number, and release year.
- Choose per-track source images, a shared source image, or no artwork.
- Choose compatibility output (AAC/ALAC as M4A, other codecs as MP3), preserve the source codec, or use MP3 throughout.
- Save tagged audio, available images, a relative-path M3U playlist, and a source manifest in one ZIP.
- Cancel inspection or export. Automatically recover transient download failures, and resume unfinished downloads after reopening the app using saved choices and completed downloads.
- Reveal a completed archive in Finder (or the platform file manager).

The interface follows a focused **link → review → download** flow. The album header shows its title, artist, and release year while metadata fields stay hidden by default. **Edit album details** opens the prefilled title, artist, and year fields when a correction is needed, along with organization and the ZIP filename. Album and playlist details use a draft: **Save changes** applies edits, while **Cancel**, Escape, or closing the dialog discards them. The filename follows album-title edits until given a custom name. Click the cover to choose artwork. The footer previews the filename, destination folder, and download choices. **Download album…** opens the native save dialog with the filename and last chosen folder prefilled. There is one download action. Existing archives are retained and new copies receive a numbered suffix. Format preferences and the last chosen save folder are remembered locally; example edits do not change format defaults. Search appears for playlists with more than eight tracks. Pasting a complete supported link starts inspection automatically; typing a link still uses Enter or **Find tracks**. Enter saves track edits; **Save & next** moves through track edits, and Command/Ctrl + Enter starts download when no dialog is open. Escape cancels a metadata draft. Inherited track fields are labeled **From album details**; **Save track & edit album details** keeps the current track edit before opening shared details.

Unavailable entries are shown prominently before download. Album mode automatically fills missing track numbers from each track's position within its disc, preserving existing numbers and original source metadata. This applies during inspection and when switching a playlist to album mode, before track selection, so deselecting tracks leaves numbering gaps. **Number by album order** remains available if numbers are manually cleared; it explicitly renumbers all entries with an undo action. Album artist and year are prefilled when every track agrees. For recognized YouTube Music albums, a public album-header lookup runs alongside track inspection to fill missing release years. It follows the exact playlist’s album link and requires its title to match the inspected album. The verified album year takes precedence for shared album export details, even when tracks report different release or reissue years; original explicit track years remain in source metadata. Unavailable or ambiguous album metadata falls back to agreeing track years, otherwise leaving the year editable and blank. Failed downloads reuse their chosen destination on retry; changing the archive name opens the save dialog again. After success, **Download another album** starts the next download with the remembered format. Unfinished downloads reopen directly in review with **Resume download**, retaining the chosen tracks, metadata, format, artwork and destination. **New download** dismisses that recovery offer while retaining cached media. **Undo new download** restores the previous complete review, including edited tags, selection, artwork, browser profile and retry destination. The offer stays available after a failed inspection and is replaced when another source loads successfully.

Source images can be video thumbnails. They are presented as candidates; the app does not promise official album covers. MP3, M4A, FLAC, Opus, and Ogg Vorbis support embedded artwork. Images are also included as separate files in the ZIP. WAV receives a separate image and an export note.

Metadata uses ID3v2.4 for MP3, iTunes atoms for M4A, and Vorbis comments for FLAC/Opus/Vorbis. Source links use the canonical comment field (`COMM`, `©cmt`, or `COMMENT`); WAV uses `ICMT`. The native Rust tag writer requires no additional installed tagging tool. WAV uses a limited RIFF INFO profile: album artist and disc number remain in the manifest, and supplied values trigger a visible export warning. Each manifest track records both requested metadata and the subset embedded in the audio file.

Inspection streams fully inspected track metadata into the review screen as each result arrives, rather than waiting for the entire playlist. The footer shows the number found, plus the total when the source provides it. Browsing and search work during inspection; editing and saving become available after it finishes. Stopped or failed inspections retain a clearly marked partial preview with an **Inspect again** action. Final collection artwork and unavailable-entry notes are reconciled when inspection completes. This improves time to the first visible result without starting audio downloads. Album-year enrichment uses up to two public YouTube Music browse requests in parallel with yt-dlp, bounded to four seconds each; lookup failure does not fail inspection.

Unknown artist and release-year fields stay unknown when the provider has no reliable album metadata. Uploader names are not used as artists; upload dates are not used as release years. Missing track numbers are inferred from source order only in album mode. Album organization remains editable. YouTube Music album browse URLs and auto-generated `OLAK5uy…` album playlist links default to album mode with one shared collection cover; ordinary playlists default to collection mode with per-track images. Guessed unsigned `/s_p/…/maxresdefault.jpg` playlist thumbnails are skipped in favor of the images actually provided by YouTube, because the guessed image can return 404 even when yt-dlp reports a larger resolution.

Shared album fields, when supplied, override individual album/album-artist/year values at export. Individual artists and real track numbers remain intact. The manifest records the original source metadata alongside the effective exported metadata.

## Standalone desktop behavior

Jacket opens one normal, resizable window (900 × 720, minimum 600 × 600) with a focused link field. Paste a supported link or press Enter to inspect it. Loading feedback, cancellation, streamed tracks, metadata editing, download progress and completion all stay in that window. Failed lookups retain the URL for correction. **New download** returns to the start screen and focuses the link field; it waits for the current operation to finish and preserves recovery state if dismissing an old job fails.

The review toolbar contains **New download** and **Settings**, with an example marker only for preview data. Downloader management stays inside Settings. Native application menus expose **New download**, **Open Jacket**, **Settings…**, and **Quit Jacket**, plus normal text-editing commands. Command/Ctrl+N starts a new download and Command/Ctrl+, opens Settings. Menu requests are retained during startup and cannot replace an active operation or metadata editor. A second launch activates the same window and preserves the workspace. Development builds also expose **Preview example**.

On macOS, closing the window hides it while preserving edits and running jobs; the Dock reopens it. On Linux/Windows, closing an idle window closes the app normally, while closing during inspection, export or destination selection minimizes it to retain a taskbar/dock entry. Explicit Quit cancels operations and keeps unfinished-export recovery. Jacket creates no system tray icon, launcher window or background-mode preference.

Startup retains a visible opening state and a Reload action if application modules fail, including stale development chunks. Rendering errors also receive a recovery screen. The main window is visible from startup, so these states remain accessible even if the interface cannot mount.

The shared build configuration does not invoke macOS resource staging on Linux or Windows. `tauri.macos.conf.json` retains macOS signing, application-only bundling, and its bundled audio runtime. Other platforms currently use locally installed audio tools. Native Linux/Windows packaging and managed downloader updates still need platform validation.

The platform design and acceptance matrix are in [desktop experience](desktop-experience.md).

Download progress reports the actual number of prepared tracks and the current track. ZIP creation has its own indeterminate stage; failures retain completed counts and offer relevant recovery guidance. File-access and storage failures allow choosing another destination without starting a download. Browser setup is optional and revealed from Settings; downloader management includes a **Back to Settings** action.

## Domain boundaries

```text
React + Mantine
  Review state, track selection, metadata edits, export choices
       │ typed invoke calls / progress events
Tauri desktop adapter
  Native save dialog, dependency status, job ownership, app cache
       │ Rust API
audio-engine
  Source inspection → export plan → audio + assets → ZIP + manifest
       │ argument-based subprocesses and HTTPS image requests
yt-dlp / ffmpeg / ffprobe
```

`crates/audio-engine` has no Tauri dependency. Its public models, `inspect`, and `export_archive` can be reused by other GPL-compatible applications or a future command-line/server adapter. `Toolchain` provides executable locations; `Cancellation` and progress callbacks keep the engine independent of a particular job runner or UI.

`src-tauri` owns native job lifetimes and cache locations. `src-tauri/src/shell.rs` owns main-window visibility, native menus, the request mailbox, and single-instance activation; `src/shell.ts` exposes the shell transport, and `src/useDesktopShell.ts` connects native menu requests to the persistent workspace. `src/domain.ts` contains frontend transport models and export-plan assembly. `src/useWorkspace.ts` owns inspection, selection, export preferences, and job progress. `src/components/WorkspaceDialogs.tsx` contains focused metadata and export dialogs. `src/desktop.ts` is the native transport boundary. The frontend doesn't spawn processes or read browser databases.

## Downloader updates

Jacket keeps yt-dlp up to date automatically by default. On opening the app it checks the official releases (cached for up to a day), then installs a newer release when no inspection or download is running. Checks, transfers, and validation happen in the background and never disable the album flow. If you start an album while an update is being prepared, the verified candidate waits until the current job finishes before activation. A prepared update is reusable after a restart. Failed or cancelled attempts keep the current executable, and automatic failures do not repeatedly retry in the same session; **Check now** retries explicitly. Automatic updates never downgrade to an older release.

The **Audio downloader** status control in Settings opens the dedicated downloader panel. It shows the active version, background update state, and any check or installation issue. **Automatic** mode includes a persisted **Keep yt-dlp up to date** switch and **Check now**. **Advanced** contains the Nightly/Stable channel choice and the bundled fallback. Nightly is the default for the newest YouTube fixes; the upstream project recommends it for regular users: [yt-dlp update channels](https://github.com/yt-dlp/yt-dlp#update). Choosing an older channel exposes an explicit **Use latest stable/nightly** action. **Use the included version and pause updates** restores the bundled executable and turns automatic updates off so it remains selected.

**Local install** accepts an absolute executable path or the native file picker; a detected local installation is suggested. Version and YouTube-extractor checks must pass before preferences change, and changing the downloader waits for the current album job. Package-manager symlinks are preserved, so external upgrades are picked up through the same selected path. Jacket never downloads, overwrites, or updates this executable and does not check GitHub releases in local mode. A missing local executable produces a clear settings error instead of silently selecting a different installation. The mode, automatic-update preference, and local path are stored in the app's private data directory. FFmpeg, FFprobe, and Node still use the app's included runtime.

For macOS, managed updates use the official `yt-dlp_macos` asset. SHA-256 and release size must match the official GitHub metadata or checksum manifest, and local version/YouTube-extractor checks must pass before staging. Versions live outside the application bundle and Homebrew installation. Activation holds the job lock only for the verified atomic pointer switch; existing jobs retain their immutable executable. Missing or corrupted managed copies fall back to the included downloader. Downloads continue when GitHub checks fail.

Downloader updates are independent of whole-app updates and do not update FFmpeg, Node, or Jacket. The managed updater currently supports macOS; public application signing and the application updater have the distribution prerequisites described in [tool bundling](tool-bundling.md).

## Authentication and local data

In Settings, choose your existing Chrome, Firefox, or Edge browser and a named profile, then select **Use this session**. Firefox profiles also list their containers separately; ordinary tabs use **Tabs outside containers**. Safari offers its available cookie store with an explicit limitation: named Safari profiles are not supported by this importer. **Refresh profiles** reads profile metadata without reading cookie values. The default remains public access, and **Stop using this session** returns to it without signing out or changing the browser.

**Test access** uses the selected profile to inspect the first item of a YouTube or YouTube Music link without downloading media or exporting cookies. A passing public link does not prove account-only access. Expired sessions, OS permissions, decryption failures, busy cookie databases, and missing profiles have specific guidance; generic provider errors do not automatically request sign-in. To reconnect, open that same browser profile, sign in normally, return to Settings, and test again. Jacket rereads the session for each operation and never silently falls back to the most recently used profile. Missing or older recovery selections require an explicit profile choice. New work starts anonymously after restarting; unfinished downloads retain the exact browser/profile ID for explicit resume. IDs are resolved against the installed browser’s current metadata; credential paths and cookie values never enter frontend state, recovery records, archives, or manifests.

There is no production in-app sign-in, cookie upload, or application server. The app-owned session experiment is excluded from normal Settings and normal recovery. It is available only through **Developer session prototype…** in a debug build launched with `JACKET_AUTH_PROTOTYPE=1`; it cannot enable itself in the normal download flow. Its separate Chrome profile, Keychain storage, temporary handoff cleanup, and synthetic lifecycle tests remain experimental; a real restricted-content download, restart, reconnect, and clean disconnect have not all been verified live. The older standalone diagnostic remains `mise run auth:prototype diagnose`. See the [authentication investigation](authentication-investigation.md). Google OAuth does not authenticate the yt-dlp download path.

Requests go from this machine to the source provider. The application ignores user/global yt-dlp configuration so hidden downloader settings do not alter the planned export.

Successful exports remove their job working directory. Failed/cancelled jobs retain completed downloads in the app's cache so a retry can reuse them. Recovery state is atomically saved in a private local `recovery.json` alongside the media cache before export starts and as progress advances. On restart the most recently active unfinished job is offered; completed, dismissed, malformed, and incompatible records are skipped. The record contains the export plan and browser selection, never browser cookies. Cache-management UI is still future work. On macOS the cache is under `~/Library/Caches/com.moonlark.audio-dl/jobs`.

Transient 403/408/429, 5xx, timeout and connection failures trigger up to two automatic fresh yt-dlp extractions, with short cancellation-aware backoff. Cached completed sources and partial downloads are retained. Explicit login, age, unavailable and removed-content failures stop immediately; a generic 403 never silently requests account access. An export fails if an item still cannot be prepared after recovery. Artwork failures become visible warnings and allow audio export to continue. The ZIP is written to a temporary file beside the destination and committed only after all selected audio is prepared. Publication never overwrites an existing archive, including a filename created while a download is running; collisions receive a numbered suffix. Cancellation leaves no partial ZIP.

