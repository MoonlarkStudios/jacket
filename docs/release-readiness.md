# Public release review

Reviewed October 15, 2026 for `MoonlarkStudios/jacket`. This is a snapshot of the reviewed working tree, including existing uncommitted desktop/authentication work. Publishing the source and shipping a polished macOS installer have different remaining requirements.

## Assessment

The core is suitable for an explicitly early macOS source release after reviewing the final diff and a first successful hosted CI run. Audio inspection/export is separated from the desktop framework, transport types are explicit, subprocesses use argument arrays, source/image URLs are structurally restricted, and archive writes are cancellable and collision-safe. Offline integration tests independently inspect actual audio tags and payloads.

A general-user binary release still needs signing/notarization, third-party distribution preparation, and clean-machine acceptance. Do not advertise Windows/Linux/Intel support or production account-session reliability based on the existing macOS public-download evidence.

## Findings addressed in this pass

| Priority | Finding | Change |
| --- | --- | --- |
| High | `rustls` 0.23.45 matched four current RustSec advisories, including TLS secret disclosure and handshake issues. | Updated the locked dependency to 0.23.46. A fresh audit found no vulnerability advisories. No claim is made that Jacket exercised every affected code path. |
| High | macOS tool staging read `minimumSystemVersion` only from the shared Tauri config. The setting had moved to `tauri.macos.conf.json`, so staging compared against the 10.13 fallback and rejected the intended runtime. | Read the shared configuration and platform override in order; added tests for override precedence, inherited values, and platform-only settings. |
| Medium | Retained job directories used default filesystem modes despite containing source URLs, metadata, and potentially private media. | Create Unix job directories with mode 0700, repair existing directory modes on save, reject symlink job directories, and test these behaviors. |
| Medium | Four synthetic Python session-capture tests were outside the normal quality gate. | Added `test:auth` to `mise run test` and therefore `mise run check`. |
| Medium | There was no repository license file and the declared license did not match the owner’s intended copyleft policy. | Added the canonical GPLv3 text, selected GPL-3.0-or-later, and aligned Rust/JavaScript metadata. |
| Medium | There was no hosted quality gate, dependency-update configuration, or documented private security-reporting path. | Added read-only, commit-pinned GitHub Actions for checks/media tests/audits/history scans, Dependabot configuration, contribution/security guidance, and issue/PR templates. Hosted execution still needs the first push. |
| Low | Tests imported `esbuild` through an indirect dependency, and Rust crates did not inherit the workspace’s declared Rust version. | Declared the existing esbuild version directly, inherited package metadata, and disabled accidental crate publication. |
| Low | Packaging generated repeated Vite page reloads from copied Python HTML and native build output. | Excluded `src-tauri` and `target` from the frontend development watcher. |
| Medium | The README was dominated by implementation history and presented no prominent user walkthrough or screenshots. | Replaced it with a branded user overview, fresh fictional-example screenshots, setup, format/privacy details, and troubleshooting. Preserved detailed behavior in `implementation.md` and added `usage.md`. |

## Remaining work

### Before a public binary release

1. **Prepare the exact runtime’s licenses, notices, and corresponding sources.** The staging script copies a large Homebrew dependency closure. Its receipts and license files are useful evidence, but are not a complete distribution package. Review the current GPL-enabled FFmpeg build and every copied library/package. See [FFmpeg’s guidance](https://ffmpeg.org/legal.html) and [third-party notices](../THIRD_PARTY_NOTICES.md).
2. **Use Developer ID signing and notarization for macOS distribution.** The configured `signingIdentity: "-"` creates local ad-hoc signatures. Sign nested helpers with the appropriate hardened-runtime entitlements, notarize/staple the final artifact, and check it on a clean Mac. [Tauri’s distribution guidance](https://v2.tauri.app/distribute/sign/macos/) describes this setup. Whole-app auto-update delivery is a separate future feature, not a requirement to publish source.
3. **Verify a clean install on the stated target.** Test Apple Silicon/macOS 26 with no developer tools or Homebrew on PATH. Complete a public export, cancellation, restart/resume, and file-manager reveal. Existing developer-machine evidence does not substitute for this test.
4. **Complete live browser-profile acceptance before making account-access guarantees.** Synthetic tests cover selection, failure handling, and cookie exclusion, but the live restricted-source, permission, expiry, reconnect, and recovery matrix remains incomplete. Keep the owned-session prototype gated off.

### Platform and maintenance limits

- **Linux dependency warning:** `glib` 0.18.5 carries `RUSTSEC-2024-0429`, an informational unsoundness advisory for `VariantStrIter`. It is pulled in through Tauri’s GTK3/WebKit stack, including `muda`/`tao`/`wry`. Replacing it with 0.20 is not a compatible lockfile-only update. This target is unvalidated; resolve the upstream chain or assess and test a targeted upstream fix before releasing Linux binaries.
- **Unmaintained dependencies:** `paste` 1.0.15 (`RUSTSEC-2024-0436`) comes through Lofty; `proc-macro-error` 1.0.4 (`RUSTSEC-2024-0370`) comes through the Linux GTK stack. Track upstream replacements. The audit workflow leaves these visible and does not suppress vulnerability advisories.
- **Cache retention:** cancelled, failed, and dismissed jobs keep media without a size limit or in-app cleanup. The [user guide](usage.md#local-data-and-cleanup) documents manual cleanup. A storage view with size, age, and explicit deletion would improve long-term use.
- **Rendered UI test coverage:** several hook tests use a small custom React-like runtime. They protect transitions but cannot reproduce React scheduling, browser accessibility, or native focus behavior. Add a small real rendered-component/integration suite for the critical link → review → save flow as the UI grows.
- **Large UI modules:** `src/App.tsx`, `src/useWorkspace.ts`, and `WorkspaceDialogs.tsx` contain substantial workflow logic. Prefer extracting a review header/track list/download footer and explicit operation-state transitions when touching these areas. A broad refactor immediately before release would add risk without proving a user benefit.

## Validation in this review

- `mise run check` passed: formatting, Biome, Clippy with warnings denied, strict TypeScript, production UI build, 53 frontend tests, 58 Rust unit tests, 5 resource-staging tests, and 4 synthetic Python session tests. The explicit live Keychain test remains ignored; the six normally ignored media tests were run separately as described below.
- All six offline media integration tests passed using real FFmpeg/FFprobe and independent Mutagen reads, including container tags/artwork, cleared fields, repeated entries, manifests, hashes, and audio payload preservation.
- Browser preview checked search, individual selection, album-draft cancellation, New download/Undo preservation, and the 600 × 600 minimum layout without horizontal overflow. Fresh screenshots show the built-in fictional album; no real account session was used.
- Gitleaks 8.24.3 found no secrets in the two existing commits or a snapshot of tracked and nonignored untracked publication files. Generated/ignored runtimes and build output were excluded from the publication snapshot.
- npm audit reported zero known vulnerabilities. Cargo audit, refreshed from RustSec, reported zero vulnerability advisories after the TLS update, plus the three upstream informational warnings listed above.
- GitHub workflow syntax was checked with actionlint. Hosted checks run on pushes and pull requests; consult the repository’s Actions tab for the latest results.
- `mise run package:mac` passed and produced a local ad-hoc signed release `Jacket.app` (approximately 253 MiB). Runtime staging validated 3,531 files and passed relocation checks. The final app passed `codesign --verify --deep --strict`, and its copied audio runtime passed manifest/hash, dependency, signature, tool-version, and synthetic codec validation.

Earlier native public-download evidence remains in [desktop experience](desktop-experience.md#validation); this pass does not claim a new live YouTube or authenticated download. The public repository starts from the reviewed source snapshot; earlier local development history is retained separately and is not published.

## GitHub setup after creating the repository

Enable private vulnerability reporting, GitHub secret scanning/push protection, and dependency alerts where available. Run the workflow and require the quality and security jobs on the protected default branch. Review Dependabot’s first runs.
