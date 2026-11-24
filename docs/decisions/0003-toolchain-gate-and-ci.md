# 0003: Toolchain, the gate, and CI that rarely makes anyone wait

Date: 2026-11-24 · Status: accepted · Ported from Wordsmith Script decisions 0003 and 0040

## Decision

- **Toolchain.** `rust-toolchain.toml` names an exact release (1.100.0) with rustfmt and clippy;
  raising it is its own change. `rust-version` (1.95, WordCraft's) is the minimum, checked by the
  scheduled CI run.
- **Lints.** `unsafe_code` is forbidden and the six panicking lints are denied at every crate root
  (`cargo xtask lint-roots` checks the attributes); clippy warnings are errors. Tests may unwrap.
- **One gate.** `cargo xtask ci` runs, cheapest first: fmt, lint-roots, layers, pins, template,
  privacy, licenses, clippy, tests. The same command runs locally, in the pre-push hook and in CI,
  so CI confirms what the developer already saw instead of being the first to find a mistake.
- **xtask has no dependencies**, so the gate never depends on a crate the gate must vet. The
  modules apps share (`template/xtask/src/`) are compiled into the core's xtask, so the core tests
  them, and `cargo xtask sync` copies them into each app.
- **Only what changed.** `cargo xtask ci --changed --base <ref>` runs clippy and tests only for the
  packages a change touches and everything that depends on them (`template/xtask/src/changed.rs`).
  A change to a manifest, the lock file or the toolchain affects everything; documentation affects
  nothing. Tests run under cargo-nextest when installed.
- **Hooks.** `cargo xtask install-hooks` turns on `pre-commit` (fmt and the privacy scan of the
  staged lines, a few seconds) and `pre-push` (`ci --changed` against what the remote has).
- **CI** (GitHub-hosted runners only) is described in `.github/workflows/README.md`; its shape
  follows from the points above: quick checks in parallel with the build, changed-only clippy and
  tests on pushes, a full cross-platform run on a schedule and before a release, warm caches saved
  only from `main`, documentation-only pushes skipped, superseded runs cancelled.
- **Actions** are GitHub's own, pinned to full commit SHAs; checkouts do not keep their token.

## Revisit when

A full CI run on Linux takes longer than about 15 minutes with a warm cache, or changed-only runs
miss a failure the full run then finds (widen what counts as "affected").
