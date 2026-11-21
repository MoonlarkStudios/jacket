# Linux packaging

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
The portable bundle uses system Python 3, CA certificates and base C/C++
libraries. Install the included desktop entry and icon into their standard
locations or set absolute executable and icon paths before using the entry.

The native runtime staging script builds from the inputs and SHA-256 values in
[`scripts/linux-runtime.lock.json`](../scripts/linux-runtime.lock.json).
It places the runtime and its corresponding-source archive under
`target/native-runtime/linux/`. The package copies the audio tools to
`jacket-linux/resources/audio-tools/` and puts the matching
`jacket-linux-runtime-sources.tar.gz` beside the candidate archive. Keep it
with the application and dependency source archives and the checksum file.

The [release checklist](releasing.md) covers desktop acceptance and publication.
