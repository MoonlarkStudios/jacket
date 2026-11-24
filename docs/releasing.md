# Release Jacket for Linux

The workspace version in `Cargo.toml` drives the About screen, `jacket --version`,
package metadata, and the Git tag. Start with a release PR that updates that version,
all workspace entries in `Cargo.lock`, and the matching section in `CHANGELOG.md`.
Use `0.2.0-rc.1` for the first 0.2.0 release candidate and `0.2.0` for the stable release.
Increment the prerelease counter for a replacement candidate. Increment the patch
version for a fix after stable.

## Prepare acceptance

1. Keep the GitHub branch rules on `main` and restrict creation and updates of
   `v*` tags to release maintainers. Enable immutable releases.
2. Create the `rpm-signing` Actions environment, restrict it to release tags,
   and configure its `JACKET_RPM_PRIVATE_KEY` secret and
   `JACKET_RPM_FINGERPRINT` variable. Use an armored release-only signing subkey.
   Keep the primary private key offline. The workflow imports the key only in the
   signing container and verifies each RPM with a fresh key database.
3. Require Native CI and review the Linux installer workflow before tagging.
   The installer workflow tests both packages on clean Ubuntu 24.04 and 26.04,
   Debian 13, and Fedora 44 and 45 containers. It also installs the published
   beta and checks replacement and profile preservation on Ubuntu 24.04 and
   Fedora 44.

## Stage and publish

1. After the release PR merges, create an annotated `v<version>` tag on that
   commit. The **Linux installers** workflow validates the tag and exact commit
   on `main`, builds both packages, signs the RPM, and exercises the final assets.
   Passing artifacts and their provenance go into an unpublished GitHub release.
2. Install those exact signed assets on clean Ubuntu and Fedora Workstation
   Wayland desktops. Record the package checksums, distro, session, and result.
   Launch from the app menu. Check the artwork and About version. Review a track,
   change the music root, save with the resolved artist/album/song rule, cancel
   Save As, and then complete a one-off save. Verify tags and artwork on disk.
   Preview an album with one track already in the library. Cancel while adding
   the remaining tracks, restart Jacket, and resume the save. Verify the existing
   audio is preserved and the finished album has its playlist and manifest.
   Verify the app can reopen a saved draft after upgrading. Verify one permitted
   public source separately on both desktops.
3. Review the [third-party notices](../THIRD_PARTY_NOTICES.md) and retain the
   matching application, Cargo dependencies, and audio-runtime source assets
   alongside the two installers. Match the draft's assets to the tested manifest.
   Publish the draft once the acceptance record passes. The follow-up workflow
   checks immutable release identity, anonymous downloads, checksums, and
   build provenance.

If acceptance fails, fix it on `main` and create a new prerelease or patch version.
Keep the published tag and its files immutable. If a draft upload was interrupted,
review the partial draft before deleting it and rerunning the same workflow.

## Portable candidates

Use the manual **Build native candidates** workflow to build the exact revision
on Apple Silicon macOS and x86_64 Linux. It runs the native quality gate, stages
the platform runtimes, checks package hashes and uploads the candidate archives
for review.

- [ ] Follow the platform guides to validate the candidate and the accompanying
  checksums. See [macOS packaging](packaging-macos.md) and
  [Linux packaging](linux-preview.md).
- [ ] Record desktop acceptance on the exact artifact for each advertised
  platform. Include clean-machine startup, identity/icon, native dialogs,
  keyboard access, resize, export, cancellation and recovery. See the
  [native qualification matrix](native-port.md#acceptance-record).
- [ ] Verify that the app discovers its bundled tools and can export using them
  on a machine without developer-installed audio tools.
- [ ] Review the bundled binary dependencies and notices. Include the exact
  corresponding source for distributed GPL/LGPL components, including applicable
  macOS runtime components. See the [third-party notices](../THIRD_PARTY_NOTICES.md).
- [ ] For public macOS distribution, apply Developer ID signing to the release
  bundle and its helpers with the required entitlements. Notarize, staple and
  verify the signed artifact on a clean Mac.
- [ ] Verify final release archive checksums after any signing or repackaging
  and publish the matching notices and source archives with the binaries.

Earlier releases retain their installer tooling and verification instructions
in their source tags.
