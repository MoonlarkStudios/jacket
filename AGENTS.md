# Repository conventions

Use mise for the development environment and repository tasks.

Follow the [Google Rust Style Guide](https://google.github.io/styleguide/rustguide.html).
Apply its API guidance in the context of this application; Chromium-specific build,
threading and directory rules do not replace Cargo conventions. See
`CONTRIBUTING.md` for module and dependency conventions.

The native app lives in `apps/jacket`; reusable crates live in `crates/`.
`src/`, `src-tauri/` and the old packaging scripts are retained for migration
reference. The native workspace must not depend on Tauri, a webview, or the
legacy frontend build.

Commit messages must be semantic and consist of a single subject line. Do not add
a commit body or message trailers. Keep tests with the feature they verify.
Each commit must build independently: update workspace membership, manifests,
Cargo.lock and affected mise tasks together. Validate the staged tree without
relying on later or untracked files.
