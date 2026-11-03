# Native architecture

Jacket is a local Rust application rendered by iced 0.14. It prepares tagged audio
for a music library and for its sister product, Resonant. The native workspace has
no Tauri bridge, browser rendering layer, application server or telemetry.

## Boundaries

```text
apps/jacket (iced application)
    ├── jacket-core (review state and export planning)
    ├── jacket-desktop (native services, persistence and job ownership)
    └── audio-engine (shared transport/domain types)

jacket-core / jacket-desktop
    └── audio-engine
        └── yt-dlp + FFmpeg/ffprobe subprocesses
```

The app owns widget composition and messages. `jacket-core` represents the review
in Rust: selection is by playlist position, original metadata remains immutable,
shared album defaults are applied before export, and source intent distinguishes
an individual song from its playlist context. Export preview and publication use
the engine's canonical naming and metadata rules.

The app's `app/` modules separate state, startup, message handling, job events,
review lifecycle and persistence. Widget composition, metadata editors, crop
geometry and the virtualized track list live in their respective leaf modules.

`jacket-desktop` owns native paths, private review/recovery storage, exact browser
profile resolution, app-owned artwork and downloader selection. It supervises
background operations so slow filesystem, HTTP and process work does not run in
the iced event handler. Workers publish progress and one terminal result; each
admitted operation owns a distinct cancellation token. Closing the app cancels
active work and allows worker teardown rather than abandoning subprocesses.

`audio-engine` handles inspection, stream acquisition, codec/container selection,
tagging, artwork, hashes and transactional publication. It does not depend on
iced or desktop dialogs. Its existing media integration suite generates synthetic
audio and verifies exports independently with Mutagen.

The retained `src/` and `src-tauri/` tree records the previous React/Tauri behavior.
It is excluded from the native Cargo workspace. Runtime staging scripts are reused
for their verified audio payloads; native packaging does not invoke Tauri.

## Review and recovery

Review storage records unfinished work, not an export history. The versioned
`review.json` contains the inspection, edited tags, selected occurrence indexes,
export choices, session identity and revision. Atomic replacement and directory
synchronization protect committed records. Tombstones and retired identities
prevent old saves from reviving discarded or completed work.

Export jobs persist their plan, destination and progress in a private job cache.
The engine keeps completed source downloads available for retry without publishing
partial output. On success, completion writes a terminal checkpoint, conditionally
clears only the review snapshot used for that export, and removes the cache.
Newer edits or unrelated drafts survive completion. Cleanup failures become
warnings on the successful result; startup can retry retained terminal cleanup.

Persistence retains the `com.moonlark.audio-dl` application namespace. Native
review and recovery records reuse the versioned Rust formats from the legacy
application. Browser cookie contents and resolved credential paths are skipped
by serialization. The old webview's localStorage preferences are not automatically
read by the native process. See [migration and qualification](native-port.md).

## Tools and credential boundaries

Native resources are discovered relative to the executable:

- macOS: `Jacket.app/Contents/Resources/audio-tools/`.
- Linux portable candidate: `resources/audio-tools/`, beside `bin/`.
- Development: installed trusted tools discovered from PATH when no resource tree exists.

A present but incomplete resource tree fails explicitly; tools are not silently
mixed between the bundle and host. The selected downloader executable is resolved
for each admitted job, so a managed update cannot change a running operation.
Runtime staging validates pinned inputs, tool execution and relocation; native
packaging rechecks the complete manifest and SHA-256 values before copying.

Public source access is the initial mode. Browser access requires an explicit
browser/profile selection. Resolution is repeated for each operation and never
falls back to another profile. Native-only credential paths remain outside review
JSON, manifests and archives. An access test that succeeds publicly does not prove
restricted-content access. The development-only owned-session experiment remains
legacy reference and is not a production sign-in flow.

The engine invokes tools with argument arrays, ignores ambient yt-dlp configuration,
validates supported HTTPS sources and owns process cancellation. Output naming,
path validation, size limits, atomic publication and non-overwrite behavior remain
engine responsibilities. See [SECURITY.md](../SECURITY.md).

## Build and qualification

mise selects the compiler and tools; Cargo.lock fixes the native dependency graph.
`mise run check` validates workflows, script behavior, dependency boundaries,
formatting, Clippy and Rust tests. `mise run test:media` exercises real local audio
exports without network or accounts. CI uses Ubuntu x86_64 and macOS Apple Silicon.

Native candidates bundle the complete audio runtime, notices and native source
snapshot. Linux also includes the staged runtime's corresponding-source companion.
Candidate workflows are manual and do not publish releases. Signing/notarization,
installer upgrades, live-account behavior and graphical acceptance need their own
platform evidence; the checklist is in [native port](native-port.md).
