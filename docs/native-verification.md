# Native port verification

Validated on November 3, 2026, on Linux x86_64 with a Wayland desktop, Rust 1.96.0,
iced 0.14 and FFmpeg 8.1.3. The port is based on `2a2aee9`, retaining the completed
single-audio-file workflow from Jacket's previous implementation.

| Check | Result |
| --- | --- |
| `mise run check` | Formatting, workflow lint, 11 native script tests, dependency notice preflight, Clippy with warnings denied, and 166 Rust unit tests passed. |
| Media integration | All 10 offline tests passed using real FFmpeg/ffprobe and independent Mutagen tag inspection. |
| Optimized binary | The Linux release executable built successfully with the committed lockfile. |
| Native rendering | 30 views rendered and visually inspected through the Wayland/wgpu window: review, settings, preview, individual/bulk editing and crop, both themes and window sizes; additional song, welcome and 10,000-entry cases. |
| Dependency inventories | Linux's 395 and macOS ARM's 341 resolved dependencies have verified source notices. Linux's exact dependency-source archive was collected and its integrity verified. |
| Packaging | Synthetic complete-runtime packaging tests verified executable modes, runtime/platform/source identity, notices, source snapshots and artifact checksums. Five existing Linux staging regression tests also passed. |

The 166 Rust tests cover engine behavior, native review state, atomic persistence,
completion and recovery, instance ownership across processes, worker supervision,
cancellation, browser profile resolution, artwork, updater activation, application
lifecycle and list virtualization. Media tests cover single files, folders and
ZIPs, codec preservation, embedded tags/covers, repeated source occurrences,
collision handling and preview agreement.

Visual inspection caught and corrected clipped actions, text reflow at the
minimum size, inherited metadata controls, dark-theme contrast and the crop
qualification fixture. Modal actions remain visible while their content scrolls;
the underlying workspace leaves the widget tree while a modal is open. Large
playlists render keyed visible rows with bounded overscan. Screenshot modes use
fictional data and do not open the ordinary data store or enable downloads.

Captures are generated under ignored `target/native-snapshots` directories. Run
`mise run qualify:ui` to regenerate the complete set on the target desktop.

macOS compilation/runtime, the AppKit Quit bridge, signing/notarization, installed
bundles, native dialog interactions, assistive technology, and live provider/account
behavior still require their platform acceptance checks. No signed release,
installer upgrade, live-account access or Windows support is claimed by this
verification. See the [acceptance checklist](native-port.md#acceptance-record).
