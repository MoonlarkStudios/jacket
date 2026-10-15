# Archive manifest, version 1

Each archive has a sanitized top-level collection folder, containing audio files, `playlist.m3u`, `manifest.json`, and optional `cover.jpg` or `artwork/*.jpg`.

The manifest contains `schemaVersion: 1`, the user-selected `title`, collection `sourceUrl`, `mode` (`album` or `collection`), an ordered `tracks` array, and nonfatal `warnings`.

Each track records:

| Field                        | Meaning                                                                       |
| ---------------------------- | ----------------------------------------------------------------------------- |
| `sourceId`, `sourceUrl`      | Provider identity and original item URL. Repeated entries share these values. |
| `sourceMetadata`             | Music fields returned during inspection, before user edits.                   |
| `metadata`                   | Requested music fields after explicit user overrides.                         |
| `embeddedMetadata`           | Music fields embedded in the audio file using its container profile.          |
| `embeddedArtwork`            | Whether the selected artwork is embedded in the audio file.                   |
| `relativePath`               | Unique audio path relative to the collection folder.                          |
| `artworkPath`                | Image path relative to the collection folder, or `null`.                      |
| `sourceCodec`, `outputCodec` | Codecs reported by ffprobe before and after preparation.                      |
| `sha256`                     | SHA-256 of the completed, tagged audio file.                                  |

Metadata contains `title`, `artist`, `album`, `albumArtist`, `trackNumber`, `discNumber`, and `releaseYear`. Missing string values are empty strings; missing numeric values are `null`. Unknown source values are retained that way instead of inferred from upload metadata.

`embeddedMetadata` and `embeddedArtwork` are additive version-1 fields populated by new exports. Older manifests may omit them; their embedding status is unknown. WAV's RIFF INFO profile omits album artist and disc number, so these remain in `metadata` while the corresponding `embeddedMetadata` values are empty/null. Supplied omitted fields generate warnings in both the manifest and export result.

Source URLs are also written into the canonical comment field: MP3 `COMM`, M4A `©cmt`, FLAC/Opus/Vorbis `COMMENT`, or WAV `ICMT`. MP3 uses ID3v2.4 with an `APIC` front-cover frame; M4A uses `covr`; FLAC uses a picture block; Opus and Vorbis use `METADATA_BLOCK_PICTURE`. Separate image files remain available in the ZIP. WAV does not embed artwork.

The array's order preserves the selected source order. Filenames in collection mode carry sequential ordering for convenience; those prefixes do not imply music track-number tags. Album filenames prefer a real track number when provided. Filename collisions gain a `(2)` suffix. Identity alone should not be used to deduplicate repeated playlist entries.

The manifest does not include credentials, browser-cookie contents, temporary local download paths, or a matching claim against an external music catalog. SHA-256 identifies these exact output bytes; it is not an acoustic fingerprint.
