#!/usr/bin/env bash
# Run as root in a disposable distribution container. The workspace and
# packaged assets are mounted read-only. Jacket runs as an ordinary user.
set -euo pipefail
cd /workspace
shopt -s nullglob
export DEBIAN_FRONTEND=noninteractive

if command -v apt-get >/dev/null; then
  format=deb
  candidates=(/packages/*.deb)
  apt-get update -qq
  apt-get install -y --no-install-recommends python3 xvfb xauth xdotool xclip wmctrl dbus-x11 openbox desktop-file-utils appstream imagemagick passwd util-linux
  install_candidate() { apt-get install -y --no-install-recommends "$1"; }
  uninstall() { apt-get remove -y jacket; }
  installed_version() { dpkg-query -W -f='${Version}' jacket; }
  package_version() { dpkg-deb -f "$1" Version; }
  verify_version_order() { dpkg --compare-versions "$1" lt "$2"; }
else
  format=rpm
  candidates=(/packages/*.rpm)
  dnf install -y --setopt=install_weak_deps=False python3 python3-rpm xorg-x11-server-Xvfb xorg-x11-xauth xdotool xclip wmctrl dbus-daemon openbox desktop-file-utils appstream ImageMagick shadow-utils rpm-sign util-linux
  local_gpg_check=0
  if [[ -f /packages/jacket-rpm-signing-key.asc ]]; then
    rpmkeys --import /packages/jacket-rpm-signing-key.asc
    rpmkeys --checksig "${candidates[0]}"
    local_gpg_check=1
  fi
  install_candidate() { dnf install -y --setopt=install_weak_deps=False --setopt="localpkg_gpgcheck=$local_gpg_check" "$1"; }
  uninstall() { dnf remove -y --setopt=clean_requirements_on_remove=False jacket; }
  installed_version() { rpm -q --queryformat '%{VERSION}-%{RELEASE}' jacket; }
  package_version() { rpm -qp --queryformat '%{VERSION}-%{RELEASE}' "$1"; }
  verify_version_order() {
    python3 - "$@" <<'PY'
import rpm
import sys

previous, candidate = (('0', *value.split('-')) for value in sys.argv[1:])
assert rpm.labelCompare(previous, candidate) < 0, (previous, candidate)
PY
  }
fi

[[ ${#candidates[@]} -eq 1 ]]
package=${candidates[0]}
version=$(python3 -c 'import json; print(json.load(open("/packages/build.json"))["version"])')
versions() {
  python3 -B -c 'import sys; sys.path.insert(0, "scripts"); from native_installers import package_versions; print(*package_versions(sys.argv[1]))' "$1"
}
read -r deb_version rpm_version rpm_revision < <(versions "$version")

# Assert the stable line sorts above its release candidates using the real
# package manager's comparator. The release job also tests an installed update.
read -r rc_deb rc_rpm rc_revision < <(versions "${version%%-*}-rc.1")
read -r final_deb final_rpm final_revision < <(versions "${version%%-*}")
if [[ $format = deb ]]; then
  expected_version=$deb_version
  verify_version_order "$rc_deb" "$final_deb"
else
  expected_version=$rpm_version-$rpm_revision
  verify_version_order "$rc_rpm-$rc_revision" "$final_rpm-$final_revision"
fi
test "$(package_version "$package")" = "$expected_version"

useradd --create-home jacket-test
home=/home/jacket-test
data=$home/.local/share/com.moonlark.audio-dl
evidence=$(mktemp -d)
chown jacket-test "$evidence"
save_evidence() {
  mkdir -p /evidence
  cp -r "$evidence"/. /evidence/
  rm -rf "$evidence"
}
trap save_evidence EXIT
install_candidate "$package"
test "$(installed_version)" = "$expected_version"
test "$(jacket --version)" = "Jacket $version"
test "$(readlink -f /usr/bin/jacket)" = /usr/libexec/jacket/bin/jacket
test -z "$(command -v ffmpeg || true)"
test -z "$(command -v yt-dlp || true)"
test -z "$(command -v node || true)"
test "$(sha256sum /usr/bin/jacket | cut -d ' ' -f1)" = "$(python3 -c 'import json; print(json.load(open("/packages/build.json"))["executableSha256"])')"
desktop-file-validate /usr/share/applications/com.moonlark.audio-dl.desktop
appstreamcli validate --no-net /usr/share/metainfo/com.moonlark.audio-dl.metainfo.xml
test -s /usr/share/icons/hicolor/256x256/apps/com.moonlark.audio-dl.png
test -s /usr/share/licenses/jacket/LICENSE
test -s /usr/libexec/jacket/resources/notices/THIRD_PARTY_NOTICES.md
libraries=$(ldd /usr/bin/jacket)
[[ $libraries != *'not found'* ]]
python3 -B scripts/stage-linux-tools.py --validate /usr/libexec/jacket/resources/audio-tools

runuser -u jacket-test -- env HOME="$home" xvfb-run -a -s '-screen 0 1280x900x24' dbus-run-session -- \
  python3 -B scripts/smoke_linux_install.py "$evidence"
uninstall
test ! -e /usr/bin/jacket
test ! -e /usr/libexec/jacket/resources/audio-tools/bin/ffmpeg
test ! -e /usr/share/applications/com.moonlark.audio-dl.desktop
test -s "$data/music-folder.json"
test -s "$data/review-library/draft/review.json"
test -s "$home/smoke/music/The Paper Trails/Room to breathe/First light.mp3"

baselines=(/baseline/*."$format")
if [[ ${#baselines[@]} -gt 0 ]]; then
  [[ ${#baselines[@]} -eq 1 ]]
  if [[ $format = deb ]]; then
    apt-get install -y "${baselines[0]}"
  else
    # The archived beta predates package signing. Its published digest was
    # verified before being mounted. The candidate uses its own trust policy.
    dnf install -y --setopt=localpkg_gpgcheck=0 "${baselines[0]}"
  fi
  previous=$(installed_version)
  verify_version_order "$previous" "$expected_version"
  sha256sum "$data/music-folder.json" "$data/review-library/draft/review.json" > "$evidence/profile.sha256"
  install_candidate "$package"
  test "$(installed_version)" = "$expected_version"
  test "$(jacket --version)" = "Jacket $version"
  sha256sum --check "$evidence/profile.sha256"
  uninstall
  printf 'Verified upgrade from %s to %s\n' "$previous" "$expected_version"
fi
