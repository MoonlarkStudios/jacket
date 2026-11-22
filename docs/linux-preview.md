# Linux packaging

## Install Jacket

Download the x86-64 package for your distribution and `SHA256SUMS` from
[GitHub Releases](https://github.com/MoonlarkStudios/jacket/releases). The
release notes list the desktop versions we tested. Keep one Jacket package in
the download directory when using the commands below.

```sh
sha256sum --ignore-missing -c SHA256SUMS
```

On Ubuntu or Debian, use the DEB.

```sh
sudo apt install ./jacket_*.deb
```

On Fedora Workstation, also download `jacket-rpm-signing-key.asc`. Compare the
fingerprint with the release notes before importing the key.

```sh
gpg --show-keys --fingerprint jacket-rpm-signing-key.asc
sudo rpmkeys --import jacket-rpm-signing-key.asc
rpmkeys --checksig ./jacket-*.rpm
sudo dnf install --setopt=localpkg_gpgcheck=1 ./jacket-*.rpm
```

Open Jacket from the app menu. The installer provides FFmpeg, ffprobe, Node,
and yt-dlp. Install a newer app version the same way to upgrade in place.

## Build DEB and RPM candidates

Stage the app, media tools, and matching sources on x86-64 Ubuntu 24.04. The
build needs the native prerequisites below, plus `dpkg-dev`. Use a fresh
output directory for each build.

```sh
mise run installers:stage
mise run installers:deb
```

Run the RPM assembly task on x86-64 Fedora with `rpm-build` installed, using
the same checkout and staged `target/linux-installers` directory.

```sh
mise run installers:rpm
mise run installers:seal
mise run installers:verify
```

Files for acceptance are under `target/linux-installers/assets`. The
[release checklist](releasing.md) covers signing, desktop acceptance, and
publication.

## Portable candidate

Build portable Jacket candidates on x86_64 Ubuntu 24.04, the CI baseline.
Install the native prerequisites in the [contribution guide](../CONTRIBUTING.md)
and the audio build tools `nasm`, `autoconf`, `automake` and `libtool`.

```sh
mise run package:linux
cd target/native-candidates
sha256sum -c SHA256SUMS
./jacket-linux/bin/jacket
```

Jacket needs a graphical X11 or Wayland session and working display drivers.
The portable bundle uses system Python 3, CA certificates, base C/C++ libraries,
and Zenity for the folder chooser and Save As. Install the included desktop entry and icon into their standard
locations or set absolute executable and icon paths before using the entry.

The native runtime staging script builds from the inputs and SHA-256 values in
[`scripts/linux-runtime.lock.json`](../scripts/linux-runtime.lock.json).
It places the runtime and its corresponding-source archive under
`target/native-runtime/linux/`. The package copies the audio tools to
`jacket-linux/resources/audio-tools/` and puts the matching
`jacket-linux-runtime-sources.tar.gz` beside the candidate archive. Keep it
with the application and dependency source archives and the checksum file.
