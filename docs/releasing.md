# Native release checklist

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
