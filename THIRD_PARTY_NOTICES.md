# Third-party software and artwork

Jacket’s own code and documentation are licensed under GPL-3.0-or-later. That does not replace the licenses of its dependencies, system components, or bundled tools. The canonical project license is in [LICENSE](LICENSE).

## Application dependencies

The Rust and JavaScript dependency graphs are recorded in `Cargo.lock` and `package-lock.json`. Major components include [Tauri](https://github.com/tauri-apps/tauri), [React](https://github.com/facebook/react), [Mantine](https://github.com/mantinedev/mantine), [Tabler Icons](https://github.com/tabler/tabler-icons), [Lofty](https://github.com/Serial-ATA/lofty-rs), [Reqwest](https://github.com/seanmonstar/reqwest), and [zip](https://github.com/zip-rs/zip2). Each retains its upstream copyright and license. Review the complete dependency graph when preparing a binary distribution; this overview is not a complete binary license inventory.

## Audio runtime

Packaged macOS builds stage installed copies of [yt-dlp](https://github.com/yt-dlp/yt-dlp), [FFmpeg and FFprobe](https://ffmpeg.org), [Node.js](https://nodejs.org), [Python](https://www.python.org), and their required libraries and Python packages. Development builds can use locally installed copies instead.

Staging retains available license files, Python distribution metadata, Homebrew receipts, recipes, and software bills of materials under the bundled runtime. Its manifest records installed sources, tool versions, and hashes. These records help identify what is shipped; they do not by themselves satisfy every license or corresponding-source requirement.

The current Homebrew FFmpeg runtime includes GPL components. Before publishing a bundled app, review every staged component, preserve its required notices, and prepare the corresponding sources and build information required by its license. Refer to [FFmpeg’s licensing guidance](https://ffmpeg.org/legal.html) and the [distribution prerequisites](docs/tool-bundling.md#distribution-prerequisites). A source-code release of Jacket does not include the ignored, generated audio runtime.

## Brand and example content

Jacket’s logo and icon sources are in the [brand kit](branding/jacket/README.md). The wordmark uses vector outlines derived from Avenir Next Demi Bold; font files are not included. The built-in “A Quiet Afternoon” album is a fictional UI fixture with local example artwork. The [documentation screenshot gallery](docs/screenshots/README.md) uses that fixture and contains no browser session, real account data, or third-party album artwork.

YouTube, Google, and other third-party marks belong to their respective owners; Jacket has no affiliation with them.
