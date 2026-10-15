# Jacket desktop experience

The standalone flow is **paste link → review album → choose destination → download**, in one normal, resizable window. The toolbar, Settings, save dialog and recovery behavior stay consistent across platforms. Native window decoration follows the desktop.

## Single-window workflow

- Startup shows the link-entry screen with the Jacket identity and a focused URL field. Pasting a complete supported link starts inspection; typed links submit with Enter or Find tracks.
- Reading feedback and Cancel appear in the same window. The first streamed tracks replace the start screen with review while inspection continues. Failed lookups retain the URL; stopped or failed partial results remain clearly marked and can be inspected again.
- Track edits, selection, search, progress and unfinished-export recovery remain owned by the same mounted workspace.
- One **Download album…** / **Download playlist…** button opens the native save dialog, prefilled with a collision-safe filename and remembered folder. Cancelling starts no job. Retry/resume retains its chosen destination and cached downloads.
- Album and playlist metadata use drafts: Save changes commits; Cancel, Escape and closing discard. Track editors identify inherited fields and can save the current track before opening album details. Dialogs restore keyboard focus to their trigger.
- **New download** returns to the start screen and focuses the link field. **Undo new download** restores the previous complete review, including edits, selection, artwork, format, browser profile and retry destination. Failed inspections retain Undo; successfully loading another source retires it. Active operations and open metadata drafts cannot be replaced through native menus. A failed recovery dismissal leaves the existing review intact.
- Progress counts actual prepared tracks, identifies the active track, and shows ZIP creation as a separate indeterminate stage. Failed and stopped jobs retain completed counts. File-access and disk-space errors offer another destination without starting a download; renaming a retry opens a new save dialog.
- Settings progressively reveals browser-session setup and includes readable downloader status with Back to Settings navigation. There is no tray/background-mode setting.
- Native menus retain New download, Open Jacket, Settings, Quit, text-editing shortcuts, and development-only example preview. A retained mailbox handles menu requests sent before the webview subscribes. Requests are processed serially.
- A second launch or macOS Dock reopening restores the existing window without resetting edits, selection or a running operation.
- Startup failures retain a visible Reload action; React rendering failures have a recovery screen. The normal window is visible before frontend startup completes.

## Platform behavior

| Area | macOS | Windows | Linux |
| --- | --- | --- | --- |
| Main window | Normal resizable window | Normal resizable window | Normal resizable window, compositor placement |
| Close while idle | Hide; reopen through Dock | Close app | Close app |
| Close during an operation | Hide; job keeps running | Minimize; retain taskbar entry | Minimize; retain dock/overview entry |
| Reopen / second launch | Restore existing window | Restore existing window | Restore existing window |
| Explicit Quit | Cancel operations; retain unfinished-export recovery | Same | Same |
| Save/reveal | Native save dialog / Finder | Native save dialog / Explorer | Native save dialog / default file manager |

No tray host, icon geometry, indicator extension, floating launcher or absolute positioning is required. Linux and Windows packaging remain unverified; their current runtime uses locally installed media tools. The managed downloader updater currently supports macOS only. macOS retains its separate packaging configuration and bundled runtime.

## Screenshots

The [documentation gallery](screenshots/README.md) shows the start screen, album review, and album-details editor using Jacket’s built-in fictional example. These preview images illustrate the interface; the native validation below records separate test runs.

## Validation

Automated checks cover native-menu startup delivery, serial requests, busy/editor guards, failed recovery dismissal, example preview, and platform close policy. Existing checks cover startup import failure and stale-chunk recovery, save-dialog defaults and cancellation, preferences, metadata, exports, retries, restart recovery and downloader management.

The standalone pass passed 31 frontend tests, 42 Rust unit tests, six offline media integration tests, and three resource-bundling tests, plus TypeScript, Biome, rustfmt and Clippy checks. A debug macOS app with embedded frontend assets and bundled media tools was built and its complete code signature verified. Native checks confirmed focused startup, Settings and downloader access, example review, track editing, protection against replacing open drafts or active inspection, preserved edits/selection after close/reopen, New download keyboard navigation, editable failed URLs, automatic paste inspection, streamed real album metadata, cancellation and reinspection. An 11-track live test source reached complete review; its native save dialog opened in the remembered Downloads folder with a collision-safe numbered ZIP filename. Cancelling returned to review without starting a download, and the toolbar New download action returned to a focused, empty start screen. Live audio export was not repeated in this pass; the six offline integration tests exercised the real media pipeline.

The October 15, 2026 polish pass passed 51 frontend tests, 57 Rust unit tests, all six offline media integration tests, three bundling tests and four authentication-capture tests, plus TypeScript, Biome, rustfmt and Clippy. The production UI and an ad-hoc signed debug macOS app were built; the final app passed deep, strict signature verification. Browser checks at 900 × 720 and 600 × 600 verified metadata draft discard/save, automatic filename following, selection-preserving New download Undo, inherited-field navigation without losing track edits, keyboard focus restoration, Settings and downloader navigation, and the compact review layout. A visual check also found and corrected an indeterminate-selection checkbox whose background had been overridden by the unchecked style. Native relaunch verification was still pending at the end of that initial polish pass; the follow-up below closes the macOS public-download gap.

The October 15 native follow-up completed two real full-album exports of an 11-track live test source. The final build produced a 112,307,843-byte ZIP with 11 MP3s (55 minutes of audio), a shared cover, playlist and manifest, with no export warnings. Independent FFmpeg decoding passed for every complete audio file; Mutagen and FFprobe verified album, artist, album artist, year 1976, numbering 1–11 and codecs. Every audio SHA-256 matched its manifest, playlist paths matched the exported order, original and requested metadata survived recovery unchanged, and every embedded cover exactly matched the included JPEG. The final archive SHA-256 is `d83dd8db98e4c5242a4ce77520fb2323a7604f186401e9f3b47a42518022e590`.

Native checks covered automatic paste inspection, streamed review, save-dialog cancellation, metadata draft discard, New download Undo, Settings/downloader navigation, 600 × 600 resizing, closing/reopening during a download, cancellation without a partial ZIP, quit/restart recovery, resume to the same destination, and Finder reveal selecting the final archive. Cancellation retained eight prepared tracks and nine cached source files; restarting restored the exact job, and resume left those cached source bytes and modification times unchanged. Both completed job caches were removed normally.

This verification found and fixed three further issues: WebKit buttons did not receive keyboard focus on click, so dialogs and cancelled native save panels needed explicit focus restoration; secondary copper actions needed explicit readable text colors; and audio preparation unnecessarily recompressed normalized JPEG covers. Picture stream copying now preserves exact JPEG/PNG artwork across MP3, M4A, FLAC, Opus and Ogg, with regression checks for picture bytes/MIME and unchanged audio payloads. The follow-up passed 53 frontend tests, 57 Rust unit tests, all six media integration tests, TypeScript, Biome, rustfmt and Clippy. The final bundled runtime retained its recorded file hashes, and the app passed deep, strict code-signature verification after live export. The native macOS public-download workflow is signed off; authenticated sources and other platforms retain their separate acceptance requirements.

Remaining platform acceptance checks include:

| Environment | Checks |
| --- | --- |
| macOS | Live browser-profile permissions, reconnect and restricted-source downloads |
| GNOME Wayland without extensions | Full workflow; compositor placement; resizing; idle close; busy minimize; reopening; save/reveal |
| KDE Plasma / tiling Wayland | Full workflow; keyboard access; native menus; busy minimize and reopening |
| Windows | Full workflow; native shortcuts; mixed-DPI resizing; idle close; busy minimize and taskbar reopening; native save/reveal |

Launch at login, tray integration, global shortcuts and completion notifications remain optional future additions. Public distribution still needs the prerequisites in [tool bundling](tool-bundling.md).
