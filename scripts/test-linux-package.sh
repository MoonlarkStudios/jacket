#!/bin/bash
# Run only inside a disposable x86_64 CI container, with /workspace read-only.
set -euo pipefail
cd /workspace
if command -v apt-get >/dev/null; then
  export DEBIAN_FRONTEND=noninteractive
  apt-get update
  package=$(find /packages -maxdepth 1 -name '*.deb' -print -quit)
  apt-get install -y xvfb xauth xdotool imagemagick dbus-x11 desktop-file-utils openbox

else
  package=$(find /packages -maxdepth 1 -name '*.rpm' -print -quit)
  dnf install -y rpm-build python3-rpm xorg-x11-server-Xvfb xorg-x11-xauth xdotool ImageMagick dbus-daemon util-linux shadow-utils desktop-file-utils openbox

fi
/usr/bin/python3 -B scripts/test-linux-package-versions.py
/usr/bin/python3 -B scripts/install-linux-upgrade-baseline.py
useradd --create-home preview
# Preserve representative private state in every application-owned location.
# Real preferences/recovery migrations still require the manual release checks.
for location in .cache .config .local/share; do
  runuser -u preview -- mkdir -p "/home/preview/$location/com.moonlark.audio-dl"
  printf '%s\n' upgrade-sentinel | runuser -u preview -- tee "/home/preview/$location/com.moonlark.audio-dl/package-test-marker" >/dev/null
done
if command -v apt-get >/dev/null; then
  apt-get install -y "$package"
  test "$(dpkg-query -W -f='${Architecture}' jacket)" = amd64
  apt-get install -y --reinstall "$package"
else
  dnf upgrade -y "$package"
  test "$(rpm -q --queryformat '%{ARCH}' jacket)" = x86_64
  rpm -U --replacepkgs "$package"
fi
for location in .cache .config .local/share; do
  test "$(cat "/home/preview/$location/com.moonlark.audio-dl/package-test-marker")" = upgrade-sentinel
done
test -x /usr/bin/jacket
test -s /usr/lib/Jacket/application-licenses/inventory.json
ldd /usr/bin/jacket | tee /evidence/"$PREVIEW_LABEL"-libraries.txt
if ldd /usr/bin/jacket | grep -q 'not found'; then exit 1; fi
desktop=$(find /usr/share/applications -iname '*jacket*.desktop' -print -quit)
test -n "$desktop"
desktop-file-validate "$desktop"
python3 -B scripts/stage-linux-tools.py --validate /usr/lib/Jacket/audio-tools
chmod 777 /evidence
runuser -u preview -- mkdir -p /home/preview/.cache/com.moonlark.audio-dl
runuser -u preview -- env PREVIEW_LABEL="$PREVIEW_LABEL" xvfb-run -a dbus-run-session -- python3 -B scripts/smoke-linux-app.py
if command -v apt-get >/dev/null; then
  apt-get remove -y jacket
else
  dnf remove -y jacket
fi
test ! -e /usr/bin/jacket
test ! -e /usr/lib/Jacket/audio-tools/bin/ffmpeg
test -e /home/preview/.cache/com.moonlark.audio-dl/package-test-marker
