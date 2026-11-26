# 0002: Layers, and the one door to upstream

Date: 2026-11-24 · Status: accepted · Replaces Wordsmith Script decision 0002 for the core

## Decision

1. **`jcj-upstream` is the only crate that names WordCraft or craft-libs.** It only re-exports
   them. Every other crate reaches upstream through it or through other `jcj-` crates.
2. **Layers point down.** Each workspace crate has a layer in `xtask/src/layers.rs`:

   | Layer | Crates |
   | --- | --- |
   | 1 | `jcj-upstream` |
   | 2 | `jcj-doc`, `jcj-proof`, `jcj-entries`, `jcj-i18n` |
   | 3 | `jcj-layout`, `jcj-engine` |
   | 4 | `jcj-output`, `jcj-project`, `jcj-interchange`, `jcj-agents`, `jcj-features` |
   | 5 | `jcj-theme`, `jcj-board` |
   | 6 | `jcj-shell` |
   | 7 | `core-testbed`, `contract-tests` |

   A crate may depend on its own layer or lower. (Table brought up to date 2026-11-26: decisions
   0010 and 0016 record the crates added since.)
3. **Apps depend only on `jcj-` interface crates**, never on `jcj-upstream`, `wordcraft-*` or
   `craft-*`. The app template's `cargo xtask layers` checks it in every app. An app may also
   depend on another app's crates under a contract (decision 0015).
4. **egui is not upstream in this sense.** It is a general library; UI crates use it directly
   (at the version the patch block pins), and `jcj-shell` re-exports it so apps build against the
   same egui.

`cargo xtask layers` checks rules 1 and 2 on every CI run.

## Why

When upstream changes, only `jcj-upstream` and the adapter that wraps the changed part move; the
apps don't notice (plan principle 2).

## Revisit when

A second door is needed (for example Typst for Book, which would come in as `jcj-typeset` behind
the same rules).
