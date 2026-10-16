<!-- jacket-release-template:v1 -->
Jacket **{{version}}** is a **Linux x86_64 {{release_kind}}**. Turn a public YouTube or YouTube Music album or playlist into tagged audio, artwork and a portable playlist, together in one ZIP.

![Jacket reviewing its fictional example album]({{raw_source_url}}/docs/screenshots/review.jpg)

*The screenshot shows Jacket’s fictional example album.*

<!-- jacket-downloads:start -->
## Download and install

Download the matching installer from **Assets** below. This works for maintainers reviewing a draft and for everyone after publication.

| Distribution | Asset filename |
| --- | --- |
| Ubuntu 24.04 / Debian 13 | `{{deb_filename}}` |
| Fedora 44 | `{{rpm_filename}}` |

On Ubuntu or Debian, open a terminal in your download folder and run:

```sh
sudo apt install ./{{deb_filename}}
```

On Fedora:

```sh
sudo dnf install ./{{rpm_filename}}
```

Launch **Jacket** from your application menu. Your package manager installs system dependencies; Node, FFmpeg/FFprobe and yt-dlp are included with Jacket.

These packages target Intel/AMD 64-bit Linux, built on Ubuntu 24.04 with glibc 2.39 or later. Older distributions and Linux ARM are outside this release’s tested scope. This release contains Linux installers only.
<!-- jacket-downloads:end -->

## What changed

{{changes}}

## Tested scope and known limitations

Automated checks cover Ubuntu 24.04, Debian 13 and Fedora 44: package installation, package-manager upgrades/reinstallation, retained user files, removal, and native window launch/idle close under Xvfb. Offline media tests exercise the bundled audio tools, codecs, tags, artwork and archives.

Real-desktop acceptance is still pending for the following scenarios:

- GNOME/KDE on Wayland and X11: rendering, scaling, keyboard access and native save/reveal dialogs.
- Full live in-app downloads, cancellation, busy-window behavior, offline restart and recovery.
- Compatibility with an earlier app’s actual preferences and unfinished jobs; automated upgrades use a disposable packaging fixture.

Public links are the focus of this Linux release. Browser-account downloads are experimental. Automatic whole-app updates and signed APT/DNF repositories are not configured; install future Jacket versions through your package manager. The optional yt-dlp updater runs separately from app updates.

<!-- jacket-verification:start -->
## Verify downloads and inspect sources

Download `SHA256SUMS` alongside your chosen assets. From that folder, verify the files you downloaded with:

```sh
sha256sum --ignore-missing -c SHA256SUMS
```

Matching application, audio-runtime and dependency source archives are attached, along with license notices, a dependency inventory and build information. Keep the complete asset set together when redistributing. Jacket is licensed under [GPL-3.0-or-later]({{source_url}}/LICENSE).

Built from [`{{sha}}`]({{commit_url}}). [Automated build, security and package checks]({{run_url}}) passed.

[User guide]({{source_url}}/docs/usage.md) · [Linux preview guide]({{source_url}}/docs/linux-preview.md)
<!-- jacket-verification:end -->

## Help shape the next release

[Report an issue]({{repo_url}}/issues/new/choose) with your distribution/version, desktop, Wayland or X11 session, Jacket version, and steps to reproduce. Remove private URLs and account details from reports. Use the [security policy]({{source_url}}/SECURITY.md) for vulnerabilities.
