# Jacket identity

Jacket is a focused desktop utility for downloading audio albums and playlists and packaging tagged audio, artwork, and supporting files. Its symbol combines a record sleeve with a narrow vertical paper band.

**Descriptor:** Download and package albums and playlists.

**Tagline:** From link to neatly packed.

## Artwork

The app-icon master uses the approved **1254 × 1254** canvas. Transparent margins surround the charcoal tile. The refined symbol exposes more than half the record's diameter beyond the copper sleeve and uses two broad sage groove arcs, a small cream center dot, and a narrower cream band. Flat vector fills replace the generated reference's surface texture. The approved visual reference is preserved in `source/approved-jacket-icon.png`.

- `svg/jacket-app-icon.svg`: full-detail vector master; intrinsic dimensions 1254 × 1254.
- `svg/jacket-app-icon-small.svg`: optical master for 16–32 px; same canvas and symbol placement, with two heavier sage grooves, a slightly stronger band and record edge, and a larger center dot.
- `svg/jacket-icon-{size}.svg`: SVGs with exact intrinsic dimensions of 16, 24, 32, 64, 128, 256, 512, and 1024 px. The 16, 24, and 32 px files use the optical variant.
- `png/jacket-icon-{size}.png`: raster exports at those sizes plus 1254 px; transparent outside the tile.
- `svg/jacket-mark.svg` and `jacket-mark-dark.svg`: flat color symbols.
- `svg/jacket-logo-light.svg` and `jacket-logo-dark.svg`: horizontal logo lockups, 1200 × 400.
- `svg/jacket-logo-stacked-light.svg` and `jacket-logo-stacked-dark.svg`: stacked lockups, 660 × 690.
- `svg/jacket-wordmark-{ink,paper}.svg`: outlined wordmark.
- `svg/jacket-mark-mono-{ink,paper}.svg` and `jacket-logo-mono-{ink,paper}.svg`: single-color artwork with transparent cutouts.

All SVG artwork is self-contained, with real vector paths, no raster images, no external resources, and no font requirement. The app icons and flat marks use solid fills without gradients, shadows, or texture. Single-color variants express the grooves and band as transparent cutouts.

## Color and type

| Token | Color | Role |
| --- | --- | --- |
| Ink | `#20221E` | Text, wordmark, record |
| Paper | `#F3EFE5` | Surfaces, packaging band, reverse wordmark, center dot |
| Copper | `#D47746` | Sleeve and primary brand accent |
| Groove | `#B4C5A6` | Sage record grooves |
| Tile | `#282B25` | Charcoal app-icon tile |
| Muted | `#787369` | Secondary text on paper |

Copper supplies the brand accent. Use Ink for small text on Paper; Copper on Paper is decorative rather than a small-text pairing. Use Ink text for Copper-filled controls.

The wordmark uses supplied outlines derived from Avenir Next Demi Bold, with adjusted tracking. Font files are not included or required to render the logos. UI and body copy use the system sans-serif stack: `-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif`.

## Usage

Allow at least one packaging-band width of clear space around the visible artwork. Keep the horizontal logo at least 180 px wide and the standalone symbol at least 32 px wide. For smaller placements, use the optical app icon.

Use the full app icon in desktop launchers and application lists. Use the flat logo in headers, product pages, documentation, and print. Light variants suit Paper or white surfaces; dark variants suit Ink surfaces. Single-color files are appropriate when color is unavailable. Preserve the sleeve notch and vertical band; stretch neither the symbol nor the wordmark.

**Voice:** direct, calm, and specific. Examples: Find tracks; Review details; Download album; Download playlist; Your package is ready. Avoid copy that implies playback, discovery, or an in-app music library.

## Review and rebuild

Open `index.html` to review the identity and exact-size PNG exports on light and dark backgrounds, including the 16, 24, and 32 px sizes. The HTML uses only local assets. `previews/jacket-small-sizes.png` shows those sizes alongside enlarged pixel previews. `brand-tokens.json` records colors, typography, and geometry. The wordmark and product typography remain the existing outlined Avenir Next and system sans-serif.

Run `node source/build.mjs` to regenerate SVG artwork. To regenerate PNGs as well, set `SHARP_MODULE` to an installed Sharp package path before running the same command. `source/wordmark-outline.json` contains the wordmark paths; regeneration does not require the original font. `source/outline-wordmark.swift` records how those outlines were extracted through CoreText on macOS.

The application uses these assets through `scripts/sync-branding.py` (`mise run branding:sync`). Regenerate the PNG exports after editing the SVG sources, then sync to update `public/branding` and `src-tauri/icons`. Tauri's icon generator creates the native containers, with the optical artwork substituted into the small ICNS entries. The app retains the original bundle identifier and preference namespace for compatibility with existing installations.
