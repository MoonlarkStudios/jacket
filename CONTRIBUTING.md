# Contributing to Jacket

Thanks for helping make Jacket a dependable desktop utility. Bug reports, focused fixes, accessibility improvements, and documentation corrections are welcome.

## Before you start

For a substantial feature or a workflow change, open an issue first so we can agree on the user problem and scope. For a small fix, send a pull request with reproduction steps and relevant validation.

The current validated development platform is macOS on Apple Silicon. Linux has a [preview build and installer-testing path](docs/linux-preview.md). Other platforms are welcome contributions, but require their own native testing before we claim support. See the [desktop acceptance matrix](docs/releasing.md#desktop-acceptance).

## Set up

Follow the [README setup](README.md#run-the-desktop-app-from-source). `mise.toml` and `rust-toolchain.toml` are the source of truth for tool versions. Commit both dependency lockfiles when updating dependencies.

| Command | Purpose |
| --- | --- |
| `mise run desktop` | Run the native app with local audio tools. |
| `mise run dev` | Explore the browser preview with fictional example content. |
| `mise run check` | Check formatting, Biome, Clippy, TypeScript, the production UI build, and all unit tests. |
| `mise run test:media` | Run offline media integration tests with FFmpeg/ffprobe and an independent Mutagen tag reader. |
| `mise run format` | Format frontend files and Rust code. |
| `mise run lint:fix` | Apply safe Biome fixes. |
| `mise run doctor` | Check local runtimes and native build prerequisites. |
| `mise run branding:sync` | Sync approved branding into web assets and native icons. |

List individual tasks with `mise tasks`. `npm test` runs frontend tests only; use `mise run check` for the full quality gate. The checks do not modify source files. Python unit tests use synthetic session fixtures and never open a real browser database. Media integration tests generate their own audio and do not contact YouTube.

GitHub Actions runs the quality gate and media integration tests on macOS and Linux. Linux jobs also build preview installers and test installation in disposable containers. A separate job checks the dependency lockfiles and Git history for known vulnerabilities and secrets. CI retains temporary preview artifacts. The [release workflow](docs/releasing.md#automated-release-process) prepares a GitHub Release draft with permanent installers and corresponding sources after a release PR is merged. A maintainer publishes the draft after acceptance; distribution signing is separate.

## Code boundaries

- `crates/audio-engine`: inspection, metadata, tagging, codec choices, cancellable subprocesses, and archive creation. Keep it independent of Tauri.
- `src-tauri`: desktop commands, native menus, browser-profile resolution, downloader updates, job ownership, and recovery.
- `src`: review UI, editing drafts, preferences, and typed desktop transport. Keep process execution and cookie access out of the webview.
- `tests`: frontend state and behavior tests, plus synthetic Python session tests.
- `scripts`: development checks, brand synchronization, platform runtime staging, and Linux package/source collection and verification.

See [architecture](docs/architecture.md) and the [manifest contract](docs/manifest.md) before changing export behavior. Preserve original source metadata and repeated playlist entries. Keep recovery compatible where possible; handle incompatible records explicitly. Avoid changing the bundle identifier or preference namespace during branding changes.

## Validate the behavior you change

Run `mise run check`. For tagging, codec, archive, or media-pipeline changes, also run `mise run test:media`. Add a focused regression test for a bug when it protects meaningful behavior. UI changes should include a screenshot and a keyboard check at the 600 × 600 minimum window size. Check focus, reduced motion, selection, and Cancel behavior when relevant.

The hook tests use a small controlled runtime to exercise state transitions; they do not replace testing React effects and focus behavior in the real app. Keep the browser example clearly labeled, and do not enable downloads from example data.

Do not use live accounts, browser cookies, or restricted media as automated fixtures. Redact private URLs, account names, local paths, and cookie values from reports. Security reports belong in the [private reporting channel](SECURITY.md).

## Pull requests

Use Conventional Commit PR titles and squash merging. `fix:` describes a bug fix, `feat:` adds a feature, and `!` marks an incompatible change; scopes such as `fix(linux):` are optional. Use `docs:`, `test:`, `ci:`, or `chore:` for maintenance. Individual development commits can be informal. Before merging, confirm the squash commit uses the reviewed PR title; it drives proposed versions and release notes. See the [version policy](docs/releasing.md#version-policy).

Explain the problem, the resulting behavior, and how you verified it. Keep changes focused; avoid unrelated formatting or dependency churn. For provider failures, include the downloader version and a public reproduction URL only if it is safe to share.

By contributing, you agree that your contribution is licensed under Jacket’s [GPL-3.0-or-later license](LICENSE). Do not copy code or assets whose license is incompatible with that license. Third-party runtime distribution needs its own notices and source preparation; see [third-party notices](THIRD_PARTY_NOTICES.md) and the [release checklist](docs/releasing.md).
