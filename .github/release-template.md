<!-- jacket-release-template:v1 -->
Jacket **{{version}}** · **Linux x86_64 {{release_kind}}**

<!-- jacket-downloads:start -->
## Download and install

Download your installer from **Assets** below, then run:

| Distribution | Install command |
| --- | --- |
| Ubuntu 24.04 / Debian 13 | `sudo apt install ./{{deb_filename}}` |
| Fedora 44 | `sudo dnf install ./{{rpm_filename}}` |

Requires glibc 2.39+. Audio tools are included.
<!-- jacket-downloads:end -->

## What changed

{{changes}}

## Limits

Desktop acceptance remains pending for GNOME/KDE Wayland/X11, live exports/recovery, and upgrades of real preferences/jobs. Browser-account access is experimental; app updates are manual.

<!-- jacket-verification:start -->
## Verify

Download `SHA256SUMS` and run `sha256sum --ignore-missing -c SHA256SUMS`. Sources, notices and build information are in **Assets**.

[Build commit]({{commit_url}}) · [Passed CI]({{run_url}}) · [GPL-3.0-or-later]({{source_url}}/LICENSE)
<!-- jacket-verification:end -->

[User guide]({{source_url}}/docs/usage.md) · [Linux details]({{source_url}}/docs/linux-preview.md) · [Report an issue]({{repo_url}}/issues/new/choose) · [Security]({{source_url}}/SECURITY.md)
