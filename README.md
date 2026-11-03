<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="branding/jacket/svg/jacket-logo-dark.svg">
    <img src="branding/jacket/svg/jacket-logo-light.svg" alt="Jacket" width="360">
  </picture>
</p>

<p align="center"><strong>From link to neatly packed.</strong><br>Prepare tagged audio for your music library and Resonant.</p>

Jacket is a native desktop application from [Moonlark Studios](https://github.com/MoonlarkStudios).
Paste a YouTube or YouTube Music link, review the tracks and metadata, and save
local audio. Jacket is the preparation companion to Resonant: export a recording,
then open it in Resonant for listening, practice and analysis.

This branch ports Jacket to **Rust and iced 0.14**, following the native direction
of Resonant v2. The audio engine, export formats and unfinished-work persistence
remain Rust. The default app and checks use Cargo through mise. The retained
React/Tauri implementation is migration reference; it is outside the native
workspace. See [native port and qualification](docs/native-port.md) for the
migration boundary and remaining platform acceptance.

## Run from source

Install [mise](https://mise.jdx.dev/getting-started.html) and your platform's native
build tools. macOS development uses Xcode Command Line Tools and Homebrew. On
Ubuntu, install the build and windowing dependencies described in
[CONTRIBUTING.md](CONTRIBUTING.md), plus FFmpeg and yt-dlp.

```sh
git clone https://github.com/MoonlarkStudios/jacket.git
cd jacket
mise trust
mise install
mise run setup
mise run dev
```

`rust-toolchain.toml` pins Rust with rustfmt and Clippy. mise also pins Python,
Node and the check tools. Node is used by yt-dlp for JavaScript challenges;
**npm and a webview are not required to build or run the native app**. Development
runs discover FFmpeg, ffprobe and yt-dlp from your installation. `mise run doctor`
checks those prerequisites without changing the machine.

Explore fictional tracks with `mise run example` (the binary's `--example` flag).
The ordinary app has no example entry point. Example content is for interface
review; real downloads require an inspected source.

## Prepare your audio

1. Paste an HTTPS YouTube or YouTube Music song, album or playlist link. When the
   link includes both a video and playlist, choose the song or the entire playlist.
2. Find tracks, choose the entries to keep, and correct their metadata. Selection
   follows each playlist occurrence, so repeated songs remain distinct.
3. Choose the format, output type and artwork behavior. Preview the effective tags
   and filenames before saving.
4. Choose a destination in the native file dialog and start the export. Progress
   and Cancel remain available while audio tools run in a background worker.

A single song can be saved as an audio file. Folder and ZIP outputs include tagged
tracks, optional artwork, a relative M3U playlist and a provenance manifest.
Existing destination files are preserved; name collisions produce numbered copies.
Unfinished reviews and interrupted exports are retained locally for restart.
Successful exports clear their associated working state after publication.
Use Ctrl/Cmd+L to focus the source, Ctrl/Cmd+S to preview files, and Ctrl/Cmd+,
to open settings. Escape closes an editor or cancels the active operation.

| Format | Behavior |
| --- | --- |
| M4A / MP3 | Keeps AAC/ALAC in M4A; converts other source codecs to MP3 when needed. |
| Keep source codec | Preserves supported AAC, ALAC, MP3, Opus, Vorbis, FLAC and PCM audio streams. |
| MP3 | Produces MP3; existing MP3 audio can be copied without re-encoding. |

Preserving a codec retains the quality available from the source. WAV has limited
tag support; exports report metadata and artwork that cannot be embedded. See the
[manifest contract](docs/manifest.md) for original metadata, effective tags, codecs,
warnings and SHA-256 hashes.

## Native workspace and themes

The workspace uses the same semantic surface hierarchy, Geist typography and
compact controls as Resonant v2, with Jacket's copper identity. The review keeps
album artwork and release details beside an ordered track table. A song has its
own focused view for one recording's tags and cover. Albums share release details;
playlists retain individual track details. The Export inspector groups the output, format, artwork and tag choices; the file preview
shows the selected cover, effective tags and filenames before saving. At narrow
window sizes, Export options open in a panel and Preview remains visible.

Choose **Theme** in the header to select Jacket Dark, Jacket Light or Catppuccin
Mocha, or load a custom TOML theme. Theme definitions use Resonant v2's version 1
semantic format. Imported definitions are retained with the application settings;
changing appearance preserves the review and export state. See [themes](docs/themes.md)
and the [native UI redesign](docs/ui-redesign.md). The latest artwork, editing and
export checks are recorded in the [workflow pass](docs/native-workflow-verification.md).

## Local data and access

Jacket has no application server or telemetry. The audio tools contact the source
service and image hosts. Optional downloader updates contact GitHub; these update
yt-dlp separately from Jacket. Whole-app automatic updates are not configured.

Public access is the default. Browser access is an explicit local selection of an
installed browser profile, resolved again for each operation. Cookie contents and
credential paths are excluded from review storage, export manifests and archives.
A successful public-link test does not establish access to private content.

The app keeps the `com.moonlark.audio-dl` data namespace for migration. Recovery
contains source URLs, metadata and export destinations, and is stored privately.
Run the native app independently from the legacy app while reviewing migration;
see [native port](docs/native-port.md) for compatibility and local data details.

Download only content you own or have permission to download, and follow the
source service's terms. Jacket does not bypass DRM or grant access to restricted
content. It is not affiliated with YouTube or Google.

## Build and verify

```sh
mise run check       # Native formatting, Clippy, unit tests, scripts and workflows
mise run test:media  # Offline FFmpeg exports, archives and independent tag checks
mise run build       # target/release/jacket
mise run qualify:ui  # Capture 96 native workflow views; needs a display
```

Build output honors `CARGO_TARGET_DIR` when set. Local app candidates with a complete bundled audio runtime:

```sh
mise run package:mac    # target/native-candidates/Jacket.app, macOS only
mise run package:linux  # target/native-candidates/jacket-linux/, Linux x86_64
```

Packaging validates runtime checksums and includes exact target-specific Cargo
notices, dependency sources, the Geist font license, and native source snapshots. Linux also includes the corresponding audio-runtime sources.
The macOS bundle is ad-hoc signed for local qualification. Native installer
acceptance, public signing/notarization and final dependency/source compliance
must be completed before public distribution. The manual candidate workflow
uploads review artifacts; it does not publish a release.

macOS Apple Silicon and Linux x86_64 are the development/CI targets. CI checks
compilation and device-independent behavior; it does not establish interactive
desktop acceptance. Windows and other architectures need separate qualification.
See [CONTRIBUTING.md](CONTRIBUTING.md), [architecture](docs/architecture.md), and
[native qualification](docs/native-port.md).

Copyright © 2026 Moonlark Studios. Licensed under
[GPL-3.0-or-later](LICENSE); dependencies retain their own licenses. See
[third-party notices](THIRD_PARTY_NOTICES.md).
