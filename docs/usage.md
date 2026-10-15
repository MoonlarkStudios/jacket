# Using Jacket

[Back to the README](../README.md)

## Find tracks

Open the desktop app and paste an HTTPS YouTube or YouTube Music link. Jacket supports video links, playlists, and YouTube Music albums. Pasting a complete supported link starts inspection; when typing, press Enter or choose **Find tracks**.

Tracks appear as inspection progresses. Wait for inspection to finish before editing or downloading. Unavailable or incomplete entries are reported. If you cancel or inspection fails, the partial preview stays visible; choose **Inspect again** to retry.

![Jacket’s link-entry screen](screenshots/start.jpg)

The browser preview uses **Try example** to demonstrate the interface. It cannot inspect real sources or export audio.

## Review an album or playlist

Choose the tracks you want. Search appears for sources with more than eight tracks, and matches title, artist, and album. Playlist order and repeated entries are retained.

**Edit album details** opens a draft for shared album title, album artist, release year, organization, and ZIP filename. Choose **Save changes** to apply it. Cancel, Escape, and closing the dialog discard the draft. The ZIP name follows the album title until you give it a custom name.

- **Album** organization applies supplied shared album, album artist, and year values. Each track retains its own artist and track/disc numbers. Missing track numbers are filled from source order within each disc; deselecting tracks leaves gaps.
- **Playlist** organization keeps individual music metadata. File-order prefixes do not invent track-number tags.

Use a track’s edit button to correct its title, artist, or other fields. Inherited values are labeled **From album details**. Shared album values take precedence at export; use the editor’s album-details action when you need to change those. **Save & next** continues to the next track.

Unknown artists and years stay unknown. Jacket does not treat uploaders as music artists or upload dates as release years. Recognized YouTube Music albums may supply a verified album-header year; review it before exporting.

## Choose artwork and format

Click the cover to review source-image candidates or choose no artwork. Source images may be video thumbnails rather than official album covers. You can use a shared cover or each track’s own image. Arbitrary local-image upload and external cover searches are not implemented.

**Download options** contains the format choice:

| Option | Behavior |
| --- | --- |
| M4A / MP3 | Keeps AAC/ALAC in M4A; otherwise copies existing MP3 or converts to MP3. |
| Keep source codec | Copies supported AAC, ALAC, MP3, Opus, Vorbis, FLAC, and PCM audio into a suitable container. Unsupported codecs require another preset. |
| MP3 | Copies MP3 sources; converts other audio to MP3. |

MP3, M4A, FLAC, Opus, and Ogg Vorbis support embedded artwork. WAV uses a separate image, and its limited tags omit album artist and disc number. Warnings and the manifest identify what could not be embedded. Changing containers or metadata does not improve the source audio quality.

## Save your archive

Choose **Download album…** or **Download playlist…**. The native save dialog suggests a filename and your last chosen folder. Cancelling the dialog starts no download. Existing ZIPs are preserved; collisions receive a numbered suffix.

Progress shows prepared tracks and the current operation. Packaging the ZIP is a separate stage. On success, reveal the archive in your file manager, extract it, and open `playlist.m3u` with a player that supports the exported audio formats. Keep the playlist beside its tracks.

The archive includes tagged audio, available images, a relative-path playlist, and `manifest.json`. The [manifest reference](manifest.md) explains source metadata, effective exported tags, warnings, and hashes.

## Interrupted downloads

Cancel stops the operation without publishing an incomplete ZIP. Completed source downloads remain cached for retry. Transient network errors receive a bounded automatic retry; account restrictions and unavailable media stop with guidance.

After restarting, Jacket offers the most recently active unfinished export as **Resume download**. It restores selected tracks, edits, format, artwork, and destination. Retry or resume reuses completed cached source downloads, though audio preparation may run again. Changing the archive name opens a new save dialog. Use **Choose another destination** if the original folder is unavailable or out of space.

**New download** returns to the link screen. **Undo new download** restores your previous complete review until a new source loads successfully. Dismissing a recovery offer retains its media cache.

On macOS, closing the window hides Jacket and preserves the current operation. Reopen it from the Dock. Use **Quit Jacket** to exit; unfinished exports remain recoverable. There is no tray icon.

## Browser sessions

Public links need no browser setup. For content requiring your account:

1. Open the link in Chrome, Firefox, Edge, or Safari and confirm you can play it there.
2. In Jacket, open **Settings → Browser session**.
3. Choose that browser and the exact named profile. Firefox containers are listed separately; ordinary tabs use **Tabs outside containers**. Safari named profiles are not supported by this importer.
4. Choose **Use this session**. Optionally test the relevant link with **Test access**.

Testing inspects the first source item without downloading audio. A passing test on a public link does not prove account-only access. Browser decryption and operating-system permissions can prevent access even when browser playback works. Jacket never switches to a different profile automatically. **Refresh profiles** reads profile metadata; cookie access happens during an explicit test or job.

To reconnect, sign in normally in the selected browser profile, return to Jacket, and test again. **Stop using this session** returns Jacket to public access without signing out of the browser. New work starts publicly after restart; unfinished jobs retain their profile identifier for explicit resume. Changing browsers can change account access.

## Downloader updates

**Settings → Audio downloader** shows the active yt-dlp version. Automatic updates are enabled by default and use the official nightly channel. Updates wait for the current job before activation. A failed update keeps the current downloader; **Check now** retries the check.

Advanced settings let you select a release channel or return to the bundled version and pause updates. **Local install** lets you choose your own yt-dlp executable; Jacket validates its version and YouTube support and does not manage that executable. Only select a file you trust.

These updates affect yt-dlp, not Jacket, FFmpeg, or Node. Managed downloader updates currently support macOS only.

## Local data and cleanup

Jacket stores format and save-folder preferences locally. The downloader’s settings and downloaded versions live in the application data directory. Unfinished-export records and cached media live in the application cache. Browser cookies are not included in recovery records or archives. Archives do include the media’s source links and metadata; review those before sharing an archive.

On macOS:

| Location | Contents |
| --- | --- |
| `~/Library/Caches/com.moonlark.audio-dl/jobs/` | Unfinished/dismissed job records, source downloads, artwork, and prepared audio. |
| `~/Library/Application Support/com.moonlark.audio-dl/downloader/` | Downloader preferences and managed versions. |

The original `com.moonlark.audio-dl` identifier is retained for compatibility. Successful exports remove their job directory; failed, cancelled, and dismissed jobs can keep substantial media files. There is no cache-cleanup screen yet. To reclaim space, quit Jacket, open the cache location in Finder, and move unwanted job folders to Trash. Removing a job folder also removes its resume state and cached downloads. Your completed ZIPs are stored wherever you chose to save them.

## Troubleshooting

| Symptom | Next step |
| --- | --- |
| A previously working source fails | Check for a downloader update, then retry. Provider behavior can change. |
| “Sign in” or account access error | Confirm browser playback, select the exact profile, and test that link. |
| Browser session cannot be unlocked | Follow the operating-system permission guidance or try another signed-in supported browser. Keep browser encryption enabled. |
| Browser cookie database is busy | Close the selected browser yourself and retry. |
| Requests are rate limited | Wait before retrying. Repeated sign-in or profile changes may not help. |
| Artwork is missing | Review source images; artwork can fail independently of audio. Check export warnings. |
| An audio tool is missing | For source development, run `mise run setup` and `mise run doctor`. For a packaged app, rebuild or reinstall the complete bundle. |
| Save location is unavailable | Choose another destination and ensure enough free space for both cached media and the final ZIP. |

If the problem persists, [open an issue](https://github.com/MoonlarkStudios/jacket/issues/new/choose). Include the app/commit and downloader versions, operating system, and steps. Share a public reproduction link only when appropriate; redact private links, account details, and local paths.
