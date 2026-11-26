# 0003: Toolchain, the gate, and CI that rarely makes anyone wait

Date: 2026-11-24 · Status: accepted · Amended 2026-11-26 (what `sync` carries; the `docs` task and the hooks) · Ported from
Wordsmith Script decisions 0003 and 0040

## Decision

- **Toolchain.** `rust-toolchain.toml` names an exact release (1.100.0) with rustfmt and clippy;
  raising it is its own change. `rust-version` (1.95, WordCraft's) is the minimum, checked by the
  scheduled CI run.
- **Lints.** `unsafe_code` is forbidden and the six panicking lints are denied at every crate root
  (`cargo xtask lint-roots` checks the attributes); clippy warnings are errors. Tests may unwrap.
- **One gate.** `cargo xtask ci` runs, cheapest first: fmt, lint-roots, layers, pins, template,
  docs, privacy, licenses, clippy, tests. The same command runs locally, in the pre-push hook and in CI,
  so CI confirms what the developer already saw instead of being the first to find a mistake.
- **xtask has no dependencies**, so the gate never depends on a crate the gate must vet. The
  modules apps share (`template/xtask/src/`) are compiled into the core's xtask, so the core tests
  them, and `cargo xtask sync` copies them into each app.
- **What `sync` carries** (amended 2026-11-26). Whole files the core owns: the toolchain, lint,
  format and cargo-deny settings, the shared xtask modules, the CI and core-pin workflows, the
  hooks and the code of conduct (`MANAGED` in `template/xtask/src/sync.rs`). Blocks inside files
  an app otherwise owns: the patch block in `Cargo.toml`, and the core's rules as marked blocks
  in `AGENTS.md`, `SECURITY.md` and `CONTRIBUTING.md` (`BLOCKS`), each between a
  `<!-- BEGIN open-source-core: NAME -->` and an `<!-- END open-source-core: NAME -->` line.
  The source of a block is the same block under `template/`. `sync` replaces a block where its
  two lines are and never chooses a place for one; `sync --check`, a task of every app's gate,
  fails when a file or block differs from the pinned core release. The core carries the blocks
  that are not only about an app in its own files; its `template` task checks them and
  `cargo xtask template --write` writes them. Before this the rules were about 115 lines copied
  by hand into five files, with nothing to notice a copy that had fallen behind.
- **Only what changed.** `cargo xtask ci --changed --base <ref>` runs clippy and tests only for the
  packages a change touches and everything that depends on them (`template/xtask/src/changed.rs`).
  A change to a manifest, the lock file or the toolchain affects everything; documentation affects
  nothing. Tests run under cargo-nextest when installed.
- **Hooks.** `cargo xtask install-hooks` turns on `pre-commit` (fmt and the privacy scan of the
  staged lines, a few seconds) and `pre-push` (`ci --changed` against what the remote has).
- **What points across repositories is checked** (amended 2026-11-26). The routes between the
  core and the apps are prose, and nothing noticed when the prose stopped being true. The `docs`
  task (`template/xtask/src/docs.rs`, in the core's gate and every app's) fails on a relative
  Markdown link that leads nowhere, on a decision reference that names no record ("core decision
  NNNN" in the core's `docs/decisions/`, a bare "decision NNNN" in the repository's own), and, in
  an app, on a `core-candidate` marker that names no core issue. `cargo xtask docs <folder>` in
  the core runs it on any checkout, which is the only way a repository without Cargo is checked.
  `pre-commit` also refuses a commit while a core dev-link is in place, and `cargo xtask ci`
  says so when a clone's hooks are off, since a fresh clone or worktree starts without them.
  Every app carries one pull request template (`template/.github/pull_request_template.md`)
  that asks what another repository must take or make first.
- **CI** (GitHub-hosted runners only) is described in `.github/workflows/README.md`; its shape
  follows from the points above: quick checks in parallel with the build, changed-only clippy and
  tests on pushes, a full cross-platform run on a schedule and before a release, warm caches saved
  only from `main`, documentation-only pushes skipped, superseded runs cancelled.
- **Actions** are GitHub's own, pinned to full commit SHAs; checkouts do not keep their token.

## Revisit when

A full CI run on Linux takes longer than about 15 minutes with a warm cache, or changed-only runs
miss a failure the full run then finds (widen what counts as "affected").
