# Security policy

## Reporting a vulnerability

Use [GitHub’s private vulnerability reporting](https://github.com/MoonlarkStudios/jacket/security/advisories/new) when it is enabled. Include the affected version or commit, reproduction steps, expected impact, and a minimal synthetic example if possible.

If private reporting is unavailable, open an issue asking for a private contact **without disclosing the vulnerability or exploit details**. Do not post browser cookies, authentication tokens, private media links, browser databases, or real account data. No response-time guarantee is offered during the early source release.

## Supported versions

Security fixes currently target the latest code on the default branch. There is no long-term support policy for older snapshots. The initial distribution target is Apple Silicon macOS; other operating systems have not been validated for release.

## Security boundaries

The native application accepts supported HTTPS YouTube URLs and restricts artwork fetches to provider image hosts. It invokes audio tools using separate arguments, ignores external yt-dlp configuration, and limits subprocess output and fetched image sizes. Archive publication uses a temporary file and refuses to overwrite an existing destination.

Browser cookies remain local and are read only for explicitly selected browser sessions. Cookie values and credential paths are excluded from the webview transport, export manifests, and recovery records. Failed or cancelled jobs retain media locally for recovery; their cache can contain private media and source details.

Managed macOS downloader updates come from official yt-dlp GitHub releases and are checked against release size and SHA-256 metadata before activation. A local executable selected in Settings is trusted code and runs with the user’s permissions. Whole-application signed updates are not implemented.

The app-owned authentication experiment is disabled in production and gated behind a macOS debug build plus `JACKET_AUTH_PROTOTYPE=1`. Do not rely on it for production account access.

See [third-party notices](THIRD_PARTY_NOTICES.md) and the [release review](docs/release-readiness.md) for known upstream and distribution limitations.
