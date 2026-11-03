# Native UI redesign

Jacket's native workspace adopts Resonant v2's semantic surface hierarchy, Geist
typography, restrained borders and compact controls. Jacket retains its own
copper accent, while Catppuccin Mocha shares the same alternate palette. The
interface puts source review, metadata correction and export decisions in the
order people need them.

## Workspace

A compact header holds application identity, the current theme and settings.
The source field starts an inspection and exposes song/playlist intent when the
link contains both. The review hero shows the album cover, title, artist, year,
track count and duration. An ordered table keeps repeated playlist occurrences
independently selectable and editable.

At 1180 × 800, the Export inspector remains beside the review. Output kind, audio
format, artwork, tag behavior and output name are grouped by the decision they
control. At the 600 × 600 minimum, Export options move into a panel and Preview
remains visible below the review. Editing forms explain shared album inheritance
rather than accepting changes that would be overridden silently.

The file preview shows the artwork selected for the export, effective metadata
and the actual naming plan. Provisional codec extensions are identified without
pretending a download has already resolved them. Audio files, folders and ZIPs
keep their distinct contents. Choosing no artwork suppresses the cover from the
export presentation. Modal actions remain outside the scrolling content.

## Appearance

Three compiled presets cover Jacket Dark, Jacket Light and Catppuccin Mocha.
The theme chooser previews each palette before selection. Custom themes use
Resonant v2's version 1 TOML format; the validated definition is embedded in the
application preference so the original file can move or be deleted. Imports
reject unknown fields, unsupported versions, malformed colors, oversized files
and invalid text. A failed import preserves the active appearance.

Theme changes recolor all application controls and panels while preserving the
review, selected tracks, metadata edits, recovery state and admitted operation.
Image pixels are not tinted by the theme. Legacy native `dark: false` settings
restore Light and `dark: true` settings restore Dark. New settings default to
Dark. A broken preference remains available for recovery until an explicit
settings change replaces it. See [theme authoring](themes.md).

## Verification

The snapshot runner defines 57 cases: review, preview, settings,
appearance, export options, individual editing, bulk editing and cropping at two
sizes in three presets; nine additional cases cover a song, the welcome state
and a 10,000-entry playlist. Additional `--theme-file` captures exercise a custom
palette without modifying the ordinary preference.

Regenerate these with `mise run qualify:ui`. Use `mise run check` for the native
quality gate and `mise run test:media` when changing audio/export behavior.
Inspect artwork resolution, text contrast, reflow, modal actions and selection
at both window sizes. Native dialogs, assistive technology, AppKit behavior and
installed distribution still need their respective platform checks.

The [initial native verification](native-verification.md) records the earlier
port milestone. Its test and capture counts are not reused as redesign results.

## Redesign validation — November 3, 2026

Validated on Linux in a Wayland desktop session using iced's wgpu renderer:

- `mise run check` passed: rustfmt, strict workspace Clippy, workflow validation,
  11 native tooling tests, dependency notice checks and 188 Rust tests. The 10
  opt-in media integration tests are outside this ordinary gate; this redesign
  does not change the audio engine or export pipeline.
- The optimized release build passed. The final 47-test application suite also
  passed after the editor focus targets were connected.
- All 57 preset/size/scenario captures completed and were visually reviewed.
  Two additional captures exercised the bundled custom Studio theme in review
  and export preview; the release review capture confirms the blue accent
  reaches actions, selection and badges.
- Artwork was present in both review and export preview. The interior image
  region was pixel-identical across Dark, Light and Mocha review captures.
  Regression tests cover imported-cover precedence, selected-track artwork,
  explicit no-artwork mode and the isolated example's cover ownership.
- Application regressions cover legacy appearance migration, embedded custom
  themes, failed imports, serialized preference writes, corrupt-preference
  preservation, review/worker ownership and modal shortcut routing.

Captures are generated artifacts under `target/redesign-final/`, not source
fixtures. These results do not qualify macOS runtime behavior, native file
pickers, complete interactive Tab order, assistive technology or installed
signed distribution. Those checks remain part of platform release acceptance.
