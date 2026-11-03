# Third-party software and artwork

Jacket's own code and documentation are licensed under GPL-3.0-or-later. Its
dependencies, fonts, system components and bundled tools retain their original
licenses and copyrights. The canonical application license is in [LICENSE](LICENSE).

## Native application dependencies

The native dependency graph is fixed by `Cargo.lock`. Major components include
[iced](https://github.com/iced-rs/iced), [wgpu](https://github.com/gfx-rs/wgpu),
[winit](https://github.com/rust-windowing/winit), [rfd](https://github.com/PolyMeilex/rfd),
[Lofty](https://github.com/Serial-ATA/lofty-rs),
[Reqwest](https://github.com/seanmonstar/reqwest), and
[zip](https://github.com/zip-rs/zip2). The native app does not ship the retained
React/Tauri frontend or its GTK/WebKit dependency tree.

`mise run licenses:native` retains the target's resolved Cargo package notices,
license expressions, repository identities and exact source trees. Native
packaging requires the generated inventory to match both the target and
`Cargo.lock`; modified or incomplete notices stop packaging. The app candidate
contains `notices/application-licenses`, and the accompanying
`jacket-native-dependency-sources.tar.gz` retains the resolved registry sources,
including build/test dependencies when present in Cargo's graph.

Some published crates omit separate license files. Checked-in supplements under
[third-party/licenses](third-party/licenses) retain upstream notices with immutable
revision/URL provenance and SHA-256 hashes in
[scripts/native-license-overrides.json](scripts/native-license-overrides.json)
and the earlier [override inventory](scripts/linux-license-overrides.json).
Exact upstream `.cargo_vcs_info.json` revisions or verified publisher release tags
identify the native supplements. Where publishers provide no separate notice, the
retained declaration and canonical legal text are explicitly identified in provenance.
For hexf-parse, which declares CC0-1.0 but omits its legal code, the canonical CC0
text is retained from SPDX's license data with an immutable revision. Missing or
changed supplements stop collection. Published package notices take precedence
when an updated package supplies its own files.

This inventory identifies the shipped sources and notices. It does not replace
review of the final binary's full dependency closure or the requirements of each
component's license before public distribution.

## Fonts and branding

The native interface embeds Geist under the SIL Open Font License 1.1. The font's
copyright and full license are in [assets/fonts/OFL.txt](assets/fonts/OFL.txt),
included in native candidates as `notices/Geist-OFL.txt` and in the source snapshot.

Jacket's logos and icon sources are in the [brand kit](branding/jacket/README.md).
The wordmark uses vector outlines derived from Avenir Next Demi Bold; those font
files are not included. Fictional example tracks and local example artwork exist
only for interface exploration and qualification.

## Audio runtime

The Linux audio runtime builds FFmpeg 8.1.3, zlib 1.3.2, LAME 3.100 and Opus 1.5.2
from hash-pinned source archives. It includes Node 24.21.0, yt-dlp 2026.8.19 and
matching yt-dlp-ejs 0.8.0. System Python, CA certificates and the C/C++ runtime are
not copied into the audio payload. FFmpeg's optional host-library discovery,
networking, GPL and nonfree build options are not enabled. zlib, LAME and Opus are
linked statically into FFmpeg/ffprobe. Notices and build settings remain under
`audio-tools/share/licenses`, Python package metadata and `build-info.json`.
Exact inputs and hashes are in [scripts/linux-runtime.lock.json](scripts/linux-runtime.lock.json).

Linux candidates must be accompanied by `jacket-linux-runtime-sources.tar.gz`.
It retains exact source archives and the build recipe, including the LGPL-covered
components. Native packaging verifies the companion's checksum, its build record,
and its source/build-recipe hashes against the staged runtime metadata.

macOS candidates stage installed trusted copies of yt-dlp, FFmpeg/ffprobe, Node,
Python and their runtime closure. Staging retains available license files, Python
distribution metadata, Homebrew receipts, recipes and software bills of materials.
The manifest identifies installed tool paths, versions and hashes. Homebrew FFmpeg
builds can include GPL components. Before public distribution, review that exact
closure and supply all required notices, corresponding sources and build information.
See [FFmpeg's licensing guidance](https://ffmpeg.org/legal.html) and
[native qualification](docs/native-port.md).

Development builds can use installed audio tools. Ignored generated runtimes are
not part of a source checkout. Optional managed yt-dlp updates retain the upstream
downloader's license and update independently from Jacket.

## Retained legacy implementation

The `src/`, `src-tauri/`, `package.json` and `package-lock.json` files retain the
previous React/Tauri implementation for migration reference. Their dependencies
include Tauri, React, Mantine and Tabler Icons. The MIT-licensed GLib safety
backport under [vendor](vendor/README.md) belongs to that legacy build, which is
excluded from the native workspace and candidates. Earlier Linux installer scripts
and notices describe that older packaging path.

YouTube, Google and other third-party marks belong to their respective owners.
Jacket has no affiliation with them.
