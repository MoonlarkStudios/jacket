# Optional YouTube session investigation

Updated 2026-10-15. Normal Settings now use existing browser profiles. The app-owned browser remains an opt-in development experiment; its complete live authentication lifecycle has not been verified.

## Current product decision

Keep public access as the default. Users explicitly choose an existing browser/profile (or a Firefox container), test a link, and select **Use this session**. Rust discovers readable profile metadata and returns opaque IDs plus display labels. It resolves the selected ID again for each test, inspection, and export; deleted or missing profiles produce an actionable error instead of selecting the most recently accessed account. Cookie values and resolved paths stay outside the frontend and recovery records. Firefox defaults to non-container tabs unless a named container is selected. Safari exposes its available cookie store and explains that named profile selection is unsupported.

An access test extracts only the first item, skips media downloading, and creates no cookie export. A public-link success is explicitly not account verification. Tests can be cancelled, and normal jobs cannot overlap a test. Failures distinguish cookie expiry, Keychain/decryption, OS permissions, a locked database, missing profiles, and rate limiting; an ordinary 403 does not automatically recommend account sign-in. Reconnection means signing in again in the same browser profile and testing again. **Stop using this session** only clears Jacket’s selection, leaving the browser untouched. New work starts anonymously after restarting, while an explicit unfinished-download resume retains its exact browser/profile ID.

The app-owned prototype is absent from normal Settings and normal recovery. To expose its separate development dialog, launch a debug build with `JACKET_AUTH_PROTOTYPE=1` and choose **Developer session prototype…** from the Jacket menu. Normal startup does not restore its selection or read its Keychain item. The native prototype includes an isolated Chrome launch, YouTube-only cookie capture, local nonsynchronizing Keychain storage, private temporary handoffs, restart cleanup, and Disconnect. Synthetic tests passed for persistence/reconstruction, old-session rejection, expiry, cancellation, and cleanup. A separate synthetic macOS Keychain write/read/delete test also passed outside the filesystem sandbox. A local debug app was opened and its isolated sign-in flow initiated; full authenticated extraction and the live restart/reconnect/disconnect sequence remain unverified. Existing experiment credentials are not automatically deleted when the feature is hidden.

The investigation and original prototype plan below are retained as historical design evidence.

Validation of the existing-browser flow: 37 frontend tests and 57 Rust unit tests passed, along with TypeScript, Biome, Clippy, formatting, and a macOS debug app build. The native Settings screen showed four installed Chrome profiles by display name and profile directory, with no app-owned-session controls. Public access remained the startup default. After the user explicitly chose **John · Profile 2**, that exact profile was attached and **Test access** passed for the supplied YouTube Music album. The UI correctly stated that a public-link result does not prove account-only access. **Stop using this session** cleared the selection; closing Settings showed **Public playlists need no setup** again. No media was downloaded by the test, and stopping session use did not sign out or modify Chrome. Real private-content access and reconnecting after actual expiry remain unverified. Firefox, Safari, Windows, and Linux discovery are covered by synthetic filesystem fixtures, not live platform verification.

## Original experiment decision (2026-10-14)

**Go:** keep anonymous public downloads as the default and prototype an optional session created in a separate, ordinary browser profile owned by Jacket. The user signs in through the browser's normal interface; the app never touches their personal profile.

**No-go:** ship a “Connect YouTube” feature implemented with Google OAuth in the Tauri webview, or promise a fully embedded login that already works. Google's documented desktop OAuth flow uses an external browser, and yt-dlp's YouTube OAuth download login is unsupported. A normal Google website login inside WKWebView has not been tested here; the OAuth restriction does not prove every website login fails, but it does make that route a poor foundation for a seamless utility. No user-agent spoofing or workaround for a rejected sign-in is proposed.

**Release gate:** prove real, user-operated sign-in, authenticated extraction, isolated persistence, expiry recovery, and local disconnect before replacing the current browser connection. A single successful public download cannot establish any of these.

## Evidence and current behavior

| Finding | Evidence level | Source |
| --- | --- | --- |
| Jacket currently offers public access or a selected Chrome/Firefox/Safari/Edge session. `AuthOptions` contains only an optional browser enum; `source::downloader` passes `--cookies-from-browser` plus the browser name. No managed profile or credential store exists. | Inspected local code | `crates/audio-engine/src/models.rs`, `crates/audio-engine/src/source.rs`, `src/components/WorkspaceDialogs.tsx` |
| An explicit profile path is supported by yt-dlp. Its documented unspecified-profile behavior selects the most recently accessed profile. Our current “default profile” description is therefore too precise. | Current primary documentation | [yt-dlp authentication options](https://github.com/yt-dlp/yt-dlp#authentication-options) |
| Cookies are the supported YouTube account mechanism; YouTube OAuth login no longer works. The project describes account use as necessary for account-restricted content, warns about account risk, cookie rotation, and separately documents PO tokens. Connecting a session therefore cannot promise to solve every 403 or playback restriction. | Current primary documentation | [yt-dlp YouTube extractor notes](https://github.com/yt-dlp/yt-dlp/wiki/Extractors) |
| Desktop OAuth authorizations must use the system browser; Google documents `disallowed_useragent` for authorization requests in WKWebView. The policy prohibits OAuth in a developer-controlled embedded user agent. | Current primary documentation | [Google native-app OAuth](https://developers.google.com/identity/protocols/oauth2/native-app), [Google OAuth policy](https://developers.google.com/identity/protocols/oauth2/policies#use-secure-browsers) |
| Chrome can launch using a custom `--user-data-dir`, including on macOS, separate from its usual profiles. That profile contains its own cookies, history, and other browser state. | Current primary documentation; not launched in this investigation | [Chromium user data directory](https://chromium.googlesource.com/chromium/src/+/HEAD/docs/user_data_dir.md) |
| Chrome's cookie API requires explicit cookie and host permissions. An extension can use native messaging with a specific allowed extension origin. This offers an explicit handoff alternative if direct profile extraction is unreliable, but adds installation and distribution work. | Current primary documentation; not implemented | [Chrome cookies API](https://developer.chrome.com/docs/extensions/reference/api/cookies), [Chrome native messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging) |

No sign-in was attempted, browser cookies read, credentials entered, login grants created, or production authentication code changed during this investigation. Findings below about runtime behavior are documentation/source inspection or proposed tests, not successful authenticated experiments.

## What WKWebView can and cannot establish

The checked-in lockfile resolves Tauri 2.12.3 and Wry 0.57.0. Inspection of those installed Rust sources confirms:

- `WebviewWindow::cookies()` reads the native cookie store, including HTTP-only cookies. JavaScript's `document.cookie` is not sufficient.
- `incognito(true)` uses Apple's nonpersistent website data store. A separate persistent `data_store_identifier` is available on macOS 14 and later; Wry falls back to the default store on older systems. The prototype must explicitly reject unsupported isolation rather than silently share a store. `data_directory` alone is not a macOS WKWebView profile solution.
- `clear_all_browsing_data` and `AppHandle::remove_data_store` provide local cleanup facilities. Their presence is not proof that server-side sessions are revoked.
- Wry 0.57.0's macOS `cookies_for_url` implementation compares cookie domains directly. A future exporter should inspect the isolated store's complete cookie list and explicitly validate domain/path semantics; it should not assume a URL-filtered call includes parent-domain cookies such as `.youtube.com`.

These are **source-inspected, not runtime-tested** claims. The corresponding public APIs are documented in [Tauri's builder](https://docs.rs/tauri/2.12.3/x86_64-apple-darwin/tauri/webview/struct.WebviewWindowBuilder.html), [webview window](https://docs.rs/tauri/2.12.3/x86_64-apple-darwin/tauri/webview/struct.WebviewWindow.html), and [app handle](https://docs.rs/tauri/2.12.3/x86_64-apple-darwin/tauri/struct.AppHandle.html). Apple documents the underlying [website data store](https://developer.apple.com/documentation/webkit/wkwebsitedatastore) and [HTTP cookie store](https://developer.apple.com/documentation/webkit/wkhttpcookiestore).

Apple's [ASWebAuthenticationSession](https://developer.apple.com/documentation/authenticationservices/aswebauthenticationsession) is a browser-mediated authorization interface; its [ephemeral option](https://developer.apple.com/documentation/authenticationservices/aswebauthenticationsession/prefersephemeralwebbrowsersession) requests separation from normal browser data. It is not a general-purpose cookie-export API. Inference: using it for ordinary OAuth would not yield the YouTube cookie jar needed by the current downloader.

## Recommended prototype

Build behind an internal development flag, outside the normal album flow. Choose one installed full browser for the initial macOS experiment, preferably Chrome; do not bundle a new browser before feasibility is established.

1. **Anonymous stays first.** Never prompt to connect on a successful public inspection. Show the connection action only in Settings or when an error specifically requires an account. A network or generic 403 error should retain retry as the primary recovery.
2. **Create an owned profile.** Rust creates a private directory under app data, with an unpredictable session ID and owner-only permissions. Launch the browser binary using structured `Command` arguments with the exact custom user-data directory and YouTube's HTTPS URL. Do not copy an existing profile or enable Chrome sync. Keep normal browser address/security UI visible. No remote-debugging port, credential injection, or sign-in automation is needed for this first experiment.
3. **Human-controlled sign-in.** The user signs in normally, including MFA/passkeys. Back in the app they choose “I've signed in.” If Google rejects the browser or the user cancels, clean up only this owned profile and remain in public mode.
4. **Exact-profile handoff.** Use yt-dlp's explicit profile path only for this owned directory. Validate its canonical path and ownership; absent/invalid profile is an error, never a fallback to the most recently used personal profile. First prove a single user-authorized restricted item can be inspected. Do not infer connection from the presence of a cookie name or an avatar. Browser cookie decryption or Keychain prompts remain an open runtime question on macOS.
5. **Limit retained secrets.** Prototype exporting only the YouTube-domain session material needed by the downloader, rather than retaining Google-wide credentials or the whole browser profile indefinitely. Identify the actual required domains through a deliberate test; do not silently expand to every Google service. Store the retained jar as an app-owned, nonsynchronizing macOS Keychain item, or encrypt it with an app key held there. Delete the temporary browser profile after successful handoff and process exit. An active browser profile has its own credential files; storing a separate jar in Keychain does not automatically protect those files.
6. **Use and clean up.** Rust materializes a Netscape cookie file only when invoking yt-dlp, in an owned private directory with file mode 0600. Keep cookie values out of React state, localStorage, command arguments, diagnostics, archives, manifests, and crash reports. Remove the file after success, error, cancellation, and timeout; remove orphaned owned files on next launch. This limits exposure but does not claim forensic erasure from SSDs or protection against malware running as the same user. Account-bound media caches need equivalent cleanup.
7. **Connection expires gracefully.** A revoked or stale session produces “Reconnect YouTube,” with public access still available. Do not automatically open login windows or repeatedly retry an invalid account session. Because cookie rotation is possible, this is a reconnectable session, not a guaranteed permanent connection.
8. **Disconnect truthfully.** Stop account-backed jobs, erase the app's Keychain item and owned caches/profile/temp files, and switch to anonymous mode. “Disconnect” means the app can no longer use its local session. Do not label this “revoke account access” without a tested server-side sign-out. Offer an optional link to Google's session management page for remote sign-out.

[Apple Keychain services](https://developer.apple.com/documentation/security/using-the-keychain-to-manage-user-secrets) provides encrypted secret storage; [deletion APIs](https://developer.apple.com/documentation/security/updating-and-deleting-keychain-items) support removing specific app items. Google documents [reviewing and signing out account sessions](https://support.google.com/accounts/answer/3067630). Cookies are browser sessions, not OAuth refresh tokens; an OAuth token-revocation endpoint is not a substitute for browser-session sign-out.

If exact-profile extraction or session-only cookie capture proves unreliable, assess a narrowly permitted browser extension with an explicit “Connect to Jacket” action and native messaging. Do not silently add a debugging interface to extract credentials. Chrome has explicitly [tightened remote-debugging profile access](https://developer.chrome.com/blog/remote-debugging-port); debugging should not become the production authentication design.

## Acceptance tests before shipping

- A synthetic fixture tests domain/path handling, HTTP-only and secure flags, session-cookie expiry serialization, and no accidental cookie-value logs without real credentials.
- App restart can use an accepted session without storing secrets in frontend preferences; locked or inaccessible Keychain yields a recoverable message.
- Separate-profile sign-in and a user-authorized restricted inspection actually succeed on supported macOS/browser versions, including MFA/passkeys. Failures must be recorded without credential values.
- Personal browser windows/profiles remain untouched. Missing profile, path traversal, symlink substitution, and cancellation fail closed.
- An expired session triggers one reconnect path, while anonymous albums continue to work.
- Success, failure, cancellation, forced termination/relaunch, and Disconnect remove every owned plaintext handoff file. Disconnect prevents any subsequent authenticated invocation.
- Google-side session sign-out is tested independently if claimed; local deletion alone is never called remote revocation.

The next concrete milestone is the isolated-profile, human-operated experiment plus synthetic cookie/store cleanup tests. It can establish whether avoiding personal browser access is viable. It cannot remove cookies from YouTube's authentication model or guarantee that account access solves unrelated download failures.

## Executable developer probe

`scripts/auth-prototype.mjs` now makes the first experiment concrete. It is separate from the production application, does not extract cookies, and has no connection to the UI. No package dependencies are needed.

Run the credential-free diagnostic:

```sh
node scripts/auth-prototype.mjs diagnose
```

This checks two known Chrome executable locations, the staged yt-dlp binary and executable directories in PATH. It only checks executable accessibility, not tool versions or authenticity. It never enumerates personal browser profiles, runs yt-dlp, opens a browser, requests the network, creates directories, or reads any cookie database. The output includes the exact future yt-dlp argument for review; it does not execute that argument. Empty/relative PATH entries are ignored.

Only a human's explicit `open` invocation launches Chrome:

```sh
node scripts/auth-prototype.mjs open
```

The fixed macOS profile is:

```text
~/Library/Application Support/com.moonlark.audio-dl.auth-prototype/chrome-profile
```

The script creates owner-only app-owned directories, rejects symlinks/non-directories and unsafe ownership or permissions for the owned root/profile, and runs Chrome's installed binary with `--user-data-dir` pointing there. It offers no profile override and no fallback to the personal profile. It uses Chrome's ordinary browser interface and does not set a user agent, remote-debugging flag, or automation flag. `open` merely launches YouTube: it does not type credentials, click Sign in, complete authentication, infer a successful connection, export a session, or store a Keychain item. Missing/extra/unknown actions fail before launch.

**Not automatically cleaned up:** Chrome writes browser state into the isolated profile if the human runs `open`. That state persists between explicit launches. The script does not delete it, close unrelated browser windows, remotely revoke account sessions, or claim that the profile is encrypted by Jacket. Close the separate Chrome instance before manually removing this exact prototype directory if it is no longer needed. Production Keychain storage, handoff cleanup, and Disconnect remain release gates.

The exported `filterYouTubeCookies` helper operates only on caller-provided fixture objects. It keeps `youtube.com` and valid subdomains, rejecting lookalike, URL-shaped, malformed, Google-account and other domains. It clones retained objects and preserves cookie attributes for a future exporter. It does not serialize a Netscape file, validate a real account, capture cookies, or decide which session/partitioned cookie attributes yt-dlp requires. Those integration tests remain pending.

Validation completed in this investigation:

- `node scripts/auth-prototype.mjs diagnose` found installed Chrome and staged yt-dlp and displayed the fixed separate profile argument. No browser was opened.
- `node --check scripts/auth-prototype.mjs` passed.
- `node --test tests/auth-prototype.test.mjs` passed all five tests, covering read-only diagnostics, rejected CLI actions, private directory creation, symlink/unsafe-permission rejection, and synthetic cookie-domain boundaries/attributes. Fixture directories were temporary and removed after each test.

Not tested: Chrome launch, human sign-in, browser-profile cookie extraction, actual authenticated inspection/downloads, Keychain behavior, expiry/reconnect, or server-side sign-out. The developer must explicitly initiate and observe those experiments; the probe deliberately provides no credential extraction action.
