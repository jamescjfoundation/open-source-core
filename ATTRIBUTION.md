# Attribution

Every asset in this repository that we did not write, and every block of code
copied from another project, is listed here with its source and license. Add the
entry in the same commit as the asset or code.

## Upstream code

WordCraft and craft-libs are dependencies, not copies (docs/UPSTREAM.md). Code
moved here from Wordsmith Script (the foundation's own, MIT OR Apache-2.0) is
listed in docs/review/script-29ed4bf.md. Code in it that Script had itself copied
from WordCraft keeps WordCraft's credit:

| Path | What it is | Source | License |
| --- | --- | --- | --- |
| `crates/jcj-shell/src/ribbon_fit.rs` (if present) | Ribbon groups that shrink to fit | WordCraft `crates/ui-egui`, via Wordsmith Script (decision 0037 there) | MIT OR Apache-2.0 |
| `crates/jcj-shell/src/icons.rs` (if present) | The icon helper and general icons drawn in code | WordCraft `crates/ui-egui`, via Wordsmith Script | MIT OR Apache-2.0 |

## Assets

None yet. Fonts the shell shows come from WordCraft's bundled set (through
`jcj-layout`) or from the app.
