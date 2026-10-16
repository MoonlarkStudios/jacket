# Linux preview

Jacket's Linux preview targets **x86_64**. CI builds on **Ubuntu 24.04** and tests the DEB on Ubuntu 24.04 and Debian 13, and the RPM on Fedora 44. These are preview candidates until the checks have passed for the exact commit. They are not a claim of compatibility with every Debian or RPM distribution, older systems, ARM, or sandboxed app stores.

## Install a preview candidate

Download the `linux-preview-x86_64` artifact from a successful [CI run](https://github.com/MoonlarkStudios/jacket/actions). Extract it and verify `sha256sum -c SHA256SUMS` in that folder. GitHub Actions artifacts expire; a public release must retain both installers and their source companions together.

Ubuntu / Debian:

```sh
sudo apt install ./Jacket_0.1.0_amd64.deb
```

Fedora:

```sh
sudo dnf install ./Jacket-0.1.0-1.x86_64.rpm
```

Use the actual filename from your download. Launch **Jacket** from your application menu. The package manager installs system dependencies. No Node, FFmpeg, yt-dlp, pip, Homebrew, or developer tools need to be installed by the user. New versions are installed with the same package-manager command. Whole-app automatic updates and signed package repositories are not configured.

The preview's supported workflow is **public YouTube links**. Browser-account downloads are experimental and require separate live acceptance; encrypted Chromium profiles may need additional Python/keyring components. The owned-session prototype remains disabled on Linux.

## What is shipped

- System libraries: GTK3, WebKitGTK 4.1, Python 3.10 or later, CA certificates, xdg-utils, and the base C/C++ runtime. The Ubuntu 24.04 build baseline requires a compatible glibc (2.39 or later); older Ubuntu/Debian versions are outside this preview.
- Private audio runtime under `/usr/lib/Jacket/audio-tools`: yt-dlp and matching EJS scripts, Node, FFmpeg/FFprobe, LAME and Opus. It uses isolated system Python without user site packages. Optional host media libraries are disabled during the FFmpeg build. The runtime includes its licenses, build settings and per-file hashes.
- The bundled downloader starts at the pinned release in `scripts/linux-runtime.lock.json`. Automatic downloader updates use the official `yt-dlp_linux` asset, independently of app updates. Updated copies live in the user's data directory; Jacket never modifies `/usr/bin` or the package-managed runtime. Local-install mode remains available.
- `jacket-linux-runtime-sources.tar.gz` contains exact upstream source archives and build recipes. `jacket-source.tar.gz` identifies the application source for the same commit. Distribute the installers with these companions, `runtime-build-info.json`, `build.json`, and `SHA256SUMS`.

## Build and automated checks

On x86_64 Ubuntu 24.04, install Tauri's [Linux build prerequisites](https://v2.tauri.app/start/prerequisites/#linux), plus `pkg-config`, `nasm`, `autoconf`, `automake`, `libtool`, `rpm`, and `desktop-file-utils`. Then:

```sh
mise trust
mise install
mise run package:linux
python3 scripts/collect-linux-preview.py
```

The staging script downloads only inputs named in the checked-in lock file and verifies SHA-256 before extraction or execution. C libraries are built from source without `-march=native`. Source archives and downloads are generated under `target/`; neither runtime nor installer bytes are committed. The packaging overlay is explicit so ordinary development and unit tests do not require generated resources.

CI checks application formatting, lint, unit tests, a release-mode GLib iterator regression, and six offline media tests against the **staged Linux tools**. It builds both installers, then installs/reinstalls/removes them in disposable Ubuntu, Debian and Fedora containers. A non-root release process opens and closes a real GTK/WebKit window under Xvfb, with screenshots and logs retained as evidence. The WebKit sandbox and hardware renderer are disabled **only inside these disposable headless tests**; shipped app settings are unchanged.

The [GLib backport](../vendor/README.md) applies the upstream fix for RUSTSEC-2024-0429. The original 0.18.5 version remains visible to dependency auditors; the advisory is not suppressed.

## Before publishing a broadly recommended Linux release

- Pass CI for the exact release commit and inspect package-test screenshots.
- On real GNOME and KDE desktops, verify Wayland and X11 rendering, scaling, keyboard access, native save/reveal dialogs, idle close, minimizing/reopening during downloads, cancellation, and restart/resume. Container/Xvfb checks do not establish this acceptance.
- Complete a live public download, check every advertised output format and artwork, test an offline restart, and verify downloader updating/rollback on the installed app.
- Test an upgrade from a previous preview version. CI currently proves installation, same-version reinstallation and removal with retained user data.
- Review the exact binary dependency/license inventory and retain corresponding sources for the distributed artifacts. CI creates the runtime source companion; the project's third-party overview is not a complete license audit.
- Keep browser-account reliability claims limited until Chromium/Firefox/keyring access, expiry and reconnect are tested on the supported distributions.

An initial public prerelease can explicitly invite testing of these remaining desktop scenarios. Do not describe a green headless build as a fully validated Linux release.
