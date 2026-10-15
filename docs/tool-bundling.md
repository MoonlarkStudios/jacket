# Self-contained macOS audio tools

The packaged app now includes a complete local runtime. End users do not need Homebrew, Python, Node or FFmpeg. Development (`mise run desktop`) can still discover installed tools when no packaged resource directory exists.

## Build and verify locally

```sh
mise run setup
mise run tools:stage
mise run tools:validate
mise run test:bundling
mise run package:mac --debug
```

Mise selects the pinned Node and Rust tools; `setup:media` installs missing Homebrew media formulae. The media distributions remain Homebrew-managed because this bundler requires their Python runtime and dynamic library closure.

`mise run package:mac` merges `src-tauri/tauri.bundle.macos.json`, which stages tools automatically through `beforeBuildCommand` and includes them as bundle resources. The automatically loaded `tauri.macos.conf.json` contains only platform settings, so ordinary development, tests, and lint work in a fresh checkout without generated resources. Direct Tauri builds need the additional bundling configuration to produce a self-contained runtime; use the packaging task above.

Staging uses installed `yt-dlp`, `ffmpeg`, `ffprobe`, and `node`, resolves their real paths, and never downloads executables. The current script expects the Homebrew Python-based yt-dlp distribution. Other distributions fail with an explanation rather than being silently mispackaged.

Generated resources go to `src-tauri/resources/audio-tools/`, mapped to `Contents/Resources/audio-tools/` in the app. `manifest.json` records versions, source locations, architecture, minimum macOS version, and SHA-256 hashes of every staged file. Staging is repeatable from the same installed inputs; this is not a byte-identical build guarantee (native tool signatures and upstream installs may change). Updating installed tools produces a different manifest and requires a new app build.

The current installed toolchain is Apple Silicon: yt-dlp 2026.8.19, FFmpeg/ffprobe 9.0.2, Node 24.21.0 and Python 3.14.7. FFmpeg was compiled for **macOS 26.0**, so this local bundle's minimum is 26.0. The staging step rejects a tool requiring a newer OS than the configured minimum, or a Mach-O that does not support the build host's architecture. It cannot cross-build an Intel or universal app from these arm64-only inputs. For broader distribution, build a smaller codec-focused FFmpeg/runtime set with the intended older deployment target, on each architecture.

## What is bundled

Copying only `/opt/homebrew/bin/ffmpeg` or `/opt/homebrew/bin/yt-dlp` is insufficient. FFmpeg dynamically depends on codec libraries under Homebrew. yt-dlp is a Python script with an absolute interpreter shebang; Homebrew's Python executable is itself a launcher that spawns `Python.app`.

The staging script copies:

- FFmpeg, ffprobe and Node.
- The actual framework Python interpreter, libPython, the standard library and yt-dlp's effective package directories (including EJS, certifi, cffi and its native extension).
- Each non-system Mach-O dependency recursively, including Python extension dependencies. Every copied load command is rewritten to an app-relative `@loader_path` reference. Unresolved dependencies fail staging.
- Installed license texts, Homebrew bottle receipts, formula recipes and SBOMs when present; Python package license metadata remains included.

The yt-dlp launcher anchors Python's stdlib and packages to its own directory, skips site configuration and external `.pth` files, disables user packages and Python bytecode writes (`-B` and `PYTHONDONTWRITEBYTECODE`), and uses bundled CA certificates. Validation uses the same no-bytecode setting, so no generated cache files are shipped or created inside the signed app. It does not use a shell PATH to find its Python interpreter. Node is passed explicitly to yt-dlp by the existing downloader setup; FFmpeg is passed explicitly during export.

Generated files are owner-writable so repeated Tauri resource copies can overwrite their prior output; staging also repairs legacy generated target copies. Installed source files are never chmodded. A regression test checks repeated copying of read-only package inputs and preservation of executable bytes/modes.

Every native copy has a local ad-hoc signature. Staging does not alter installed executables or weaken Gatekeeper settings. This local signature is not a Developer ID distribution signature.

Jacket also sets `bundle.macOS.signingIdentity` to `-`, so Tauri signs the main executable and seals the outer application bundle, rather than relying on the linker's executable-only signature. The bundled helpers keep their staging signatures. For the release bundle, verify the complete seal with `codesign --verify --deep --strict target/release/bundle/macos/Jacket.app`. Developer ID distribution signing must replace this local identity before public notarization; see [Tauri's signing configuration](https://v2.tauri.app/distribute/sign/macos/#ad-hoc-signing).

Validation checks manifest hashes and rejects missing/unexpected resource files, including untracked bytecode caches. Its temporary synthetic audio file is removed before the staging manifest is generated. Validation scans every Mach-O for remaining non-system dependencies and verifies signatures. It checks all four tool versions, imports native Python extensions and EJS, verifies the CA bundle, encodes synthetic audio to MP3 and probes it. Staging repeats these checks after copying the entire runtime into a temporary path containing spaces, using only `/usr/bin:/bin` in PATH, an empty HOME and no inherited runtime variables. This verifies relocation and removes PATH/profile dependence; a separate clean Mac smoke test remains necessary before shipping publicly.

To validate the runtime inside the final app:

```sh
mise exec -- python scripts/stage-macos-tools.py --validate 'target/debug/bundle/macos/Jacket.app/Contents/Resources/audio-tools'
```

## Runtime integration

Use `Toolchain::discover_with_resources(&app.path().resource_dir()?)` at each Tauri toolchain initialization, with `tauri::Manager` imported. It selects `audio-tools/bin/{yt-dlp,ffmpeg,ffprobe,node}` when that resource directory exists. An incomplete/nonexecutable bundled runtime is an explicit reinstall error, without mixing host binaries into a broken package. If no runtime is present, it falls back to installed-tool discovery for development.

## Distribution prerequisites

Local inspection found **zero valid code-signing identities**. Xcode's `codesign`, `install_name_tool` and `notarytool` are installed, but credentials were not provided or invented.

A public macOS release needs an Apple Developer account, a **Developer ID Application** certificate/private key, and Apple notarization credentials. Native helper executables and libraries must be signed before the outer app with the hardened runtime and appropriate narrow entitlements (in particular Node's JIT requirements). The final archive must be notarized, stapled and verified on a clean Mac. Review resource helper signing explicitly; merely signing the outer app is insufficient. [Tauri macOS signing](https://v2.tauri.app/distribute/sign/macos/), [Apple notarization requirements](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution).

Automatic updates additionally need an application-owned updater signing key pair, the public key embedded in the app, signed release artifacts and an HTTPS release feed. None are configured; adding a placeholder feed or silently generating production keys would not make updating functional. After release ownership and hosting are decided, add the Tauri updater plugin and publish authenticated update artifacts. [Tauri updater](https://v2.tauri.app/plugin/updater/).

The installed FFmpeg is compiled with GPL components (`--enable-gpl`, x264/x265). Retaining licenses and bottle recipes is useful provenance, but **does not itself fulfill corresponding-source distribution obligations**. Before public distribution, choose the intended FFmpeg build/licenses and provide the corresponding source and notices for the shipped components. No public release/upload is performed by these scripts. [FFmpeg license guidance](https://ffmpeg.org/legal.html).
