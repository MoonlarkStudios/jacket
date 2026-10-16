# GLib 0.18 safety backport

Tauri's GTK3 dependencies require GLib 0.18, so upgrading only `glib` to 0.20 is incompatible. `glib/` contains the unmodified crates.io **glib 0.18.5** source, except for the two-line upstream fix in `src/variant_iter.rs`.

- Advisory: https://rustsec.org/advisories/RUSTSEC-2024-0429.html
- Upstream fix: https://github.com/gtk-rs/gtk-rs-core/commit/b5a4071e439bef2b5eea76c3aa25e5ae84839e34
- Change: make the C output pointer mutable and pass `&mut p`, preserving the compiler's aliasing rules.
- Original license and copyright remain in `glib/LICENSE` and `glib/COPYRIGHT` (MIT).

The workspace patches crates.io to this local copy. Linux CI exercises the affected string iterator operations **with optimizations enabled**, in addition to the application tests. The package retains its original version; an auditor can still report the advisory by version. No audit suppression is added. Remove this backport when Tauri's dependency chain permits a patched upstream release.

Keep unrelated changes out of the vendored source. Compare it to the original registry package when updating it.
