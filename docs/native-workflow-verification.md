# Native workflow pass — November 3, 2026

This pass follows the native UI redesign and addresses the complete song,
album and playlist workflows on Linux with Wayland and iced's wgpu renderer.

## Resulting behavior

A standalone song has a dedicated review with its cover and effective tags.
There is no one-row selection table or per-track artwork menu. The single cover
can be included, omitted, replaced or cropped. Albums retain shared release
editing and track selection; playlists retain individual tags and their own
name in the export preview, even when every song comes from the same album.
One-track albums still expose their inherited album fields for editing.

Song inspection optionally supplements yt-dlp with the exact recording's public
YouTube Music queue entry. A square image is accepted as an album cover only from
an album-linked row matching the video ID. Failure leaves the original source
thumbnail available. The lookup runs alongside inspection, has a four-second
request timeout and a bounded response, and does not use browser credentials.
The fallback image selector prefers the largest valid square cover.

JPEG, PNG and WebP source images share bounded reads and decoding checks. Review
and preview preserve the image's proportions; only an explicit crop changes its
composition. Imported and fetched images are normalized into owned JPEG assets,
without enlarging small covers and with a 2000-pixel cap for large ones. Remote
crop preparation is staged until Apply: cancelling or a stale completion cannot
replace the chosen artwork. Preview failures offer a retry. Missing selected
owned artwork must be replaced or omitted before export.

Output-name editing changes the filename or folder name independently of tags.
Cancelling a native destination dialog retains the captured export plan for
retry. Familiar audio suffixes typed into that dialog do not get duplicated;
ordinary dotted names remain intact. ZIP suffix validation is case-insensitive.
Failed or cancelled cover conversions cannot publish a partial reusable cache
entry. Text-selection contrast is stronger without changing track-row tint.

The ordinary welcome screen has no example action. Fictional content remains an
explicit development path through `mise run example` / `--example` and isolated
screenshot flags.

## Executed validation

- `mise run check` passed: rustfmt, workspace Clippy with warnings denied,
  workflow checks, 11 native tooling tests, dependency notices and 213 Rust tests
  (63 engine, 16 core, 75 desktop, 59 application).
- `mise run test:media` passed all 10 offline integrations with actual FFmpeg
  exports and an independent Mutagen reader. Coverage includes MP3, M4A, FLAC,
  Opus, Ogg and WAV; files, folders and ZIPs; tags, embedded artwork, cancellation,
  collision handling and preservation of source audio.
- The optimized native release build passed.
- All 96 preset/size/scenario captures completed and were visually reviewed.
  The final release refreshed affected export previews after the playlist-title
  correction. Two extra captures checked a custom Studio theme in song review
  and preview. Generated artifacts are in `target/e2e-final/` and
  `target/e2e-custom/`; they are not source fixtures.
- Public metadata-only inspection of the song shown in the reported screenshot,
  `lC7PEkD6agQ`, resolved **Cousin Mary (2020 Remaster)** from **Giant Steps** to
  its actual 544 × 544 album image. A disposable desktop probe fetched that
  image, imported it at 544 × 544 and produced an exact 456 × 456 crop while
  preserving the original asset. No song audio, credentials or real saved review
  were used by this probe.
- Application regressions cover cancelled destination pickers, isolated output
  renaming, failed validation, remote crop staging/cancellation/stale results,
  artwork failure/retry fencing and missing-owned-cover recovery. Domain and
  media tests cover effective metadata and legacy single-song cover resolution.

## Scope of evidence

The live source check establishes public inspection and artwork behavior, not a
live audio download or access to restricted media. Export validation used
synthesized recordings offline. Dialog callbacks and cancellation were tested in
the controller; native picker interaction, screen-reader operation and installed
macOS behavior still require their platform checks. iced's stock buttons and
pick lists do not participate in Tab focus: tested keyboard support covers text
fields and explicit application shortcuts, not a complete keyboard-only UI.
Public signing, notarization and distribution acceptance are separate work.

The already running native window is not replaced automatically. Reopen the
rebuilt native executable to use these changes; saved reviews retain their
identity and edits. A fresh inspection uses the exact-song album-cover lookup;
previously saved source thumbnails remain valid fallbacks.
