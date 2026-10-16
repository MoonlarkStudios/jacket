# Release checklist

Use this checklist for the exact commit and artifacts being published. Record commands, platform/browser versions, results, and artifact hashes in the release PR or CI artifacts. Historical test counts and developer-machine results do not establish acceptance of a new build.

Ongoing maintenance is tracked separately: [retained-job storage management](https://github.com/MoonlarkStudios/jacket/issues/10), [rendered workflow tests](https://github.com/MoonlarkStudios/jacket/issues/11), and [unmaintained dependency replacements](https://github.com/MoonlarkStudios/jacket/issues/12).

## Automated release process

Jacket uses one release version for the desktop app, Rust workspace, npm package and both lockfiles. Release Please maintains one release PR against `main`, proposing the version and changelog from Conventional Commit squash messages. CI rejects version drift.

1. Merge feature/fix PRs with reviewed Conventional Commit titles. The repository uses squash merges so this title is the commit automation reads.
2. The **Prepare release** workflow refreshes the release PR. Review its version, changelog, compatibility implications, and bundled-tool changes. Do not auto-merge release PRs.
3. Merging the release PR creates an immutable `v…` tag and an **unpublished GitHub Release draft** targeting that exact commit. It starts the same macOS quality/security checks, Linux runtime/media checks, installer builds and three-distribution install/upgrade/launch tests used by CI.
4. Only after every check succeeds does the workflow verify the commit, versions, asset inventory and checksums, then attach installers, sources and notices. GitHub verifies the uploaded asset digests. The workflow never publishes a release.
5. Follow the checklist below, edit the user-facing release notes and acceptance results, then select **Publish release** in GitHub. Keep beta releases marked as prereleases and out of Latest.

Only Linux x86_64 installers are currently released. macOS checks protect the source, but macOS distribution awaits Developer ID signing, notarization and clean-machine acceptance. This pipeline does not enable application updates or package repositories.

### Repository configuration

GitHub Actions must be allowed to **create and approve pull requests** in Settings → Actions → General → Workflow permissions (GitHub combines these in one setting; the bot never approves PRs). Workflow permissions are limited per job. No PAT, GitHub App, signing key, or additional secret is required: Release Please uses the repository `GITHUB_TOKEN`.

Bot-created PRs do not emit normal CI events with that token. The workflow explicitly dispatches both CI and the PR-title check on each created/updated release PR. Release builds call the CI workflow directly using the resolved commit, rather than depending on a bot-created tag to trigger another workflow.

Require the Conventional Commit title and CI checks before merging, and use the PR title as the squash commit title. The title checker reads PR metadata and executes only trusted base-branch code; it never checks out fork code in `pull_request_target`.

### Retry a failed draft build

On the **Prepare release** Actions page, choose **Run workflow**, select `main`, and enter the existing draft tag (for example `v0.1.0-beta.1`). It rebuilds and validates that exact tag commit, then resumes/replaces draft assets. Leaving the tag empty refreshes the release PR. Published releases, moved tags and commits outside `main` are rejected. Never reuse a published version or replace its installers; release a new version for a fix.

Re-running the original workflow is also safe, but an existing draft may no longer appear as a newly created Release Please output. Use the explicit tag input when recovering an incomplete draft. A failed run leaves the release unpublished; never publish a draft whose build did not finish and whose complete assets have not been verified.

## Version policy

The unreleased development baseline is `0.1.0-beta.0`; the first proposed release is `0.1.0-beta.1`. Beta counters increase for fixes/features within the same preview series. The app shows SemVer; package metadata uses `~beta.N` for Debian and RPM release `0.beta.N`, with stable RPM release `1`. CI checks native ordering through beta.10, rc and stable. Build metadata and other prerelease labels are rejected to avoid ambiguous package ordering.

`fix:` normally proposes a patch and `feat:` a minor once using the default stable strategy. Before 1.0, breaking changes propose a minor (`bump-minor-pre-major: true`); a feature does not silently establish 1.0 readiness. Changes to saved preferences, unfinished jobs, export contracts and platform requirements need explicit compatibility review.

To promote the preview to `0.1.0`, merge a reviewed PR changing the root package’s release configuration to `"versioning": "default"` and `"prerelease": false`. Include a `Release-As: 0.1.0` footer in that PR’s squash commit body to select the promotion version. Let Release Please update all version files and the manifest together in its next PR; do not manually edit the generated release branch. The `initial-version` setting only applies before the first release. Never leave a fixed `release-as` setting in the config, which would pin later releases to the same version.

For a future beta series, deliberately switch back to the prerelease strategy and prerelease flag, with the desired `Release-As: X.Y.Z-beta.1` footer. Reaching 1.0 and promoting to stable both require a maintainer decision. Keep stable and preview downloads clearly identified.

## Scope and automated validation

- [ ] State whether this is a source release, a preview candidate, or a supported binary release. List the platforms, architectures, and minimum OS versions actually tested. Keep README and platform-guide claims consistent with that scope.
- [ ] Run `mise run check` and `mise run test:media`; require successful hosted quality and security jobs for the release commit. Review dependency-audit findings rather than suppressing vulnerability advisories.
- [ ] Review the final diff, dependency locks, license/notices, and generated inventory. Keep private links, account details, cookie values, and personal paths out of shared evidence.
- [ ] Verify the packaged app uses its included audio tools with no developer tools or package-manager media tools on PATH. Check versions and hashes against the final runtime manifest/build information.
- [ ] Retain installers, checksums, notices, corresponding sources, and build information together. Follow [third-party distribution preparation](../THIRD_PARTY_NOTICES.md) for the exact shipped dependency closure.

## macOS distribution

Follow the [macOS packaging runbook](packaging-macos.md). The configured target is Apple Silicon/macOS 26 or later; local builds use ad-hoc signing.

- [ ] Build with `mise run package:mac` and validate the runtime inside the final app.
- [ ] For public binary distribution, replace ad-hoc signing with Developer ID signing for nested helpers and the outer app, using the required hardened-runtime entitlements. Notarize, staple, and verify the final distributed artifact.
- [ ] Verify the complete code signature and install/launch on a clean Mac meeting the stated minimum, without Homebrew or development tools.
- [ ] Complete the desktop and public-export checks below on that installed app.

## Linux distribution

Follow the [Linux preview guide](linux-preview.md), which defines the x86_64 Ubuntu 24.04 build baseline, installer test distributions, build commands, and source companions.

- [ ] Pass the exact commit's Linux build, staged-runtime media tests, optimized GLib regression, and DEB/RPM install/reinstall/remove jobs. Inspect retained window screenshots and logs.
- [ ] Retain all source companions and generated inventory/build/checksum files listed in the preview guide alongside the installers.
- [ ] Test an upgrade from an earlier preview, including preservation of preferences and unfinished jobs. Same-version reinstallation is not upgrade acceptance.
- [ ] Complete real GNOME/KDE Wayland/X11 and public-export checks. Container/Xvfb results alone support a preview candidate, not a fully validated desktop release.

## Desktop acceptance

For each advertised environment, test installation and launch, keyboard access, native menus, minimum-size layout, scaling, native save/reveal, and second-launch activation. Preserve edits, selection, and active work when reopening the app. Compare close behavior with the [platform matrix](architecture.md#desktop-shell).

| Environment | Additional checks |
| --- | --- |
| macOS | Dock reopen after idle/busy close; native focus restoration; clean-machine installation on the minimum OS. |
| GNOME Wayland without extensions | Compositor placement, resizing/scaling, idle close, busy minimize, overview/dock reopen. |
| KDE Plasma / tiling Wayland | Tiling/resizing, keyboard access, busy minimize, taskbar reopen. |
| Linux X11 | Rendering/scaling, window close/minimize/reopen, native dialogs on the claimed desktop. |
| Windows, if introduced | Native packaging/tool discovery, shortcuts, mixed-DPI resizing, idle close, busy minimize, taskbar reopen. |

### Public export and recovery

- [ ] Inspect a live public source; check streamed review, selection, repeated entries, metadata draft save/discard, and New download/Undo.
- [ ] Cancel the save dialog without starting a job. Export to an existing filename and verify collision-safe naming and correct reveal in the file manager.
- [ ] Check every advertised output format, metadata, artwork, relative playlist paths, manifest entries, and SHA-256 hashes with independent readers/decoders. Report artwork warnings truthfully.
- [ ] Cancel an active export without leaving a partial ZIP. Close/reopen during work, then quit/restart and resume with the same choices/destination and cached source downloads. Check unavailable destinations and storage-error recovery.
- [ ] Verify successful-job cache cleanup and the documented cleanup instructions for retained jobs. Check startup and offline restart behavior.
- [ ] Test managed downloader updating, bundled fallback, and local-install mode on each supported updater platform. Verify failed checks/transfers keep the usable downloader and activation waits for album jobs.

## Browser-session acceptance

Make account-access claims only for tested OS/browser/profile combinations. Public exports and synthetic fixtures do not prove restricted-source access.

- [ ] Confirm public access on fresh startup. Select an exact browser/profile/container and test user-authorized restricted content. Verify missing profiles never switch accounts automatically.
- [ ] Exercise operating-system permissions, cookie decryption, a busy database, expiry, reconnect, cancellation, and restart/resume with the intended profile. Confirm stopping session use leaves the browser unchanged.
- [ ] Verify cookie values and resolved credential paths stay outside frontend state, logs, manifests, archives, and recovery records.
- [ ] Keep the owned-session prototype gated off until live human sign-in, account-only verification, download, restart, expiry/reconnect, and Disconnect all pass. Confirm failed/cancelled operations and relaunch remove plaintext handoffs, personal browser profiles remain untouched, and Disconnect prevents subsequent credential use and removes owned caches. Test remote sign-out separately if claiming it.

## Publication

- [ ] Publish the tested artifacts with clear platform limits and known issues. Preview releases should name the desktop/account scenarios still awaiting acceptance.
- [ ] Confirm the documented private security-reporting channel and repository dependency/secret protections are available, and protect the default branch with the required checks.
- [ ] Do not advertise whole-app automatic updates until signed artifacts, an application updater key, and a release feed are configured and tested. yt-dlp updates are a separate feature.
