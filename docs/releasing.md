# Release checklist

Use this checklist for the exact commit and artifacts being published. Record commands, platform/browser versions, results, and artifact hashes in the release PR or CI artifacts. Historical test counts and developer-machine results do not establish acceptance of a new build.

Ongoing maintenance is tracked separately: [retained-job storage management](https://github.com/MoonlarkStudios/jacket/issues/10), [rendered workflow tests](https://github.com/MoonlarkStudios/jacket/issues/11), and [unmaintained dependency replacements](https://github.com/MoonlarkStudios/jacket/issues/12).

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
