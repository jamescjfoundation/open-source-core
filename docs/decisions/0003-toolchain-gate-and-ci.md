# 0003: Toolchain, the gate, and CI that rarely makes anyone wait

Date: 2026-11-24 · Status: accepted · Amended 2026-11-26 (what `sync` carries; the `docs` task
and the hooks; the shared workflows, what a change affects, and checks on every push; the gate an
app shares, its layer table, and one run of `sync`; what CI keeps between runs, cargo-nextest in
every gate, and the apps built against the core; a third shared workflow, an app's release,
decision 0019) · Ported
from Wordsmith Script decisions 0003 and 0040

## Decision

- **Toolchain.** `rust-toolchain.toml` names an exact release (1.100.0) with rustfmt and clippy;
  raising it is its own change. `rust-version` (1.95, WordCraft's) is the minimum, checked by the
  scheduled CI run.
- **Lints.** `unsafe_code` is forbidden and the six panicking lints are denied at every crate root
  (`cargo xtask lint-roots` checks the attributes); clippy warnings are errors. Tests may unwrap.
- **One gate.** `cargo xtask ci` runs, cheapest first: fmt, lint-roots, layers, pins, template,
  workflows, docs, privacy, licenses, clippy, tests. The same command runs locally, in the pre-push hook and in CI,
  so CI confirms what the developer already saw instead of being the first to find a mistake.
- **xtask has no dependencies**, so the gate never depends on a crate the gate must vet. The
  modules apps share (`template/xtask/src/`) are compiled into the core's xtask, so the core tests
  them, and `cargo xtask sync` copies them into each app.
- **What `sync` carries** (amended 2026-11-26). Whole files the core owns: the toolchain, lint,
  format and cargo-deny settings, the shared xtask modules, the files that call the shared
  workflows (two, and from decision 0019 a third, `release.yml`), the Dependabot settings, the hooks and the code of conduct (`MANAGED` in
  `template/xtask/src/sync.rs`). Blocks inside files
  an app otherwise owns: the patch block in `Cargo.toml`, and the core's rules as marked blocks
  in `AGENTS.md`, `SECURITY.md` and `CONTRIBUTING.md` (`BLOCKS`), each between a
  `<!-- BEGIN open-source-core: NAME -->` and an `<!-- END open-source-core: NAME -->` line.
  The source of a block is the same block under `template/`. `sync` replaces a block where its
  two lines are and never chooses a place for one; `sync --check`, a task of every app's gate,
  fails when a file or block differs from the pinned core release. The core carries the blocks
  that are not only about an app in its own files; its `template` task checks them and
  `cargo xtask template --write` writes them. Before this the rules were about 115 lines copied
  by hand into five files, with nothing to notice a copy that had fallen behind.
- **An app's gate is shared code** (amended 2026-11-26). An app's `xtask/src/main.rs` used to
  be a copy of the template's: the runner, the list of tasks and two checks, repeated in every
  app, so each core release that added a task asked for hand edits in every app, and a fix to
  the runner reached none. Now `main.rs` holds only the app's data (the product's name, the
  programs it ships, the repositories it builds on besides the core) and the app's own tasks,
  and ends in `gate::main(&APP)`. The runner, the list and order of the shared tasks and each
  check are in `template/xtask/src/gate.rs`, which `sync` carries; `main.rs` declares one
  module, `shared`, which lists the others. A release that adds a check or a module needs no
  edit in an app. The shared gate also holds what each app had written for itself: one `assets`
  check, one `screenshots` task, and one `app-layers`.
- **An app's layers are a table in its `AGENTS.md`** (amended 2026-11-26), between two marker
  lines, and `cargo xtask app-layers` reads it there. The table people and agents read is the
  rule the check enforces, so the two cannot differ; the copies kept in code before had drifted
  from the tables beside them. `template/xtask/src/applayers.rs` says how a table is read.
- **More of an app's settings are blocks** (amended 2026-11-26). The marked blocks also cover
  files that are not Markdown, between `# BEGIN open-source-core: NAME` and
  `# END open-source-core: NAME`: the workspace lints and build profiles in `Cargo.toml` (the
  core carries the same block), the Cargo configuration, and the shared lines of `.gitignore`
  and `.gitattributes`. `xtask/Cargo.toml` is a managed file. `deny.toml` is a managed file
  with one part that is the app's (decision 0015).
- **One run of `sync` is enough** (amended 2026-11-26). `sync` runs as the app's xtask, built
  from the app's copy of the shared modules, so a release that changed its lists was written by
  the old lists and needed a second run. A run that changed any shared xtask source now builds
  the new xtask and runs it once more. It builds it in a folder of its own, because a program
  cannot be replaced while it runs on Windows.
- **Only what changed.** `cargo xtask ci --changed --base <ref>` runs clippy and tests only for the
  packages a change touches and everything that depends on them (`template/xtask/src/changed.rs`).
  A change to a manifest, the lock file or the toolchain affects everything.
- **Every test, run at once** (amended 2026-11-26). The `test` task of the core's gate and of
  every app's runs the tests under cargo-nextest when it is installed, then the tests in
  documentation with `cargo test --doc`; without it, `cargo test`. Both run every test, and the
  task says which ran. CI always has cargo-nextest. `cargo test` runs one test program after
  another, and in an app with long end-to-end tests that was most of every push's wait: about
  ten minutes, with nothing left to build. Nothing was taken out of the per-push gate to make it
  faster.
- **What a change touches** (amended 2026-11-26). At first a file counted only for the package
  whose folder holds it, so a change to a built-in design, a font, a fixture or a golden file at
  the root, or to a page of `docs/` that a test compares, ran no test at all and was reported
  green. Now a changed file also counts for every package whose Rust sources name it or a folder
  it is in: the argument of `include_str!` and its like from the source file's folder, any other
  string literal from the package's folder, and a literal with a `/` or a `.` in it and no `..`
  from the root as well. The sources are read as text, so this is a guess, and it is made to
  err toward testing: a changed file under the root `assets/` or `tests/` that no package names
  affects everything.
  The gate's own package is left out, since the files it names are read by its checks, which
  always run. `cargo xtask affected` prints what a change affects and why. What the guess
  cannot see is a path put together from bare words or from values that are not literals; a
  test that reads a file that way should name the file in one literal.
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
  only from `main`, superseded runs cancelled.
- **The checks run on every push** (amended 2026-11-26). Pushes that changed only Markdown used
  to start no workflow. The marked blocks, the links and the decision references live in
  Markdown, and the privacy scan reads every file, so exactly the changes those checks exist
  for were the ones CI never saw. Every push and pull request now runs the tasks that need no
  build. The build job asks `cargo xtask affected` first and ends without restoring a cache
  when the change affects no package. It runs `cargo xtask ci --only clippy,test`, so a task an
  app adds to its gate runs once, in the checks job, without the workflow knowing its name.
- **Shared workflows** (amended 2026-11-26). An app's CI used to be a whole workflow file copied
  by `sync`, so a fix to it reached an app only with a core release and a pin move, and apps on
  different pins ran different generations of CI. The jobs are now two reusable workflows in
  the core, `shared-ci.yml` and `shared-core-pin.yml`; what `sync` copies into an app is two
  short files that say when to run and call them at the tag `workflows-v1`. An app's release
  is a third, `shared-release.yml`, called the same way (decision 0019): it asks of an app the
  tasks that the core release bringing its calling file also brings.
  - The tag is moved by a person or an agent to a commit of `main` on which the core's own
    `ci.yml` has passed. That workflow calls `shared-ci.yml` from its own commit, so a change is
    tried in the core first, and one tag move then carries it to every app.
  - Nothing reaches an app by landing on `main` or by a release of the crates, and the move is
    undone by moving the tag back.
  - Apps on different pins call the same tag, so it may use only what every core release from
    0.1.19 on gives an app; `.github/workflows/README.md` lists it. A workflow that needs more
    gets a new tag (`workflows-v2`), which the callers name from the release that brings what
    it needs, and the old tag stays for apps that have not moved.
  - Considered and not chosen: calling the workflow at `main` (every push to the core would
    change every app's CI at once, a broken one included); naming a commit in each app (safe,
    but a fix is then a change in every app again); taking the workflow from the pinned
    release (what there was).
  - The xtask modules stay with the pin. They are compiled together with the app's own
    `xtask/src/main.rs`, and a module newer than the file that calls it may not build.
- **Actions** are GitHub's own, pinned to full commit SHAs; checkouts do not keep their token.
  The call to a shared workflow is the one reference by a tag that moves: it is the core's own
  file, it runs with the caller's token and no secret, and the caller grants that token
  read-only (the core-pin job may also write issues). Dependabot proposes updates to the pinned
  actions weekly: in the core, and in each app for the app's own workflow files
  (`template/.github/dependabot.yml`).
- **A failed scheduled run is reported** (amended 2026-11-26): `nightly-failed.yml` opens an
  issue, since a red mark on the Actions page reaches nobody.
- **What CI keeps between runs** (amended 2026-11-26) is the build output of the dependencies
  and nothing of the workspace's own, under a name made of what that output depends on: the
  toolchain, Cargo's settings, the locked packages that come from elsewhere and the manifests.
  It is saved from `main` only, after a whole build, and only when no archive of that name
  exists. `.github/workflows/README.md` has the rules. Before this the name ended in the
  commit, so every push to `main` saved a new archive of one to five gigabytes, most of it the
  workspace's own packages, which Cargo builds again on every fresh checkout whatever is kept.
  Three repositories sat over GitHub's 10 GB at once, and what was dropped to make room was
  as often the archive the next run needed. Considered and not chosen: paying for more
  storage; a compiler cache (sccache), which still asks the compiler about every package on
  every run and cannot keep what must be linked; keeping no fallback to an older archive, which
  would build every dependency from nothing each time one of them moves.
- **Tools are downloaded, not built** (amended 2026-11-26). cargo-nextest and cargo-deny come
  as the files their projects publish, each checked against a SHA-256 checksum written in the
  workflow. Compiling them took two to four minutes whenever their cache was gone, and an
  action of someone else's is not allowed here. The `workflows` task of the core's gate holds
  the workflow files to that, and to GitHub's own actions at a full commit.
- **The apps are built against the core** (amended 2026-11-26) every night and before a
  release, each with its own gate; decision 0011 says what a red app means for a release.
  Which repositories those are is a list in `.github/apps.json`, so the core's code still
  names no product (decision 0013).

## Revisit when

A full CI run on Linux takes longer than about 15 minutes with a warm cache, or changed-only runs
miss a failure the full run then finds (widen what counts as "affected").
