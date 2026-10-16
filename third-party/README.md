# Supplemental dependency notices

Some published crates and frontend packages omit their separate license files. Linux packaging retains upstream notices here, with exact package versions, URLs and SHA-256 hashes in [`scripts/linux-license-overrides.json`](../scripts/linux-license-overrides.json). Most come from the exact commit recorded in the published crate's `.cargo_vcs_info.json`.

For `selectors`, retain Mozilla's canonical MPL 2.0 text alongside the original licensed source headers. For `react-remove-scroll-bar`, the npm package and README declare MIT but omit the license; its registry `gitHead` is unavailable upstream. The recorded immutable official repository commit supplies the MIT notice. These exceptions are documented in the provenance entries, rather than represented as files from those release archives.

The collection script verifies these hashes, records provenance in the generated inventory, and stops packaging when a dependency has no separate notice and no reviewed supplement. If a dependency update starts shipping its own notice, the published notice takes precedence. Review and prune supplements when updating dependencies.

These files retain their original third-party licenses and copyrights.
