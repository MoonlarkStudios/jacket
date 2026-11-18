# Supplemental dependency notices

Some published crates omit their separate license files. Native packaging retains
upstream notices here with exact package versions, immutable upstream revisions
and SHA-256 hashes recorded in [`scripts/native-license-overrides.json`](../scripts/native-license-overrides.json).
Most notices come from the exact commit in the published crate's
`.cargo_vcs_info.json`. The provenance entries identify canonical license texts
used when an upstream release omits the legal text.

The collection script verifies these hashes, records provenance in the generated inventory, and stops packaging when a dependency has no separate notice and no reviewed supplement. If a dependency update starts shipping its own notice, the published notice takes precedence. Review and prune supplements when updating dependencies.

These files retain their original third-party licenses and copyrights.
