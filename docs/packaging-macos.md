# macOS packaging

Build on Apple Silicon macOS 26 or later with Xcode Command Line Tools,
Homebrew, and the mise tools listed in the [contribution guide](../CONTRIBUTING.md).

```sh
mise run setup:media
mise run package:mac
cd target/native-candidates
shasum -a 256 -c SHA256SUMS
codesign --verify --deep --strict Jacket.app
open Jacket.app
```

The package uses `com.moonlarkstudios.jacket` as its bundle identifier.
`MACOSX_DEPLOYMENT_TARGET` in [mise.toml](../mise.toml) supplies the deployment
target for the Rust build, runtime staging check and application plist.
The icon is stored in [the app's packaging directory](../apps/jacket/packaging/macos/).

Staging copies locally installed tools into `target/native-runtime/macos/audio-tools/`.
It relocates and ad-hoc signs the native dependencies, checks the runtime manifest
and runs offline smoke checks before packaging. The candidate contains those
tools under `Jacket.app/Contents/Resources/audio-tools/`, including Python and
Node. Its manifest records versions, installed source paths and file hashes.

The candidate uses ad-hoc signing for local testing. See the
[release checklist](releasing.md) for distribution signing, notarization,
corresponding source and clean-machine acceptance.
