# Artwork decoding and display quality

The native app uses `image 0.25.10`, with `image-webp 0.2.4` for WebP decoding.
This is image-rs's independent, memory-safe Rust implementation. Its
[release notes](https://github.com/image-rs/image-webp/blob/main/CHANGES.md)
document fixes for matching libwebp's filtering and bilinear chroma upsampling.

## Decoder comparison — November 3, 2026

Compared decoded pixels with Google's reference libwebp 1.6.0:

- The real YouTube WebP thumbnail associated with the reported song: identical.
- Four 257 × 193 reference-encoded fixtures covering lossy/lossless compression
  and opaque/alpha content: identical RGBA output for all four.

Maximum pixel-channel difference was zero in all five WebP samples. This is
sample evidence, not a claim of exhaustive decoder equivalence. The resolved
544 × 544 album image for that song is JPEG; its Rust/Pillow decoding difference
was 0.104 RMS on the 0–255 channel scale. The original example PNG decoded
identically with Rust, Pillow and FFmpeg. No dependency replacement was indicated.

## Display correction

The visible defect was reproduced with the original PNG example: the GPU was
shrinking a much larger texture with bilinear sampling and no mip levels. Fine
lettering broke apart and diagonal edges aliased. Remote previews also used the
fast integer `thumbnail` filter, which is not appropriate for this quality goal.

`artwork::Raster` now decodes and prepares Lanczos3 variants outside view
construction, at the physical pixel sizes required by the window's current
scale. These cover all six UI artwork sizes and the crop canvas. Filtering uses
premultiplied alpha to avoid colored fringes. Imported covers, source artwork,
export previews and crop previews use the same prepared-raster path.

Display changes rebuild variants from retained pixels, without downloading or
re-encoding. A suspended review refreshes its remote artwork after restoration
if the display density changed while it was suspended. Source identity, URL or
asset identity fences asynchronous results; an owned-raster cache holds at most
four entries. Encoded reads are capped at 10 MiB, decoder/RGBA allocations are
bounded, and the retained source is capped at 1024 pixels. Filtering never
invents a larger source image. Scale normalization is bounded to 0.5–4.0.

In a 184-pixel native PNG preview at 1× display scale, RMS difference against an
independently Lanczos-filtered Pillow reference fell from 6.243 to 0.152 on the
0–255 RGB scale. This measures that reproduction's sampling error; it is not a
general codec-quality score. Lettering and diagonal edges were visually improved.

## Verification

`mise run check` passed: 223 Rust tests, 11 tooling tests, strict Clippy,
rustfmt, workflow validation and dependency notices. The release build passed.
Seven native renders cover all six cover sizes and the crop canvas in desktop
and compact layouts. Seven raster tests exercise area-wide checkerboard
antialiasing, transparent edges, 1×/1.25×/2× dimensions, aspect ratio, source
identity, file limits and grayscale-to-RGBA memory bounds. Application tests
cover display changes, stale callbacks, cache bounds and restored reviews.

Native renders were exercised on Linux/Wayland at 1×. The generated variants
were tested at fractional and Retina densities; moving an installed application
between real HiDPI monitors and macOS display qualification remain platform tests.
Comparison artifacts are in `target/artwork-quality/`; codec probes are
throwaway files under `/tmp/jacket-codec-audit/`.
