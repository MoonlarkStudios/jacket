#!/usr/bin/env bash
# Install a bundle into a disposable user Flatpak installation and test it.
set -euo pipefail

bundle=$1
version=$2
expected_executable_sha=$3
evidence=$(realpath "$4")
app_id=com.moonlark.audio-dl
resources=/app/libexec/jacket/resources
branch=stable
if [[ $version == *-* ]]; then branch=beta; fi
ref="$app_id/x86_64/$branch"
mkdir -p "$evidence"

flatpak install --user --noninteractive --reinstall --bundle "$bundle"
test "$(flatpak info --user --show-ref "$ref")" = "app/$ref"
test "$(flatpak run --user "$ref" --version)" = "Jacket $version"
test "$(flatpak run --user --command=sha256sum "$ref" /app/libexec/jacket/bin/jacket |
  cut -d ' ' -f1)" = "$expected_executable_sha"

flatpak run --user --command=sh "$ref" -eus -- "$resources" <<'SCRIPT'
  bin=$1/audio-tools/bin
  "$bin/node" --version
  "$bin/yt-dlp" --version
  "$1/catalog-tools/bin/youtube-music" --version
  zenity --version
  test -s "$1/notices/THIRD_PARTY_NOTICES.md"
  fixture=$(mktemp -d)
  trap 'rm -rf "$fixture"' EXIT
  "$bin/ffmpeg" -hide_banner -loglevel error -f lavfi -i sine=duration=0.2 \
    -c:a libmp3lame -metadata title="Jacket Flatpak smoke test" "$fixture/test.mp3"
  test "$("$bin/ffprobe" -v error -show_entries format_tags=title \
    -of default=noprint_wrappers=1:nokey=1 "$fixture/test.mp3")" = "Jacket Flatpak smoke test"
SCRIPT

timeout 90s flatpak run --user --filesystem="$evidence" "$ref" --screenshot "$evidence/jacket.png"
test -s "$evidence/jacket.png"
printf 'Verified Flatpak install, bundled tools, audio encoding and rendered window.\n'
