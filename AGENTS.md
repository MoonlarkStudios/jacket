# Repository conventions

Use mise for the development environment and repository tasks.

Follow the [Google Rust Style Guide](https://google.github.io/styleguide/rustguide.html).
Apply its API guidance in the context of this application; Chromium-specific build,
threading and directory rules do not replace Cargo conventions. See
`CONTRIBUTING.md` for module and dependency conventions.

The native app lives in `apps/jacket` and reusable crates live in `crates/`.
Keep app-specific packaging assets in `apps/jacket/packaging/`, repository
tooling in `scripts/`, and generated build and packaging outputs in `target/`.
The native workspace must not depend on Tauri or a webview.

Commit messages must be semantic and consist of a single subject line. Do not add
a commit body or message trailers. Keep tests with the feature they verify.
Each commit must build independently: update workspace membership, manifests,
Cargo.lock and affected mise tasks together. Validate the staged tree without
relying on later or untracked files.
