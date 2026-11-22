#!/usr/bin/env bash
# Run with a release-only signing subkey in an isolated build environment.
set -euo pipefail
shopt -s nullglob
packages=("$1"/*.rpm)
[[ ${#packages[@]} -eq 1 ]]
fingerprint=${JACKET_RPM_FINGERPRINT:-}
fingerprint=${fingerprint^^}
if [[ ! $fingerprint =~ ^([0-9A-F]{40}|[0-9A-F]{64})$ || -z ${JACKET_RPM_PRIVATE_KEY:-} ]]; then
  printf 'The Jacket RPM signing key and full fingerprint must be configured.\n' >&2
  exit 1
fi

GNUPGHOME=$(mktemp -d)
export GNUPGHOME
trap 'gpgconf --kill gpg-agent; rm -rf "$GNUPGHOME"' EXIT
printf '%s\n' "$JACKET_RPM_PRIVATE_KEY" | gpg --batch --import
unset JACKET_RPM_PRIVATE_KEY
gpg --batch --list-secret-keys "$fingerprint" >/dev/null
gpg --batch --armor --export "$fingerprint" > "$1/jacket-rpm-signing-key.asc"

rpmsign --define '_openpgp_sign gpg' --define "_openpgp_sign_id $fingerprint" --addsign "${packages[0]}"
rpmkeys --dbpath "$GNUPGHOME/rpmdb" --import "$1/jacket-rpm-signing-key.asc"
rpmkeys --dbpath "$GNUPGHOME/rpmdb" --define '_pkgverify_level all' --checksig "${packages[0]}"
