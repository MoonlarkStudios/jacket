# Jacket product direction

Jacket turns a music link into a recording you can keep and use. A song deserves
the same care as an album: clear metadata, considered artwork, portable files,
and a dependable path from inspection to a finished download.

Jacket is the acquisition and preparation product beside Resonant. Resonant v2
imports a local recording for listening, practice, stem separation, and analysis.
It currently works with one source recording per document. Jacket should make
that recording easy to obtain and recognize, while remaining useful with other
players and tools.

## Product principles

- **Start with what the person brought.** A video link means that recording. A
  playlist link means its ordered entries. Offer context without silently
  expanding the request.
- **Let the music keep its identity.** Unknown metadata stays unknown. Preserve
  original source values, real track/disc numbers, playlist order, and repeated
  occurrences. Applying album-wide metadata is an explicit choice.
- **Make the result tangible.** Show the selected recordings, effective tags,
  planned filenames, format, and destination before publishing. Local files are
  the product output; they belong to the person using Jacket.
- **Keep decisions near their consequences.** A song review emphasizes title,
  artist, album, and year. A collection adds selection, order, and shared details.
  Advanced options remain available without dominating the first download.
- **Earn trust through completion.** Cancellation, retry, recovery, filename
  collisions, and partial provider failures are core product behavior. Report
  what happened and preserve useful work.

Jacket retains its paper, ink, and copper identity alongside Resonant's cyan
workbench. Calm typography, strong hierarchy, readable controls, keyboard access,
and explicit editing boundaries form the shared family resemblance.

## 1. Songs are first-class

This is the first implementation milestone. The complete path is:

**Paste a link → review the song or collection → check files → save → open the
output folder.**

The start screen describes songs, albums, and playlists. Browser examples expose
both a single song and an album, remain visibly fictional, and cannot download.
One inspected recording gets a song-focused review with its title, artist, album,
and release year. Collection review retains the established album and playlist
tools. Selecting one entry from a collection does not reinterpret its metadata
as a new single or erase its album context.

When a YouTube URL contains both a video and a playlist, Jacket inspects the song
by default and offers the playlist as an explicit alternative. Playlist-only
links continue to inspect the playlist. Intent is resolved before invoking the
downloader so inspection and export agree about the requested source.

New reviews default to **Music folder**, giving immediate access to portable
audio without an extraction step. **ZIP archive** remains available. Existing
saved reviews and unfinished exports retain their chosen output kind; older
recovery records keep their legacy ZIP interpretation.

Acceptance criteria:

- Watch, youtu.be, and YouTube Music song links inspect one recording.
  A combined video/playlist link does so even when the playlist has many entries;
  choosing its playlist explicitly inspects the ordered collection.
- Playlist-only and YouTube Music album links retain collection behavior.
  Malformed URLs, unsupported hosts/schemes, duplicate query parameters, and
  missing video or playlist identifiers are handled deliberately, with no
  accidental expansion or downloader-option injection.
- Song review makes title, artist, album, and year understandable and editable.
  Missing artist/year are not inferred from uploader/upload date. Existing
  track/disc numbers survive review, export, saved-review restoration, and retry.
- Original source metadata remains distinct from edits and effective exported
  tags. Repeated playlist occurrences remain distinct selections.
- Folder output is the new-review default; explicit ZIP choices, saved reviews,
  and interrupted-job output/destination choices survive restoration.
- Preview and export share canonical naming and tagging. Existing files remain
  intact, cancellation publishes no incomplete output, and successful output
  clears only its associated unfinished work.
- Song and album examples, keyboard navigation, focus, reduced motion, and the
  600 × 600 minimum window are verified. Existing album editing remains usable.

This milestone retains the existing export bundle: audio, available artwork, a
relative-path playlist, and a provenance manifest inside a named folder or ZIP.
A direct loose-audio-file export can be evaluated separately; song support does
not require changing the manifest schema or inventing a third metadata mode.

Implementation status: the first slice is implemented with regression coverage
for source intent, song review, metadata, export naming, and legacy recovery.
The full quality gate and offline media suite pass. Real UI review, screenshots,
and keyboard/focus checks at 600 × 600 remain outstanding because browser control
was disabled in the implementation environment. Complete those checks before
release; existing documentation screenshots show the earlier album interface.

## 2. Prepare several recordings with confidence

Add a multiple-source queue once the single-review path is dependable. People
should be able to collect several unrelated songs without making a fictitious
album. Keep each item's source, metadata, artwork, access context, progress, and
retry state explicit. A failed or unavailable item should have a clear resolution
while preserving completed work.

Add destination preferences around actual use: a remembered parent folder,
readable per-recording filenames, and an optional organization rule. Preview the
resolved paths before saving. Do not silently merge unrelated sources, deduplicate
repeated recordings, or change an existing collection's numbering.

The queue needs a documented ownership and persistence model before shipping:
stable item identities, bounded concurrency, cancellation scope, destination
collisions, partial completion, disk-space errors, authentication changes, and
restart recovery. Acceptance includes mixed songs and collections, duplicate
links, an interrupted item beside completed items, and a missing destination.

## 3. Offer an explicit Resonant handoff

The initial compatibility contract is a local audio file. Resonant v2's import
picker currently offers WAV, FLAC, MP3, AAC/M4A, OGG, and AIFF; actual support
depends on its decoder. Its document title currently derives from the imported
filename. Keep Jacket filenames useful, and qualify the ordinary M4A/MP3 path
against real Resonant imports before advertising compatibility. Preserving a
source codec does not automatically establish compatibility with every player.

A future **Open in Resonant** action must follow an agreed, versioned contract
between the products. Define supported formats, target application discovery,
file identity and lifetime, single versus multiple selections, dispatch and
acknowledgement, and errors when Resonant is missing, busy, or incompatible.
Transport paths as data; never construct shell commands from source metadata.
Opening a recording must not delete it, replace active Resonant work, or imply
that analysis has completed. Cancellation and rejected imports must leave both
products' work recoverable.

Ship that action only after installation, launch, import, and failure cases pass
on supported desktop platforms. Until then, use **Open folder** and ordinary
Resonant import. There is no shared library, application protocol, or direct
handoff in the first milestone.

## Boundaries

Jacket does not become a music library, streaming player, discovery service,
metadata-matching service, or analysis workbench. It does not fabricate release
metadata, bypass access restrictions, or treat source thumbnails as verified
official covers. This path adds no cloud account, telemetry, background library
watcher, or direct `.resonant` document authoring.

Each milestone should leave a complete useful workflow. Verify behavior with
focused regression tests, the repository quality gate, offline media checks for
pipeline changes, and real UI review. Record remaining platform qualification
honestly; a browser example does not establish desktop export acceptance.
