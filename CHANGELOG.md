# Changelog

Each release says what changed for apps. While at 0.x, a patch release never breaks an app; a
minor release may, and then **What apps must do** says exactly what (decision 0011).

## Unreleased

### Added

- **Each app's whole gate on macOS and Windows, every night and on demand** (decision 0003,
  amended; issue #49). An app's CI runs on Linux, and so does the core's build of each app;
  the first run of an app's tests on macOS and Windows used to be its release run, and the
  first releases needed several runs to find tests that had never run there. A fourth shared
  workflow, `shared-platforms.yml`, runs `cargo xtask ci`, whole, on `macos-latest` and
  `windows-latest`, with the archives, tools and cargo-nextest of a release's gate. A new
  managed file, `.github/workflows/platforms.yml`, calls it at `workflows-v1` every night and
  when started by hand (`gh workflow run platforms.yml --ref <branch>`, with
  `-f system=macos` or `-f system=windows` for one system). It runs on no push or pull request
  and is not a required check. A scheduled run that fails opens the issue "The macOS and
  Windows gate failed" in the app, or adds a comment to the open one. Nothing is kept between
  its runs.
  - An app whose tests rarely run on a system may set `fail-fast = false` in its own
    `.config/nextest.toml`, so that one run names every failing test, not the first.

### Changed

- **Every change lands through a pull request whose checks pass, and its author merges it**
  (decision 0013, amended; 0011 and 0003 too). Apps built on the core have public releases now,
  so a broken `main` can reach users with an app's next release. The `main` rule of the core and
  of every app requires a pull request and the three checks of the shared CI
  (`gate / checks (every task but clippy and test)`, `gate / cargo-deny (licenses, advisories,
  sources)`, `gate / clippy and tests (affected packages)`); GitHub refuses a direct push. No
  approval is required: one account does the work, so an approval would not be a second pair
  of eyes. The carried text says so: the `app-and-core` block of `AGENTS.md` ("How a change
  lands", and step 2 of "Releasing an app"), the `contributing` block of `CONTRIBUTING.md`, and
  the managed pull request template. `cargo xtask release` now ends by saying to land the
  release commit through a pull request. The pre-push hook stays: it runs the gate before a
  branch is pushed.

### Fixed

- **The package data of 0.1.22's "What apps must do" ends with `..Package::DEFAULT`.** An app
  that writes every field of `Package` must leave that line out: clippy refuses a `..` that
  fills nothing (`needless_update`), and the gate's clippy fails. `docs/app-releases.md`, the
  documentation of `Package::DEFAULT` and the template's `xtask/src/main.rs` say so now.

### What apps must do

1. Move every `jcj-` dependency to this release's tag, then run
   `cargo metadata --format-version 1 >/dev/null` to bring `Cargo.lock` in line.
2. Run `cargo xtask sync`, once, then `cargo xtask sync --check`. It rewrites the
   `app-and-core` block of `AGENTS.md`, the `contributing` block of `CONTRIBUTING.md`,
   `.github/pull_request_template.md`, `xtask/src/shared.rs` and `xtask/src/sync.rs`, and adds
   one new managed file, `.github/workflows/platforms.yml`: the app's whole gate on macOS and
   Windows, every night at 02:23 UTC and when started by hand, through the core's
   `shared-platforms.yml` at `workflows-v1`. It is not a required check and needs no setting:
   the repository keeps issues on, as it already does for `core-pin.yml`. Nothing in the app's
   own code changes, and the gate passes as before.
3. Commit, land it through a pull request, and start the new workflow once on `main` to see
   where the app stands: `gh workflow run platforms.yml --ref main`. A test that fails there
   is the app's to fix (or to mark for its system) before its next release, whose gate runs
   the same tests on the same systems.

## 0.1.22 (2026-11-27)

### Added

- **One release contract and one release pipeline for every app** (decision 0019;
  `docs/app-releases.md` is the contract). Three apps had each written a release workflow, a
  way to package and names for the files; none had run, two named files the update check does
  not find, one wrote checksum lines it does not read and one published no checksums.
  - **The contract.** A release is the tag `vX.Y.Z` with four files: one package a system,
    named `<id>-<version>-linux-x86_64.tar.gz`, `<id>-<version>-macos-aarch64.dmg` and
    `<id>-<version>-windows-x64-portable.zip`, and `SHA256SUMS.txt` in the form `sha256sum`
    writes. Every package holds the app's license files and `THIRD-PARTY-LICENSES.txt`. The
    desktop program is named by the release id and every other program `<id>-<name>`. The
    release's text is the changelog section. The document says what each package holds, file
    by file, and what a launcher or a website may rely on.
  - **The fonts' license texts go with the programs.** The core's interface fonts are built
    in from WordCraft, and `THIRD-PARTY-LICENSES.txt` said their license texts sat beside it;
    in two apps' packages none did. `cargo xtask package` now takes them from the
    dependency's source into `licenses/assets/fonts/` of every package, beside the license
    texts of the app's own fonts (`license_files`).
  - **`cargo xtask package`** in every app builds the package of the system it runs on into
    `dist/`. `cargo xtask package --sums <folder>` checks a folder of release files and writes
    the checksum file; `--describe` prints the release's id, version and file names.
  - **`cargo xtask release <version>`** in every app makes the changes of a release commit:
    the version in `Cargo.toml` and `Cargo.lock`, and `## Unreleased` in `CHANGELOG.md` as
    `## <version> (<date>)`. `--check` says whether the checkout is a release commit;
    `--notes` prints the release's text.
  - **What a release carries is data**: `package: Some(Package { .. })` in `App`, with the
    release id, the app's id on a desktop, icons, file types and the license texts of what the
    programs embed. The menu entry, the description of the file types, the macOS property
    list and icon, the scripts that register file types on Windows and the Linux install
    script are written from it.
  - **The gate has a `release-data` task**: the files the package data names are there, the
    icons are the size they are said to be, the programs carry their names, and the
    changelog's newest section is `## Unreleased` or a dated release. An app with no package
    data passes.
  - **`shared-release.yml`**, called by a third managed file, `.github/workflows/release.yml`,
    at `workflows-v1`. Started by hand for a release commit on `main`, it runs the whole gate
    on Linux, macOS and Windows with cargo-deny, builds and checks every file, attests them,
    and only then makes the tag and publishes: the core's own order. `-f dry-run=true` builds
    and checks every file on any commit and stops, and the same dry run starts by itself once
    a week; a scheduled one that fails opens an issue in the app.
  - **Signing is built in and waits for certificates.** The macOS and Windows steps run when
    their secrets exist (named in the contract) and say so when they do not.
- In the core: the names and the checksum file of the contract are tested against
  craft-update's own reader (`crates/jcj-shell/src/updates/desktop.rs`), and the packaging
  code against the same example (`docs/app-release-example/`). `nightly.yml` makes the
  template app's release package on Linux, macOS and Windows
  (`cargo xtask template-check --package`), so the packaging of every system is tried each
  night and before a core release.

### What apps must do

1. Move every `jcj-` dependency to this release's tag, then run
   `cargo metadata --format-version 1 >/dev/null` to bring `Cargo.lock` in line.
2. Run `cargo xtask sync`, once. It brings four changed modules (`xtask/src/gate.rs`,
   `shared.rs`, `sync.rs`, `licenses.rs`), a changed comment in `.github/workflows/ci.yml`,
   and one new managed file, `.github/workflows/release.yml`, which replaces an app's own file
   of that name (step 3). Then `cargo xtask sync --check`. The gate passes as before with no
   other change: its new `release-data` task asks nothing of an app that has no package data
   yet.
3. **Release data and names** (decision 0019). `cargo xtask sync` has replaced the app's
   `.github/workflows/release.yml` with the managed one; that is meant. Then, in the app:
   - In `xtask/src/main.rs`, give `APP` its `package`:

     ```rust
     use shared::package::{FileType, Package};

     const APP: App = App {
         product: "<the app's name>",
         shipped: &["<the desktop program's package>", "<the command-line program's package>"],
         package: Some(Package {
             id: "<the release id>",
             app_id: "<the id the window is given, such as org.example.app>",
             summary: "<what the app is for, in a few words>",
             categories: "Office;",
             icons: &[(256, "assets/brand/icon-256.png")],
             windows_icon: Some("assets/brand/icon.ico"), // when a build script embeds one
             file_types: &[FileType { name: "<kind>", extensions: &["<ext>"], mime: "application/x-<type>", ..FileType::DEFAULT }],
             license_files: &["assets/fonts"],
             ..Package::DEFAULT
         }),
         ..App::DEFAULT
     };
     ```

     `docs/app-releases.md`, "What an app gives", says what each field is.
   - `shipped` holds **package** names, as `cargo build -p` takes them, the desktop program's
     first. An entry that named a program whose package is called otherwise never worked:
     `cargo xtask third-party-licenses` fails on it.
   - The desktop program is named by the release id, and every other program `<id>-<name>`
     (`<id>-cli`). Rename the `[[bin]]` of a program that is named otherwise, and every place
     that names it: the guide, the README, tests that start it, agent configuration examples.
   - The release id is the `id` of the app's `ReleaseSource`, and `app_id` is the id the
     window is given (`with_app_id`). Keep a test in the app that the two places agree, since
     the xtask cannot read the app's code.
   - `CHANGELOG.md`: one `## ` section a release, newest first; `## Unreleased` until the
     release, then `## X.Y.Z (YYYY-MM-DD)`. A heading in another form
     (`## 1.0.0 (unreleased)`) fails `release-data`.
   - Delete what the shared code replaces: the app's own packaging (a `packaging/` folder, a
     packaging script, a `bundle` task and its module), and the desktop entry, MIME, property
     list and registry files kept for it. `cargo xtask package` writes them from the data.
     A check that a release step ran in the app's old workflow and that is not in its gate
     (a test run in release mode) moves into the app's tests or is dropped.
   - Run `cargo xtask release-data`, then `cargo xtask package` on Linux, and look inside the
     file in `dist/`.
4. **The update check** (`apps/<app>/src/main.rs`), where the app has one:
   - an app that starts through `Desktop::main` or `Desktop::run` (0.1.21) has the updater's
     call already, and `Desktop::finish_update()` is the call for one that reads its own
     command line first. An app that keeps its own start-up makes the call of 0.1.21 first in
     `main`, before it reads its own arguments:

     ```rust
     let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
     if let Some(done) = RELEASES.finish_update(&args) {
         return if done.is_ok() { ExitCode::SUCCESS } else { ExitCode::FAILURE };
     }
     ```

   - close the window when `shell.close_requested()` is true, and call `shell.finish()` last
     in `on_exit`. An app that does neither installs an update and never starts it.
   - `installs_in_place` stays `!cfg!(target_os = "macos")`.
5. **Say it where users and contributors read**: the app's guide and README name the three
   files and how each is opened (an unsigned build until certificates exist), and the app's
   `AGENTS.md` or `CONTRIBUTING.md` points at the "Releasing an app" steps of the shared block
   in place of its own. A decision record of the app that described its old release workflow
   is marked superseded by core decision 0019.
6. **The first release** of an app, once the steps above are on its `main`:
   `gh workflow run release.yml --ref main -f dry-run=true` until it is green, then
   `cargo xtask release <version>`, `cargo xtask ci`, commit, push, and
   `gh workflow run release.yml --ref main`. Afterwards check the release: four files, and the
   app's update check, started from the release before, finds it.

**The release workflow** is new at `workflows-v1`: the tag is moved to a commit that has
`shared-release.yml` before this release is cut, or an app's `release.yml` names a workflow
that is not there yet. The app's `release.yml` starts a dry run every Tuesday; until step 3 is
done in an app that run fails (the app has no package data) and opens an issue saying so.

## 0.1.21 (2026-11-26)

### Added

- **cargo-nextest runs the tests of every app's gate when it is installed** (decision 0003,
  amended). The `test` task ran `cargo test`, which runs one test program after another; in an
  app with long end-to-end tests that was about ten minutes of every push with nothing left to
  build. Now, in `xtask/src/gate.rs`:
  - with cargo-nextest on the machine, the task runs `cargo nextest run --locked`, then the
    tests in documentation with `cargo test --locked --doc` for the packages that have a
    library (cargo-nextest does not run those);
  - without it, `cargo test --locked`, as before;
  - both run every test; the task's first line says which ran, so a result on a laptop and one
    in CI can be told apart when they differ.
  CI installs cargo-nextest for every app, so an app's CI uses it from the push that moves its
  pin here. No test leaves the per-push gate. The core's own gate uses the same code.
- **`cargo xtask prune-target <older lock file> [--target <folder>]`** in every app: removes
  from `target/` the build output of every package that moved between that lock file and the
  one in use, and of everything built on one. The shared CI workflow runs it after it restores
  build output kept from another lock file. It reads `.cargo/dev-link/Cargo.lock` when the app
  is linked. Nobody needs to run it by hand.
- In the core: **each app is built against a core commit, with its own gate, every night and
  before a release** (decision 0011, amended; the job `apps` of `nightly.yml`). For each
  repository in `.github/apps.json` it checks out the app at the newest commit of its `main`
  that passed the app's own CI, links it to the core commit with the app's own
  `cargo xtask dev-link`, takes the commit's shared files with `cargo xtask sync`, and runs
  `cargo xtask ci --linked`. An app that does not build or does not pass, a golden file that
  differs included, fails the run. `.github/workflows/README.md` says what a green job shows
  and what it cannot: an app that builds on another app's crates is built with them at the tag
  it names, not at that app's `main`; the apps are built on Linux only; an app on a core older
  than 0.1.20 cannot be linked this way and fails until it moves.
- In the core: **a release is gated before its tag exists, and the workflow makes the tag**
  (decision 0011, amended).
  - `cargo xtask release <version>` makes the changes of a release commit: the version in
    `Cargo.toml` and `Cargo.lock`, the tags in `template/Cargo.toml`, and `## Unreleased` in
    this file as `## <version> (<date>)`. It refuses a section with no **What apps must do**.
  - `release.yml` is started for that commit (`gh workflow run release.yml --ref main`). It
    runs the gate on Linux, macOS and Windows with the minimum Rust version, cargo-deny, the
    template and the apps; only when all of it has passed does it make the annotated tag and
    publish the GitHub release. A run of the gate that has already passed on the commit is
    used, not repeated. Pushing a tag by hand starts nothing; nobody does it.
  - A red app stops the release unless the release's section of this file carries, for that
    app, the line `**App gate override, <owner>/<repository>:** <the reason>`.
- In the core: the gate has a `workflows` task. Every workflow uses only GitHub's own actions,
  each at a full commit; two workflows that download the same tool name the same version and
  checksums; a script two workflows must run alike is the same text in both; and
  `.github/apps.json` can be read.

- **An app's whole start-up, from its data: `jcj_shell::desktop`** (decision 0014, amended; core
  issue #46). Three apps each wrote the code that opens the window, and each copy left out a
  step another had: the close that starts an installed update and its helper at the end; a
  file whose name is not Unicode; the shell's file dialogs; coming back to write a preference
  that waited. The steps are now the core's, and an app's `main` can be one call.
  - `Desktop` is the app's data: its name, machine name, window app id and version, its brand,
    where its releases are (`Releases`), a variable that names another settings folder, and
    the window's first size. `Desktop::new(...)` fills the rest, so a field added later needs
    no edit in an app.
  - `DesktopApp` is what only the app can do, each with a default but its preferences:
    `apply_prefs`, `prefs`, `open` (the files it was started with), `report` (where a problem
    with the preferences is shown), `may_close` and `close_confirmed` (the question before a
    window with unsaved work closes), `tick`, `window_title` and `exit`.
  - `desktop::Window` is the window's state and the steps of a frame (`logic`, `ui`) and of
    the end (`exit`). It needs no window toolkit, so a test drives it; dropping it ends it.
  - With the crate's new `window` feature, `Desktop::main` is the whole `main` of an app and
    `Desktop::run` is the same for an app that reads its own command line first. The feature
    brings in eframe and is off by default: an app that keeps its own start-up builds exactly
    as before, with nothing new compiled.
  - `prefs::Contents` lets the start-up work with any app's preferences file, the usual
    `Prefs<A>` or an app's own struct; `prefs::NoPrefs` is for an app with none of its own.
  - `desktop::arguments` and `files_named` give the files named on the command line as the
    system wrote them.
- **The updater's helper has its entry point** (core issue #36). On Windows the updater
  finishes an update by starting a copy of the app with `--finish-update`.
  `ReleaseSource::finish_update(args)` does that work, and `Desktop::finish_update()` is the
  call for the top of `main`: `Some(exit code)` when the process was the helper, so the app
  returns without a window. `Desktop::main` and `Desktop::run` make the call themselves.
  `updates::desktop::is_finish_update` and `FINISH_FLAG` say how the start is told apart.
- **Brand loading: `jcj_theme::brand`** (decision 0014, amended; core issue #46). The brand
  stays the app's; what three apps each wrote around it is the core's.
  - `Brand::new(tokens_json, icon_png)`: `tokens()` (the neutral set when the file does not
    read, so a window always opens), `read_tokens()` (the error, for the app's test),
    `section::<T>(name)` (the app's own part of the tokens file), `icon()` (the window's icon)
    and `apply(ctx, mode)` (fonts and theme in a context the shell does not manage).
  - `Mask::new(key, png)` paints a mark from a white mask in any color, with its texture made
    once for each context.
  - `brand::contrast`, `luminance`, `hue` and `hue_distance` are the WCAG figures a brand's
    tests hold its colors to; `brand::ico_sizes` reads the sizes in a Windows icon file;
    `brand::decode_png` decodes a PNG within a size bound.
- **A project as a folder of plain files: `jcj_project::folder`** (decision 0018; decision
  0009, amended; core issue #45). The standard for an app's project is now a folder of plain
  files, which two apps had each built for themselves; the single-file container stays for a
  document that is one file. `crates/jcj-project/FOLDER.md` is the specification.
  - `Format` describes one app's folder as data: the manifest and its schema, the files
    written last, which paths belong, the packed form's `mimetype` and extension, the endings
    stored uncompressed, whether conflict copies are kept, the `Limits`.
  - `Folder::open` refuses a folder without the manifest and a project saved by a newer
    version. The app reads through `Source`, which a `Folder` and files in memory (`Memory`)
    both are, so one reader serves the folder, a packed project and a recovery copy.
  - `Folder::save` writes only what changed, each file atomically, the manifest last, and
    removes what the app no longer renders; text the app read and did not change is never
    rewritten. `Folder::outside_changes` finds files changed elsewhere, and a save keeps a copy
    of one before writing over it. `Sink` and `Disk` let a test fail a save part way.
  - `pack`, `pack_folder`, `folder_files`, `unpack` and `unpack_into`: the folder as one file,
    the same bytes for the same files, read back within limits.
  - `write_recovery`, `recovered`, `discard_recovery`, `recovery_path` and
    `Folder::save_safely`: unsaved work kept inside the folder and found after a crash.
  - `backup`, `backup_packed` and `backups`: dated packed copies, the oldest removed.
  - `schema_of`, `read_manifest`, `toml_nesting` and `plain_path`: the checks on a manifest
    and on a path, for an app's own code.
  - `files::backup_bytes` is `files::backup` for bytes that are not a file yet.
  `jcj-project` now depends on `toml`.
- **The app template starts whole.** `template/apps/app/src/main.rs` is its data and a
  `DesktopApp`, with `DESKTOP.main(...)` for its `main`. `template/apps/app/build.rs` embeds
  the Windows icon and reads the program's name, company and copyright from
  `[package.metadata.winresource]` in the package's `Cargo.toml`, so the script is the same in
  every app.
- `core-testbed` opens its window through `jcj_shell::desktop`, with a brand of its own loaded
  through `jcj_theme::brand` and a preferences file; its self-test has a `folder` step that
  uses every part of the folder layer; and it makes the updater's call first in `main`.

### Changed

- **What the shared CI workflow keeps between runs** (decision 0003, amended;
  `.github/workflows/README.md`, "What CI keeps between runs"). The name of the build archive
  ended in the commit, so every push to `main` saved a new one of one to five gigabytes, most
  of it the workspace's own packages, which Cargo builds again on every fresh checkout whatever
  is kept. Three repositories were over GitHub's 10 GB at once. Now:
  - the archive holds the build output of the dependencies only, and is named after what that
    output depends on: the system, the toolchain and Cargo's settings, the locked packages that
    come from elsewhere, and the manifests without the workspace's own version;
  - it is saved from `main` only, after a build of everything, and only when no archive of
    that name exists. A push that moves no dependency saves nothing;
  - a run on `main` that finds no archive for its dependencies builds and tests everything
    once, so that what it saves is whole;
  - after a dependency moves, a run starts from the newest archive of the same toolchain and
    first drops what the move made stale (`cargo xtask prune-target`, where the app has it).
- **CI compiles no tool.** cargo-nextest and cargo-deny are downloaded as the files their
  projects publish, and each file is checked against a SHA-256 checksum written in the
  workflow. Compiling them took two to four minutes whenever their cache was gone.
- In the core: `nightly.yml` and the release use the same archives as `ci.yml`, saved from
  `main` only (a run for a tag used to save archives no other run could read, which pushed out
  the ones in use), run the tests under cargo-nextest on all three systems, and restore the
  cargo home in the cargo-deny job.
- In the core: `release.yml` no longer starts when a `v*` tag is pushed.

### What apps must do

**The gate and CI.**

1. Move every `jcj-` dependency to this release's tag, then run
   `cargo metadata --format-version 1 >/dev/null` to bring `Cargo.lock` in line.
2. Run `cargo xtask sync`, once. It brings three changed modules (`xtask/src/gate.rs`,
   `changed.rs`, `corepin.rs`). Then `cargo xtask sync --check`. No file of the app's own
   changes: not `xtask/src/main.rs`, not a workflow file.
3. Install cargo-nextest where the gate is run by hand, if it is wanted there (the gate works
   without it): <https://nexte.st/docs/installation/pre-built-binaries/>. Run `cargo xtask ci`
   once with it installed. cargo-nextest runs each test in a process of its own and several
   test programs at a time, which two kinds of test notice:
   - **A test that asserts a time** (a frame under so many milliseconds, a page within a
     second). Under `cargo test` its program had the machine nearly to itself; now other test
     programs run beside it, and on a busy machine it fails. Give it the machine: in
     `.config/nextest.toml`, which is the app's own file,

     ```toml
     [[profile.default.overrides]]
     filter = 'binary(<the test program>)'
     threads-required = "num-test-threads"
     ```

     cargo-nextest then runs nothing beside it. `cargo test` does not read the file, so it
     can be added before the pin moves. Wordsmith Book: the programs `live` and `preview` of
     `crates/book-ui/tests/` failed this way on a heavily loaded machine and passed on a less
     loaded one.
   - **A test that passed only because of its neighbours**: because another test of the same
     program ran before it, or because no other program ran beside it (two that write the
     same file, say). Make it stand alone; where two tests must not run together, put them in
     a test group in the same file.

**The shared workflow** asks nothing of an app. It reaches an app when the core moves the tag
`workflows-v1` to a commit that has it, not with this release, and no file in an app changes
for it. The tag may move while apps are on any core release from 0.1.19 on, which is every app
that calls it: the workflow runs the same commands of an app's xtask as before (`ci --skip`,
`ci --only`, `affected`), and tries `prune-target` and goes on without it where an older core
has none. An app on 0.1.19 or 0.1.20 gets the new archives and the downloaded tools at once and
keeps running its tests with `cargo test` until it moves here. After the tag has moved, the
first run on an app's `main` builds everything once from nothing, because the archives have new
names; `gh workflow run ci.yml --repo <owner>/<app> -f full=true` starts that run by hand, so
that it is nobody's push that waits for it.

**The start-up, brand loading and folder projects.** Nothing is required, and nothing an app
does today changes: every item above is new, and the start-up is behind a feature that is
off. What an app may switch to, when it chooses:

- **Its start-up.** Give `jcj-shell` the feature (`features = ["window"]`), put the app's data
  in a `Desktop`, implement `DesktopApp` for the type that implements `App`, and make `main`
  `DESKTOP.main(|start| ...)`, or `DESKTOP.finish_update()` first and `DESKTOP.run(files, ...)`
  after the app's own command line. The app's own `eframe::App`, its preferences plumbing,
  its icon decoding and its `eframe` dependency go. What changes for a user is what the app's
  copy left out: see core issue #46 for each app.
- **The updater's call**, for an app that keeps its own start-up: first thing in `main`,
  `if let Some(done) = RELEASES.finish_update(&args) { ... return }`, with the arguments
  without the program's name. Without it an update cannot finish in place on Windows.
- **Its brand.** `Brand::new(include_str!(...), include_bytes!(...))` in place of the app's
  own token reading and icon decoding; `Mask` in place of its own mask texture;
  `brand::contrast` in place of the helpers in its tests.
- **Its build script.** The template's `build.rs`, with the four names moved to
  `[package.metadata.winresource]`.
- **Its project folder.** Decision 0018 lists, for each of the two apps that have one, what
  moves to `jcj_project::folder` and what stays; neither app's files on disk nor its packed
  form change.

## 0.1.20 (2026-11-26)

### Added

- **An app's gate is the core's code, and `xtask/src/main.rs` holds only what is the app's**
  (decision 0003, amended). Each app's `main.rs` was a copy of the template's: the `ci` runner,
  the list of tasks, the layer and license checks. Every release that added a task asked for
  hand edits in each app, and a fix to the runner reached none.
  - The runner, the list and order of the shared tasks and every shared check are in
    `xtask/src/gate.rs` (`template/xtask/src/gate.rs` here), a managed file.
  - `main.rs` holds the app's data, `const APP: App` (the product's name, the programs it
    ships, the repositories it builds on besides the core, and a few options), and the app's
    own tasks, `Task::check(name, help, function)` for the gate and `Task::other(...)` for a
    task run by name. It ends in `gate::main(&APP)`.
  - `main.rs` declares one module of the core's, `mod shared;`. `xtask/src/shared.rs` lists the
    others, which sit beside it as before.
  - So a release that adds a check, a shared task, a module or a field of `App` needs no edit
    in an app: `cargo xtask sync` brings it, and `..App::DEFAULT` fills a new field.
  - The gate runs the shared checks, then the app's own, then clippy and the tests.
    `cargo xtask help` prints all of it.
- **`cargo xtask app-layers` in every app, reading the table in `AGENTS.md`.** Two apps had each
  written the check, with the rule in code and a table beside it for people; the two had
  drifted. The table agents read is now the rule: it sits between the lines
  `<!-- BEGIN layer table (cargo xtask app-layers reads it) -->` and `<!-- END layer table -->`,
  and nothing else holds the layers. `xtask/src/applayers.rs` says how it is read; in short:
  - rows are layers, lowest first; with a `Layer` column, rows with one number are one layer;
  - the packages column names workspace packages in backticks, by package name, by folder
    (`crates/app-model`), by the folder's last part, or `apps/*` for every package under a
    folder; every package but `xtask` is in exactly one row;
  - the last column with "use" or "depend" in its heading names, in backticks, the crates from
    outside the workspace a layer may use; a crate is allowed from the layer that names it
    upwards, and `(only here)` after a name keeps it to that row's packages;
  - it fails on a package in no row or in two, a name that is no package, a dependency on a
    package of the same or a higher layer, and a use of a core crate (`jcj-`), a provider's
    crate or any crate the table names that the table does not allow there. Other libraries
    are not its business.
- **One `assets` check and one `screenshots` task**, in place of three of each.
  `cargo xtask assets`: every file under `assets/` that Git tracks or would add is named in
  `ATTRIBUTION.md` in backticks, by its path or by a folder above it and below `assets/`; no
  asset is a symbolic link; `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`, `ATTRIBUTION.md` and
  `SECURITY.md` exist, and the files the app adds in `legal_files`. `cargo xtask screenshots`
  runs the `screenshots` example of the package named in `screenshots`, into `docs/images/`.
- **A dev-link changes no tracked file** (decision 0013, amended).
  `cargo xtask dev-link ../open-source-core` used to write the link, with the full path of a
  folder on one machine, into `.cargo/config.toml`, which every app tracks, and a build while
  linked rewrote `Cargo.lock`.
  - The link is now the folder `.cargo/dev-link/`, which Git ignores. It holds a Cargo
    configuration file with the `[patch]` tables, which the tracked `.cargo/config.toml`
    includes when it exists (`include = [{ path = "dev-link/config.toml", optional = true }]`),
    and the lock file of the linked build (`resolver.lockfile-path`).
  - Plain `cargo build`, `cargo test` and an editor's rust-analyzer build against the link with
    no flag. The app's own `Cargo.lock` is not written while linked, so `dev-unlink` has
    nothing to restore: link, build, unlink leaves `git status` clean.
  - A relative folder is read from the app's root, wherever the command is run.
  - `cargo xtask dev-link` alone says what is linked. It refuses to link where Git would not
    ignore the folder or Cargo would not read it, and it checks that Cargo took the link.
  - It needs the Cargo that `rust-toolchain.toml` names (1.100). An older Cargo ignores
    `resolver.lockfile-path` (1.95 does) and would write the app's own `Cargo.lock`: with one,
    `dev-link` puts the lock file back, keeps no link and says why.
  - The gate's `unlinked` task still fails while linked: the gate says what CI will say, and CI
    builds the pinned release. `cargo xtask ci --linked` runs the whole gate against the link
    and says so in its last line.
  - `unlinked` also fails, linked or not, on a `[patch]` table or a `paths` entry in
    `.cargo/config.toml` and on a `[patch]` entry with a local path in `Cargo.toml`. The
    pre-commit hook runs only that part (`cargo xtask unlinked --tracked`), so committing
    while linked is fine.
- **A second address for the link: a provider.** An app that builds on another app's crates
  (decision 0015) names that repository in its `main.rs`,
  `providers: &[Provider { name: "other", git: "https://github.com/<owner>/<repository>" }]`,
  and links a checkout of it by that name: `cargo xtask dev-link other=../other-app`, alone or
  with the core in one command. The core names no app for this.
- **A temporary pin, so a pull request can be checked against a core change before its
  release** (decision 0013, amended). On a branch of the app, every `jcj-` line names the same
  whole commit of the core, `rev = "<40 characters>"` in place of `tag`. `cargo xtask core-pin`
  passes there and in the branch's pull request, with a warning that GitHub shows on the
  checks page; `sync --check` compares with that commit's files and cargo-deny accepts it. The
  same pin fails on `main`, on a tag, in a merge queue and where the run cannot be told to be
  a branch. A line at a branch, a short commit, or lines at different commits fail everywhere.
  The shared workflows are unchanged and `workflows-v1` does not move for this.
- **An app's own git sources in `deny.toml`.** The file stays the core's but for the lines
  between `# BEGIN app sources` and `# END app sources` in `allow-git`. `cargo xtask sync`
  writes the addresses of the app's `providers` there, and `sync --check` fails when they are
  not exactly those. The core's own file has nothing between the two lines.
- **More shared settings, as marked blocks** in files that are otherwise the app's, between
  `# BEGIN open-source-core: NAME` and `# END open-source-core: NAME`:
  - `Cargo.toml`, `lints-and-profiles`: `[workspace.lints.rust]`, `[workspace.lints.clippy]`
    and the three build profiles. The core's own `Cargo.toml` carries the same block;
  - `.cargo/config.toml`, `cargo`: the line that reads a dev-link, and the `xtask` alias;
  - `.gitignore`, `ignore`: `/target`, `/dist`, `/.private`, `/.cargo/dev-link/`, `.DS_Store`,
    `*.swp`;
  - `.gitattributes`, `attributes`: `* text=auto eol=lf`.
  `xtask/Cargo.toml` is a managed file.
- **One rule in the `app-and-core` block: when a promotion is finished** (decision 0013,
  amended; issue #30). Code promoted from an app is finished only when the app it came from
  uses the core's copy and its own copy is deleted. The core issue stays open until then, and
  the `core-candidate` marker on the app's copy names it.

- `jcj-doc`: the word list and the word-level comparison, where a crate with no engine and no
  layout can use them (core issue #31). They were in `jcj-features`, which needs `jcj-engine` and
  `jcj-layout`, so an app's model crate kept its own copy of both.
  - `jcj_doc::verify` is the whole word-for-word check: `words`, `paragraph_words`,
    `document_words`, `compare`, `Difference`, `Check`, `CONTEXT`. `jcj_doc::words` is the word
    list again at the crate's root, beside `count_words`.
  - `jcj_doc::wordiff` is the whole comparison: `Change`, `Segment`, `Diff`, `tokens`,
    `paragraph`, `paragraphs`, `documents`.
  - `jcj_features::verify` and `jcj_features::wordiff` are the same items under the old names:
    the same types, fields and results, so nothing that uses them changes.
  `core-testbed` lists words and compares two lists of paragraphs through `jcj-doc` alone, and
  still checks its exports and compares documents through `jcj-features`.
- `jcj-shell`: `jcj_shell::i18n` gives the translation lookups beside `set_language` (core issue
  #32): `tr`, `tr_with`, `trn`, `tl`, `language`, and for a test of an app's catalog `Catalog`,
  `Entry`, `texts_in`, `singulars_in` and `compare`. They are `jcj-i18n`'s own items, so an
  interface crate that already depends on `jcj-shell` needs no second dependency to translate
  its texts. No behaviour changes. `core-testbed` no longer names `jcj-i18n`: it marks its
  labels with `tl`, shows a count with `trn`, and a test holds its catalog to its window's code.
  - Not added: a way to translate an app's texts while the shell's stay in English. One language
    holds for the whole window, because the shell shows the labels an app gives it through the
    same lookup as its own texts; installing only an app's catalog would turn the shell texts
    that share a key with it and leave the rest. An app that moves its texts onto the core's
    catalogs gets the shell's in that language too, and says so in its own changelog.
- `jcj-interchange`: the EPUB writer's notes are the caller's to choose, and the writer says
  which words it added (core issue #33). `export_epub(doc, &EpubOptions) -> EpubExport`:
  - `EpubOptions::default().note_numbering(NoteNumbering::Continuous)`: notes are numbered once,
    from the first chapter file to the last, instead of from 1 in each (`PerChapter`, the
    default);
  - `.note_backlink(..)`: the link under each note is `NoteBacklink::Text("…")` in the
    document's language, `NoteBacklink::Symbol { label }` (a return arrow with a name for screen readers,
    which adds no word), or `NoteBacklink::None`; the default is the word "Back";
  - `.note_heading("…")`: a heading above each chapter's notes (none by default);
  - `EpubExport { bytes, added_words }`: the file, and the words in its chapters that are the
    writer's and not the document's, in order: each note's mark where it is called, then at the
    chapter's end the heading's words and, for each note, its mark again and the link's words.
    A mark stands directly after the word before it, so a check that reads the file back by
    white space compares letters, as a PDF check does.
  `export(Format::Epub, doc)` and `export_epub(doc, &EpubOptions::default())` write the same
  bytes as before: a test holds the content of every file in the package to a hash taken before
  the change. `core-testbed` writes a book with all three choices and checks its words.
- `jcj-shell`: the rule for a shifted key with no command of its own (core issue #29).
  `KeyMap::command_for(shortcut) -> Option<Cmd>` is what a key press runs: the command bound to
  it, or else what `KeyMap::shifted(shortcut)` gives: when the key without Shift runs a caret
  movement (an id starting `keys::CARET_COMMANDS`, `caret.`), the same movement with
  `{"extend": true}`; when it deletes back (`keys::BACKSPACE`, `text.backspace`), the same. A
  key the user turned off runs nothing. `bound_to` and the shell's own key handling are
  unchanged: the shell still takes only keys with Mod, Ctrl or Alt and function keys that are
  bound, and leaves the rest to the document, whose view asks `command_for`. `core-testbed`
  runs Shift+Right through the rule on a session and sees the selection grow.
- `jcj-shell`: the system's file dialogs, asked for through the shell and answered as a command
  (core issue #28). Until now an app showed them itself, modally, or asked for a typed path.
  - `Shell::ask_file(FileAsk, then: Cmd) -> bool` queues a dialog; when the user has chosen,
    `then` runs with `"path"` (the file, folder or place to save) and `"paths"` (every file
    chosen) added to its parameters. Closing the dialog runs nothing, unless
    `Shell::ask_file_or(ask, then, cancelled)` named a command for that. One dialog shows at a
    time; a shell holds at most `files::MAX_WAITING` for its window.
  - `files::FileAsk`: `open()`, `open_many()`, `save()`, `folder()`, then `.title(..)`,
    `.filter(name, &[extensions])` (any letter case), `.in_folder(..)`, `.file_name(..)`.
  - `files::FileDialogs`: the window's part. The app's window keeps one beside its shell and
    calls `files.show(&mut shell, ui.ctx(), Some(&*frame))` once at the end of every frame,
    after `Shell::ui`. The shell does not own the window, so without this call a dialog asked
    for never opens. The dialogs live there and not in the shell because a dialog on screen
    belongs to the thread that showed it: `Shell` is still `Send`, and a test says so.
  - `FileDialogs::answer_with(..)` answers in the system's place (`files::FileAnswer`) and
    `FileDialogs::tick(&mut shell, ctx)` needs no window, for tests and automation;
    `Shell::file_asks_waiting()`, `FileDialogs::is_open()` and `waiting()` say what is pending.
  - The dialogs are the upstream layer's (craft-dialogs), which keeps the window drawing while
    one is up. `jcj-shell` gains one direct dependency, `raw-window-handle` 0.6.2, for the
    window's handles; it was in every app's lock file already. A dialog that cannot be shown
    is said in the status bar (one new text, in the shell's Spanish catalog too).
  `core-testbed` has Open, Save As and Export To through it, and a test answers them.
- **One convention for command lines and agent tools** (decision 0017, new;
  `docs/command-lines-and-agent-tools.md`, new). The three apps' command lines each chose their
  own exit codes, `mcp` arguments and tool names. The page decides them:
  - exit codes 0 done, 1 failed, 2 usage, **3 a check of the content did not pass** (two texts
    differ, a check blocks an export), so a script can tell "they differ" from "the check did
    not run";
  - no argument is a usage error (the usage on standard error, code 2); `--help`, `-h` and
    `help` give the usage with code 0; `--version` and `-V` print `<program> <version>`;
  - `<program> mcp [<file>] [--read-root <folder>] [--write-root <folder>]`, the two flags being
    the folders an agent's commands may read and write;
  - every tool that is an app's own is named `<app>_<command>`.
- `jcj-agents`: the code for that convention, and the adapter two apps each wrote.
  - `jcj_agents::cli`: `Exit` (`Done`, `Failed`, `Usage`, `Differs`, with `code()` and
    `Into<ExitCode>`), `standard(&args) -> Option<Standard>` (`Help`, `Version`, `NoCommand`),
    `version_line(program, version)`, and `McpArgs` (`parse`, `USAGE`, `roots()`, and
    `confine(program)`, which puts the roots in force for the process or says on standard error
    that nothing is confined).
  - `AppCommands` (a trait: `commands() -> Vec<AppCommand>`, `run(id, params)`) and `AppCommand`
    (`new(id, label)`, `.params(..)`, `.read_only()`, `.reads(&[..])`, `.writes(&[..])`): an
    app's own commands, for an app whose commands run on its own state (a project) and not on a
    document session. `AgentServer::for_app(Box<dyn AppCommands>, prefix, config)` serves them
    and nothing else: each is a tool named `<prefix>_<command>`; the files its declared
    parameters name are checked against the agent's folders before it runs; the general
    document tools are left out, because there is no document behind them.
  - `AgentServer::with_app_commands(..)` adds such commands to a server over a document;
    `.tool_prefix("app")` names every tool of the app's own (exported commands too)
    `app_<command>`; `.without_general_tools()` leaves the general document tools out;
    `tool_names()` lists the app's own tools; `app_tool_name(prefix, id)` is the naming rule.
  - `AgentServer::headless`, `AgentConfig` and every existing tool name are unchanged: all of
    this is asked for by the app.
  `core-testbed` follows the page (`--version`, `verify` with code 3, `mcp` with a file and the
  two flags, its tools named `testbed_…`, the board's commands through the adapter), and
  `apps/core-testbed/tests/command_line.rs` runs the built program to hold it there.

### Changed

- **One run of `cargo xtask sync` is enough.** A release that changed `sync.rs` or its lists
  used to be written by the app's older copy of them, and asked for a second run. A run that
  changed any shared xtask source now builds the new xtask, in `target/xtask-sync`, and runs it
  once more. The folder of its own is what makes that work on Windows, where a program cannot
  be replaced while it runs. The move to this release is the last that needs a second run by
  hand, because the program that starts it is the old one.
- **`sync` reads everything from the core before it writes anything**, and refuses a core that
  lacks a file or a block it manages: that core is an older release than the app's shared
  files are from, and writing half of it would leave an xtask that does not build. It says to
  move the pin or to link a checkout. A file that is already right is not rewritten.
- The `app-and-core` block's steps for a core change an app needs say what the link is, name
  `cargo xtask ci --linked` and gain the temporary pin as their step 5. "Something one app
  needs from another app" says how a provider is linked. The `app` block lists what the core
  owns in three groups and says what `main.rs` holds and where the layers are.
- `cargo xtask ci` in an app prints `(skipped)` for a task left out and says which packages a
  `--changed` run covers. With `--changed` and no `--base`, the base is `origin/main` when the
  clone has it and `HEAD` otherwise (it was always `origin/main`).
- What the unified checks ask that an app's own did not:
  - `assets`: a path counts only in backticks (Wordsmith Script's check took a bare path); the
    legal files are checked in every app, `SECURITY.md` among them (Wordsmith Manuscript's
    check had none); files Git would add count, not only tracked ones (Wordsmith Book's read
    tracked files); the folder `assets/` itself names nothing.
  - `app-layers`: every package is in the table, programs included (Wordsmith Manuscript's
    left `apps/` out); a core crate no row names fails (Wordsmith Book's checked only the
    crates its code listed); Wordsmith Script had a table and no check.

### Fixed

- In the core: the core's own files carry the `attributes` block too. With the block only
  under `template/`, a Windows checkout had LF there and CRLF in the core's copies of the
  marked blocks, and the `template` check failed.

- **A dev-link to a core checkout of another version than the pinned one linked nothing.**
  Cargo keeps what a lock file holds, so the pinned packages stayed and the `[patch]` entries
  were reported unused. The lock file of the linked build now starts as the app's own without
  the packages of the linked addresses, so exactly those are resolved again, to the checkout,
  and every other dependency stays where the app pins it.
- `dev-link` patched every crate of the core, used or not, which made Cargo warn about each
  unused one on every command. It patches the packages the app takes from the address.
- `deny.toml` named an app's repository so that another app could depend on it, and the comment
  above the workspace lints named a product. The core's files name none (decision 0013).
- The core's `.gitignore` ignored a dev-link file no tool ever wrote.

### What apps must do

**The tooling.**

Steps 2 to 9 change files only; nothing in an app's
product code changes. An app that stops after step 5 has a working gate with its old
`main.rs`; `sync` says on every run that it is not on the shared gate yet.

1. Move every `jcj-` dependency to `tag = "v0.1.20"`, then run
   `cargo metadata --format-version 1 >/dev/null` to bring `Cargo.lock` in line.
2. Run `cargo xtask sync`. The program that runs is the app's older one: it writes the files
   it knows and says to run it again.
3. Run `cargo xtask sync` again. It adds `xtask/src/shared.rs`, `gate.rs`, `applayers.rs` and
   `assets.rs`, runs itself once more, and ends with "marked block(s) have no place yet",
   naming four blocks (three in an app with no `.gitattributes`: the file is written whole).
   That is expected. From here on a core release needs one run.
4. Give each of those blocks its place: put its BEGIN line above, and its END line below, the
   lines the block replaces. Do not delete those lines yet: `cargo` needs the `xtask` alias and
   the lint tables to run the next step, and `sync` replaces whatever is between the two lines.
   - `.cargo/config.toml`: `# BEGIN open-source-core: cargo` as the first line of the file,
     and `# END open-source-core: cargo` after the line `xtask = "run --quiet --package xtask --"`.
   - `Cargo.toml`: `# BEGIN open-source-core: lints-and-profiles` above `[workspace.lints.rust]`,
     and `# END open-source-core: lints-and-profiles` after the last line of
     `[profile.release]`. The five tables between them (`[workspace.lints.rust]`,
     `[workspace.lints.clippy]`, `[profile.dev.package."*"]`, `[profile.dev]`,
     `[profile.release]`) become the core's; a table of the app's own that stands among them
     moves below the END line.
   - `.gitignore`: `# BEGIN open-source-core: ignore` and `# END open-source-core: ignore`
     around the lines `/target`, `/dist`, `/.private`, `.DS_Store` and `*.swp` and their
     comments. Any other line is the app's and goes outside.
   - `.gitattributes`: `# BEGIN open-source-core: attributes` and
     `# END open-source-core: attributes` around the line `* text=auto eol=lf` and its comment.
     The app's own lines go after the END line, where they take precedence.
5. Run `cargo xtask sync`. It fills the four blocks. In an app that had no `.gitattributes`,
   run `git ls-files --eol | grep crlf`: a file listed there is stored with Windows line ends,
   and one that must keep them gets a line of the app's own after the block,
   `<path> -text`.
6. Put the app's layers in `AGENTS.md` as the table `app-layers` reads ("Added" above has the
   rules, and `template/AGENTS.md` an example): the line
   `<!-- BEGIN layer table (cargo xtask app-layers reads it) -->` above the table and
   `<!-- END layer table -->` below it; rows lowest layer first; package names and crate names
   in backticks; each core crate named once, in the lowest row that uses it, and marked
   `(only here)` where only that row may. Correct the text around the table: it is the rule
   now, and no constant in `xtask/` repeats it.
7. Replace `xtask/src/main.rs` with the app's data and its own tasks. The whole file, for an
   app with one task of its own:

   ```rust
   // SPDX-License-Identifier: MIT OR Apache-2.0
   //! `cargo xtask <task>` for <the app>, built on open-source-core.

   #![forbid(unsafe_code)]
   #![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

   mod shared;

   use std::path::Path;
   use std::process::ExitCode;

   use shared::gate::{self, App, Task};
   use shared::util::{TaskResult, cargo};

   const APP: App = App {
       product: "<the product's name>",
       shipped: &["<program>", "<program>"],
       screenshots: Some("<the package with the `screenshots` example>"),
       tasks: TASKS,
       ..App::DEFAULT
   };

   const TASKS: &[Task] = &[Task::other("wasm", "build the web program", |root, _| wasm(root))];

   fn main() -> ExitCode {
       gate::main(&APP)
   }

   fn wasm(root: &Path) -> TaskResult {
       cargo(root, &["build", "--locked", "-p", "<package>", "--target", "wasm32-unknown-unknown"])
   }
   ```

   - `PRODUCT` and `SHIPPED` become `product` and `shipped`. The second argument of
     `lintroots::check_lint_roots` becomes `no_indexing`. Legal files the app's `assets` check
     asked for beyond `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`, `ATTRIBUTION.md` and
     `SECURITY.md` become `legal_files`. The package of the app's `screenshots` task becomes
     `screenshots`; leave the field out when the app has no such task.
   - An app that builds on another app's crates adds
     `providers: &[Provider { name: "<a short name>", git: "<the address its Cargo.toml names>" }]`
     and `use shared::link::Provider;`.
   - Delete the app's copies of what is shared now: `TASKS` as a list of names, `ci`,
     `run_task`, `scoped`, `workspace_root`, `check_layers`, `check_licenses`, the `match` in
     `main`, its own `assets` and `app-layers` checks with their tables, its `screenshots`
     function, and the tests of those. Delete every `mod` line but `mod shared;` and the app's
     own modules.
   - A check of the app's own that the gate ran stays, as `Task::check("name", "help", function)`;
     a task run by name is `Task::other(...)`. The function takes the workspace's folder and
     the arguments after the task's name. A task may not take a shared task's name.
   - In a module of the app's own beside `main.rs`, `crate::util` becomes `crate::shared::util`,
     and so for every shared module; `crate::SHIPPED` and `crate::PRODUCT` become
     `crate::APP.shipped` and `crate::APP.product`.
8. Run `cargo xtask sync` once more. In an app with `providers` it writes their addresses
   into `deny.toml`, which step 2 emptied of them. Then `cargo xtask sync --check`.
9. Run `cargo xtask ci` and fix what the two unified checks report ("Changed" above says what
   each asks that the app's own did not): in `ATTRIBUTION.md`, put an asset's path in
   backticks; in the layer table, name a core crate a row uses, or give a program its row.
10. Correct what the app's own documents say: that the dev-link is written into
    `.cargo/config.toml`, must be removed before a commit, or leaves `Cargo.lock` to restore;
    that the layers are also a constant in `xtask/`; which files of `xtask/src/` are the
    app's. Run `cargo xtask install-hooks` in a clone whose hooks are off.

An app that follows another app's tag, and so cannot move its pin until that app has, can take
the files first, as before: in a core checkout at this release, `cargo xtask sync <the app's
folder>` does what steps 2, 3, 5 and 8 do, and the app writes its providers' addresses between
the two `app sources` lines of `deny.toml` by hand, one quoted address and a comma per line.
Until its pin has moved, the app's own `cargo xtask sync` and `sync --check` say that the
pinned core is an older release and write nothing; `cargo xtask dev-link <that checkout>` and
`cargo xtask ci --linked` run its gate meanwhile.

**The crates.** Nothing is required: every addition to a crate above is one an app asks for.
An app that only moves its pin builds and behaves as before, and no existing call writes
different output.

What an app may now delete or switch to, each in a change of its own. The core issue named stays
open until the app that holds the copy uses the core's.

1. **A model crate's own word list and word-level comparison** (#31). Replace the copy of the
   comparison with `pub use jcj_doc::wordiff::{Change, Diff, Segment, paragraph, paragraphs,
   tokens};` and keep the app's own entry point over its own text type, calling `paragraphs`.
   Make the app's word list call `jcj_doc::words`. A test that only compared the copy with the
   core's can go. Wordsmith Manuscript: `crates/manuscript-model/src/diff.rs` and `words.rs`
   (`words`), and `crates/manuscript-io/tests/words.rs`. Nothing a crate built on that model
   sees changes: the types have the same names, fields and results.
2. **A direct `jcj-i18n` dependency in an interface crate** (#32). Call `jcj_shell::i18n::tr`,
   `tr_with`, `trn`, `tl` and `language`, write the catalog test with `jcj_shell::i18n::Catalog`,
   `texts_in`, `singulars_in` and `compare`, and drop `jcj-i18n` from that crate's `Cargo.toml`
   and from the app's layer table. Wordsmith Book: `crates/book-ui`. An app with catalog code of
   its own (Wordsmith Manuscript: `crates/manuscript-ui/src/i18n.rs`) can delete it and call
   `jcj_shell::i18n::set_language(code, &[("es", CATALOG)])`; the shell's own texts then follow
   the language too, which users see, so that app says so in its changelog and guide.
3. **Notes written into the document before the EPUB writer is called** (#33). Call
   `jcj_interchange::export_epub` with the numbering, link and heading the reading copy has
   today, and take the words to leave out of the word check from `added_words`. Wordsmith
   Manuscript: `notes_as_endnotes` in `crates/manuscript-io/src/export.rs`, which numbers
   through the book under a "Notes" heading with no link back:
   `EpubOptions::default().note_numbering(NoteNumbering::Continuous).note_heading("Notes")
   .note_backlink(NoteBacklink::None)`. Readers of the file will see three differences from
   that workaround, all on purpose: a note's number where it is called is a link to the note,
   the notes are a numbered list marked as endnotes for reading systems, and "Notes" is a
   heading where it was a bold paragraph. Update the golden file with them.
4. **An app's own rule for shifted caret keys** (#29). Where the document view asks the key map
   what a key runs, call `map.command_for(&shortcut)` in place of `map.bound_to(..)` followed by
   the app's rule. Wordsmith Script: `shifted` in `crates/wordsmith-ui/src/editor.rs`. One
   difference: a shifted key the user turned off now runs nothing.
5. **File dialogs an app shows itself** (#28). Ask with `shell.ask_file(FileAsk::open()…, Cmd)`
   and handle the command that comes back with `"path"`; add a `jcj_shell::files::FileDialogs`
   to the window beside the shell and call `files.show(&mut shell, ui.ctx(), Some(&*frame))`
   after `shell.ui(..)`. Then drop the app's own dialog dependencies. Wordsmith Script:
   `apps/wordsmith/src/files.rs` and its `rfd` and `raw-window-handle` lines. Wordsmith Book's
   typed path box can become an Open dialog the same way.
6. **An app's own adapter from its commands to agent tools** (decision 0017). Implement
   `jcj_agents::AppCommands` for the value the commands run on (`commands()` from the app's
   command list, with `reads` and `writes` naming the parameters that are paths; `run` calling
   the app's own dispatch, and saving as it does today), and build the server with
   `AgentServer::for_app(Box::new(tools), "<app>", config)`. The engine commands registered only
   to stand for app commands, the macro that lists them and the global the project was kept in
   all go. Wordsmith Manuscript: `crates/manuscript-ui/src/agents.rs`. Wordsmith Book:
   `crates/book-ui/src/agents.rs`. The tool names stay as they are (`manuscript_…`, `book_…`).
   One difference agents see: the general document tools (`execute`, `list_commands` and the
   rest), which worked on an empty document, are no longer listed.
7. **Follow `docs/command-lines-and-agent-tools.md`** with `jcj_agents::cli`: `Exit` for the
   exit codes, `standard(&args)` for `--help`, `--version` and no argument, `McpArgs` for `mcp`.
   Each app changes something a user of its command line can see (an exit code, a tool's name,
   what no argument does), so each does it in a change of its own with a changelog line, a guide
   line and a test that runs the built program, as `apps/core-testbed/tests/command_line.rs`
   does.

## 0.1.19 (2026-11-26)

### Added

- **An app's CI is the core's shared workflow, called at a tag the core moves** (decision 0003,
  amended). Until now an app's `ci.yml` was a whole workflow copied by `cargo xtask sync`, so a
  fix to CI reached an app only with a core release and a pin move, and apps on different pins
  ran different generations of CI.
  - The jobs are two reusable workflows in the core's `.github/workflows/`: `shared-ci.yml`
    (checks, cargo-deny, clippy and tests) and `shared-core-pin.yml` (the weekly look at the
    pin).
  - What an app carries is two short managed files, `.github/workflows/ci.yml` and
    `core-pin.yml`. They say when to run and which token the jobs get, and call the shared
    workflows at the tag `workflows-v1`.
  - The core moves `workflows-v1` to a commit of its `main` on which its own CI has passed; the
    core's `ci.yml` calls `shared-ci.yml` from its own commit, so a change is tried there first.
    One tag move carries a CI fix to every app, with no release and no change in an app.
    Nothing reaches an app by landing on the core's `main`.
  - `workflows-v1` may ask of an app only what every core release from this one on gives it;
    the core's `.github/workflows/README.md` has the list. A workflow that needs more gets a
    new tag, named by the two files from the release that brings what it needs.
  - A called workflow runs in the app: its code, its commit, its read-only token, no secrets.
    The actions it uses are pinned to full commits in the core, where Dependabot updates them.
- **`cargo xtask affected [--base <ref>]` in every app** (the core had it): the packages a
  change affects, as one line (`all`, names, or empty), and why each is in it on standard
  error. The shared module is `xtask/src/changed.rs`.
- **`cargo xtask ci --only a,b`** runs those tasks alone; `--skip a,b` is as before. CI's build
  job uses `--only clippy,test`.
- **Dependabot settings for every app**, `.github/dependabot.yml`, a managed file: weekly update
  proposals for GitHub Actions, so the commits an app's own workflows pin do not go stale. Cargo
  is not listed: the `jcj-` crates move with the core pin, and a crate only the app uses moves
  when the app refreshes its lock file. The core keeps its own file, with Cargo.
- Two sentences in the marked blocks. `public`: the Foundation's name is always written "The
  James C. J. Foundation", with its capital "The" wherever it stands in a sentence, never in a
  shorter form. `contributing`: a pull request opened by Dependabot is merged by anyone with
  write access once its checks pass.
- In the core: `nightly-failed.yml` opens an issue when a scheduled nightly run has failed.

### Changed

- **`--changed` tests everything a change touches.** A file used to count only for the package
  whose folder holds it, so a change to a built-in design, a font, a fixture or a golden file at
  the root, or to a page of `docs/` that a test compares, ran no clippy and no test, in CI and
  in the pre-push hook, and was reported green. Now:
  - a changed file also counts for every package whose Rust sources name it, or a folder it is
    in: the argument of `include_str!`, `include_bytes!`, `include!` and `#[path = ...]` from
    the source file's folder; any other string literal from the package's folder
    (`"../../tests/fixtures"`); and a literal with a `/` or a `.` in it from the root as well
    (`root().join("docs/guide/keys.md")`);
  - a changed file under the root `assets/` or `tests/` that no package names affects every
    package;
  - the `xtask` package is left out: the files it names are read by its checks, which always
    run.
  The sources are read as text, so this is a guess that errs toward testing. It cannot see a
  path put together from bare words (`root().join("docs").join("guide")`) or from values that
  are not literals.
- **CI runs on every push and pull request, a change to Markdown alone included.** Such a push
  used to start no workflow, so the checks of the marked blocks, the links and the privacy scan
  never ran for the changes they exist for. The checks job always runs. The build job asks
  `cargo xtask affected` first and ends without restoring a build cache when the change affects
  no package.
- **A task an app adds to its gate runs once in CI**, in the checks job. The build job named
  the tasks to skip, could not know an app's own, and ran them a second time (`docs` too).
- The cargo-deny job restores the cargo home, so git dependencies are not cloned again on every
  run.
- The `app` block lists `.github/dependabot.yml` among the files the core owns and says that
  the two workflow files only say when CI runs.
- The `security` and `contributing` blocks read true in a repository that has no `cargo xtask`,
  no changelog or no releases (a website, say): "the repository's gate", "where the repository
  keeps a changelog", "as the repository's license files say".
- `deny.toml` cited `template/xtask/src/spdx.rs`, a path that exists only in the core; it now
  names the shared module.
- The Foundation's name is written in full in the core's `README.md`, `NOTICE`, `AGENTS.md` and
  `template/AGENTS.md`.

### Fixed

- The opening comment of `core-pin.yml` said to run the workflow by hand after GitHub has
  switched its schedule off, which it does when a repository is quiet for 60 days. The
  documented step is to enable it again, `gh workflow enable core-pin.yml`, and the comment now
  says so, and that nothing tells anyone it was off (`gh workflow list --all` shows it).
- In the core: the upstream train ran on even weeks of the year, which leaves three weeks
  between two runs after a year of 53 weeks (2026 is one); it now counts whole weeks. The
  release and train workflows no longer say a person approves a release (decisions 0004 and
  0011).
- The cache step of the app `ci.yml` listed `target` with a different indent from the lines
  around it. The step is now in `shared-ci.yml`, with one indent.

### What apps must do

1. Move every `jcj-` dependency to `tag = "v0.1.19"`, then run
   `cargo metadata --format-version 1 >/dev/null` to bring `Cargo.lock` in line.
2. Run `cargo xtask sync`. It brings the new `sync.rs`, `changed.rs` and `util.rs`, the two
   short workflow files and the reworded blocks, and says to run it again. First:
3. Add one line to the app's `xtask/src/main.rs`, in the `match task` of `main`, after the
   `"core-pin" => corepin::task(&root, rest),` arm:

   ```rust
           "affected" => changed::task(&root, rest),
   ```

   `cargo xtask sync` says so while the line is missing, and CI's build job fails without it.
4. Run `cargo xtask sync` again. This run adds `.github/dependabot.yml`.
5. Leave `.github/workflows/ci.yml` and `core-pin.yml` as `sync` wrote them. Under its opening
   comment, `ci.yml` is now this, and holds no job of its own:

   ```yaml
   name: CI

   on:
     push:
       branches: [main]
     pull_request:
     workflow_dispatch:
       inputs:
         full:
           description: "Run every test, not only what changed"
           type: boolean
           default: false

   permissions:
     contents: read

   concurrency:
     group: ci-${{ github.ref }}
     cancel-in-progress: true

   jobs:
     gate:
       permissions:
         contents: read
         actions: read
       uses: jamescjfoundation/open-source-core/.github/workflows/shared-ci.yml@workflows-v1
       with:
         full: ${{ inputs.full == true }}
   ```

   An app's other workflows (a release workflow, say) are its own and do not change.
6. Delete a test of the app's own that reads `.github/workflows/ci.yml` for the task names it
   passes to `--skip`. The file names no task now, so such a test fails, and what it guarded
   against can no longer happen. Wordsmith Script's `xtask/src/main.rs` has one,
   `the_core_workflow_skips_only_tasks_that_exist`.
7. Dependabot: `sync` has replaced an app's own `.github/dependabot.yml` (Wordsmith Script has
   one; its Cargo updates and ignore rules end here, on purpose). Delete a settings file kept
   under the other spelling, `.github/dependabot.yaml`: GitHub would read one of the two.
   Correct any text in the app that describes the old settings.
8. Write the Foundation's name as the `public` block now says, everywhere in the app: its
   documents, `NOTICE`, the text the app shows, the properties compiled into its programs. A
   golden file that holds the old form changes with it, on purpose. No task checks this.
9. Correct what the app's own documents say about CI: a push that changes only Markdown now
   runs the checks, and the jobs are in the core. A test that reads a file outside its package
   should name it in one string literal (`"../../docs/format.md"`, or
   `root().join("docs/format.md")`), not piece by piece, so that a change to the file runs the
   test.
10. `cargo xtask sync --check`, then `cargo xtask ci`. `cargo xtask affected --base origin/main`
   shows what the next push will build and why.
11. After the push, look at the CI run: its jobs are named `gate / ...` and come from the
   shared workflow. The first run builds without a cache, because the cache's list of paths
   changed. A repository that requires status checks by name must name the new ones.

No change to an app's product code. The tag `workflows-v1` exists from this release on; an app
on an older core keeps its own full `ci.yml` and is not touched. An app that follows another
app's tag, and so cannot move its pin until that app has, can take the files first: in a core
checkout at this release, `cargo xtask sync <the app's folder>` writes them, and step 3 makes
the build job of its CI work. The app's own `cargo xtask sync --check` passes once its pin has
moved.

## 0.1.18 (2026-11-26)

### Added

- **`cargo xtask docs`, a task of the gate in the core and in every app** (decision 0003,
  amended). The routes between repositories are prose, and nothing noticed when a file moved, a
  record was renumbered or a marker never got its issue. The task reads the tracked Markdown
  files, leaving out `docs/history/` and fenced code blocks, and fails on:
  - a relative link (a Markdown link, an image, a reference definition, an `href` or `src`) whose
    target is not a tracked file or folder, or whose `#fragment` is not a heading or `id` of the
    Markdown file it points at;
  - a decision reference that names no record: "core decision NNNN" in the core's
    `docs/decisions/` (in an app, the pinned release's), a bare "decision NNNN" in the
    repository's own. "project decision NNNN", "Script's decision NNNN" and the like are somebody
    else's and are left alone, and so is a bare number inside a marked block;
  - in an app, a `// core-candidate:` comment that names no core issue (`#12`,
    `open-source-core#12` or the issue's address).
  The shared module is `xtask/src/docs.rs` (`template/xtask/src/docs.rs` here).
- In the core: `cargo xtask docs <folder>` runs the same checks on another checkout. It is the
  only way a repository without Cargo is checked.
- **One pull request template for every app**, `.github/pull_request_template.md`, a managed
  file. It asks what another repository must take or make first, what a user will see, and
  whether brand files were changed at their source. The core has its own.
- **`cargo xtask ci` says when a clone's git hooks are off.** A fresh clone and a fresh worktree
  start without them. It is a warning, not a failure, and silent in CI.
- **`pre-commit` runs `cargo xtask unlinked`**, so a local core dev-link is refused at the
  commit, not only at the push.

### Changed

- Wording inside the marked blocks; `cargo xtask sync` brings it. `app-and-core`: what belongs in
  the core now lists entries kept beside documents and interface translations (decision 0016),
  and the rule against product names says where it holds: the core's code, interface text and
  `template/` (decision 0013). `app`: the brand may live in a project's brand kit with copies in
  the app, the app's interface theme is its own `tokens.json`, and the pull request template is
  among the files the core owns. Every block's first line says "its tooling rewrites the
  copies", since the copies in the core and in a repository without Cargo are written by other
  commands than `cargo xtask sync`.
- `template/Cargo.toml` no longer lists `serde`, which nothing in the template used.

### What apps must do

1. Move every `jcj-` dependency to `tag = "v0.1.18"`, then run
   `cargo metadata --format-version 1 >/dev/null` to bring `Cargo.lock` in line.
2. Run `cargo xtask sync` twice. The first run brings the new `sync.rs`, the reworded blocks
   and the pre-commit hook, and says to run it again; the second adds `xtask/src/docs.rs` and
   `.github/pull_request_template.md`.
3. Add four lines to the app's `xtask/src/main.rs`. `cargo xtask sync` names the ones still
   missing each time it runs.

   ```rust
   mod docs;
   ```

   beside `mod corepin;`.

   ```rust
       ("docs", "links in Markdown lead somewhere, decision references name a record, core-candidate markers name an issue"),
   ```

   in `TASKS`, after the `("sync", ...)` row.

   ```rust
           "docs" => docs::app_task(root),
   ```

   in `run_task`, after the `"sync" => ...` arm.

   ```rust
       util::warn_if_hooks_off(root, "hooks");
   ```

   in `ci`, after the line `let skip = util::skipped(&skip_args, &names)?;`.
4. Delete a pull request template the app keeps under another name
   (`.github/PULL_REQUEST_TEMPLATE.md`, a file in `docs/` or at the root): GitHub would offer
   only one of the two. Move a question that is only the app's into `CONTRIBUTING.md`.
5. Run `cargo xtask docs` and fix what it reports:
   - a link that leads nowhere: correct the path, or give the full address when the target is
     in another repository;
   - a decision reference that names no record: write "core decision NNNN" for a record of the
     core, name the owner for another repository's ("project decision NNNN"), or correct the
     number;
   - a `core-candidate` marker without an issue: open the core issue (label `core-candidate`)
     and put its number in the comment, `// core-candidate: open-source-core#N ...`.
6. `cargo xtask sync --check`, then `cargo xtask ci`. Run `cargo xtask install-hooks` if the
   gate says the hooks are off.

No code changes. An app that follows another app's tag, and so cannot move its pin until that
app has, can take the files first: in a core checkout at this release,
`cargo xtask sync <the app's folder>` writes them and `cargo xtask docs <the app's folder>`
checks the app. The app's own `cargo xtask sync --check` passes once its pin has moved.

## 0.1.17 (2026-11-26)

### Added

- **The core's rules reach every app by `cargo xtask sync`, not by hand** (decision 0003,
  amended). Until now the rules every app carries were about 115 lines copied by hand into five
  files, and `SECURITY.md`, `CONTRIBUTING.md` and the code of conduct were written separately
  in each repository; nothing noticed a copy that had fallen behind.
  - A marked block is the text from a line `<!-- BEGIN open-source-core: NAME -->` to a line
    `<!-- END open-source-core: NAME -->`. Its source is the same block in a file under
    `template/`. The text around a block is the app's own.
  - Five blocks (`BLOCKS` in `template/xtask/src/sync.rs`): in `AGENTS.md`, `app` ("Before you
    commit", "House rules for code"), `public` ("What is public") and `app-and-core` ("Building
    between an app and the core"); in `SECURITY.md`, `security` ("Reporting a problem", "How we
    handle a report"); in `CONTRIBUTING.md`, `contributing` ("Rules for a public repository",
    "Commits, pushes and pull requests").
  - `cargo xtask sync` replaces a block where its two marker lines are. A file an app does not
    have yet is written whole from `template/`. A file an app has without the marker lines is
    left alone and named in the error, after everything else is written: `sync` never chooses a
    place for a block.
  - `cargo xtask sync --check`, a task of every app's gate, names each file or block that is
    missing or differs from the pinned core release.
  - `CODE_OF_CONDUCT.md` is a whole managed file, worded for any project
    (`template/CODE_OF_CONDUCT.md`).
  - `template/SECURITY.md` and `template/CONTRIBUTING.md` are new: the core's block, and
    placeholders for the app's own parts.
- In the core: `cargo xtask template --write` copies the blocks from `template/` into the core's
  own `AGENTS.md`, `SECURITY.md` and `CONTRIBUTING.md`, and the code of conduct. The `template`
  task of the gate fails when one of them differs. The core carries every block but `app`: it has
  its own "Before you commit" and house rules for code.

### Changed

- A record of the core is cited as "core decision NNNN" in everything an app carries: linked in
  the marked blocks, and with the address of the core's `docs/decisions/` folder in `deny.toml`,
  `rust-toolchain.toml`, the shared xtask modules and the two workflows. A bare "decision 0003"
  in a copied file pointed at an app's own record of that number.
- The wording of the carried rules, so that one text is true in the core and in an app: "the
  core's `docs/decisions/`" for "here", "an app" for "this app". Three sentences say more than
  before: the brand "lives in the app's repository or in its project's brand kit, never in the
  core"; a security problem in the shared parts may be reported in the core or in the app, and
  the report is moved; "Things agents must not do" names editing a file or block the core owns.
- `template/AGENTS.md` no longer tells an agent to read the core's `AGENTS.md` as well: the
  blocks are that text.
- `README.md` and the opening of `AGENTS.md` say where a project built on the core keeps its map,
  and link the foundation's own.

### What apps must do

An app that carries the rules as hand copies does this once.

1. Move every `jcj-` dependency to `tag = "v0.1.17"`, then run
   `cargo metadata --format-version 1 >/dev/null` to bring `Cargo.lock` in line.
2. Run `cargo xtask sync`. It brings the new `sync.rs` and says to run it again.
3. In `AGENTS.md`, find the hand copy of the core's rules (from "## Before you commit" to the end
   of "## House rules for code"). Delete it and put these six lines in its place, in this order,
   each pair with one empty line after it:

   ```
   <!-- BEGIN open-source-core: app -->
   <!-- END open-source-core: app -->

   <!-- BEGIN open-source-core: public -->
   <!-- END open-source-core: public -->

   <!-- BEGIN open-source-core: app-and-core -->
   <!-- END open-source-core: app-and-core -->
   ```

   Keep the app's opening lines, a project block if the app has one, and the app's own rules.
   Remove "Read the core's AGENTS.md too" from the opening.
4. In `SECURITY.md`, delete the app's "## Reporting a problem" and "## How we handle a report"
   and put the pair `<!-- BEGIN open-source-core: security -->`,
   `<!-- END open-source-core: security -->` under the title. Keep "## What counts" below it,
   and move there any sentence of the deleted sections that is only about the app (what never
   to attach to a report, for example).
5. In `CONTRIBUTING.md`, delete the app's "## Rules for a public repository" and its commit and
   pull request section, and put the pair `<!-- BEGIN open-source-core: contributing -->`,
   `<!-- END open-source-core: contributing -->` there. Move any rule that is the app's own
   (a naming rule, a commit subject form, a progress file), or that only adds to what the block
   says, to a section below the pair; do not keep a second copy of a rule the block states. End
   the file with a line that points at `CODE_OF_CONDUCT.md` if it has none.
6. Run `cargo xtask sync` again. It fills the five blocks and replaces `CODE_OF_CONDUCT.md` with
   the core's. If it names a block it could not place, the marker lines for that block are
   missing or misspelled; each must be a line of its own.
7. Read the three files once through: text outside a block must not contradict the block, and a
   bare "decision NNNN" in the app's own text that means a core record becomes "core decision
   NNNN".
8. `cargo xtask sync --check`, then `cargo xtask ci`.

Nothing changes in an app's `xtask/src/main.rs`, and no code changes.

An app that follows another app's tag, and so cannot move its pin until that app has, can take
the files first: in a core checkout at this release, `cargo xtask sync <the app's folder>` writes
them, and `cargo xtask sync <the app's folder> --check` checks them. The app's own
`cargo xtask sync --check` passes once its pin has moved.

## 0.1.16 (2026-11-26)

### Added

- Shared xtask (`template/xtask/src/corepin.rs`, new, managed): `cargo xtask core-pin` checks
  that an app builds on one core release (decision 0011). Until now nothing did: an app could
  name two tags, hold two cores in its graph, or sit many releases behind unnoticed.
  - **One tag.** Every line of the root `Cargo.toml` that names the core's git address carries a
    release tag (`tag = "v0.1.20"`), all of them the same; the lines that differ are listed.
  - **One core in the graph.** `Cargo.lock` holds packages from one ref of the core's address.
    A second one is named with the packages built from it and the packages that bring them in
    (another app's crates that pin a different core, for example; decision 0015).
  - **Distance.** `cargo xtask core-pin --behind` lists the core's release tags
    (`git ls-remote`, so it needs the network), orders them as versions and prints, for example,
    `core pin v0.1.4 is 11 releases behind v0.1.15`, then `pinned=`, `newest=` and `behind=`
    lines for a workflow to read. `--max-behind <n>` fails when the pin is more than `n` behind.
  - The first two read two files and are a task of the template's `ci` (`core-pin`, after
    `unlinked`); the distance is never part of `ci`. A workspace with no core pin passes.
  - In the core, which has no pin of its own, `cargo xtask core-pin <app dir> [--behind]
    [--max-behind <n>]` looks at an app checkout, as `cargo xtask sync <app dir>` does.
- `template/.github/workflows/core-pin.yml` (new, managed): every Monday, and on demand, an app
  runs `cargo xtask core-pin --max-behind 3`. When the pin is more than three releases behind,
  the workflow opens an issue in the app, "Move to open-source-core vX.Y.Z", with the distance, a
  link to this changelog and the steps. While such an issue is open no second one is opened: it
  is renamed and rewritten when the core has released again. It uses only the run's own token
  (`issues: write`). CI stays green for a release an app does not need yet.
- Shared xtask (`template/xtask/src/sync.rs`): `cargo xtask sync` says when `sync.rs` itself
  changed, because the run that brings a longer list of managed files cannot yet copy the files
  the list adds; and `sync` and `sync --check` print a note while the app's own
  `xtask/src/main.rs` does not run the core-pin check.
- `cargo xtask template-check` runs the template's `core-pin` task.

### Changed

- The rules every app carries (`template/AGENTS.md`, from "Before you commit" to the end) follow
  the decisions recorded with this release: how a change lands and what "no product name" covers
  (0013), who cuts a release and how far behind an app may be (0011), upstream pins (0004,
  accepted), reviews and working notes (0012), apps that depend on apps (0015, new), and the
  entries and translation crates (0016, new). The port plan and the first review moved to
  `docs/history/`.

### What apps must do

- Move every `jcj-` dependency to `tag = "v0.1.16"`, then run `cargo xtask sync` **twice**: the
  first run brings the new list of managed files, the second the two files it adds
  (`xtask/src/corepin.rs` and `.github/workflows/core-pin.yml`). `cargo xtask sync --check` passes
  after the second. The patch block is unchanged.
- Add the check to the app's own `xtask/src/main.rs` (four lines; `template/xtask/src/main.rs`
  shows them in place). Nothing fails without them, but then nothing is checked, and the new
  workflow's run fails because the task is unknown:
  1. With the other `mod` lines: `mod corepin;`
  2. In `TASKS`, after the `unlinked` entry:
     `("core-pin", "every jcj- dependency names one core release tag, and Cargo.lock holds one core"),`
  3. In `main`, among the arms of `match task`, before the `name =>` arm:
     `"core-pin" => corepin::task(&root, rest),`
  4. In `run_task`, among the arms of `match name`: `"core-pin" => corepin::task(root, &[]),`
- The workflow arrives with the sync and needs no setting. It opens issues, so issues must be
  turned on in the repository.
- Replace the shared rules in the app's `AGENTS.md` (from "Before you commit" to the end of
  "House rules for code") with the same part of `template/AGENTS.md`. This is still a copy by
  hand; nothing checks it yet.
- An app whose lock file holds two cores fails `cargo xtask ci` once it has the task. That is the
  point of the check: move the dependency that brings the second core to a release built on the
  app's own core release (decision 0015) in the same change, or first.

## 0.1.15 (2026-11-25)

### Added

- `jcj-output`: `conformance` (`declare`, `Conformance`, `ConformanceError`, `srgb_profile`):
  PDF/A-2b and PDF/UA-1 declared on a tagged export as an incremental update: the
  identifications in the XMP metadata (PDF/UA's schema declared for PDF/A), an output intent
  with an sRGB profile made in code, `DisplayDocTitle`, and the language and author kept the
  same in the XMP and the information dictionary (core issue #27). Checked with the standard
  validator where it is installed (`JCJ_PDF_VALIDATOR`): both standards pass. `core-testbed`
  declares both on its sample.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.15"` when convenient (the patch block is
  unchanged).

## 0.1.14 (2026-11-25)

### Fixed

- `jcj-shell`: the Preferences page's auto-save interval and idle-time fields are named, for
  screen readers, by the words beside them (core issue #26). Before, they were unnamed spin
  buttons. A shell test and a `core-testbed` test check that no control on the File pages or the
  testbed's window lacks a name.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.14"` when convenient (the patch block is
  unchanged).

## 0.1.13 (2026-11-25)

### Added

- `jcj-i18n` (new crate): interface translation catalogs (core issue #18). One TOML file per
  language maps each text, keyed by its English, to the text shown; plurals take the language's
  forms (`trn`), `{name}` placeholders are filled (`tr_with`); catalogs are layered, the first
  that has a text giving it; a text no catalog has stays English. `texts_in`, `singulars_in` and
  `compare` let a test check that every text is in every catalog and none is unused.
- `jcj-shell`: `i18n` (`LANGUAGES`, `catalog`, `set_language`): the shell's own texts in Spanish,
  and a language turned on with the app's catalogs layered over the shell's. The shell shows its
  texts, and the labels, titles, tooltips, tab and group names apps give it, through the
  catalogs. `core-testbed` switches its window to Spanish (`CORE_TESTBED_LANG=es`).

### What apps must do

- Nothing. Until an app calls `jcj_shell::i18n::set_language`, every text is shown in English
  exactly as before. Move every `jcj-` dependency to `tag = "v0.1.13"` when convenient (the patch
  block is unchanged).

## 0.1.12 (2026-11-25)

### Fixed

- `jcj-interchange`: the EPUB writer keeps a document's footnotes and endnotes, as numbered
  endnotes at each chapter's end (custom marks kept), each linked from its reference and back,
  with `noteref` and `endnote` semantics. Before, they were left out (core issue #21).
  `core-testbed` checks that a footnote survives EPUB export and import.

### What apps must do

- Nothing. Apps that wrote notes into their EPUBs by hand to work around this can stop when
  convenient. Move every `jcj-` dependency to `tag = "v0.1.12"` (the patch block is unchanged).

## 0.1.11 (2026-11-25)

### Added

- `jcj-output`: `boxes` (`set_page_boxes`, `Boxes`, `BoxError`): trim and bleed boxes set on an
  exported PDF's pages as an incremental update (the original bytes kept), deterministic and
  bounded (core issue #25). `core-testbed` sets trim boxes and reads them back with `read_facts`.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.11"` when convenient (the patch block is
  unchanged).

## 0.1.10 (2026-11-24)

### Added

- `jcj-output`: `facts` (`read_facts`, `PdfFacts`, `PageFacts`, `PageBox`, `PdfImage`, `PdfFont`):
  a PDF's page boxes (media, crop, trim, bleed), rotation, annotations, images with their pixel
  and placed size (so their resolution), fonts with whether each is embedded, and bookmarks,
  read with the same bounds as `read_pages` (core issue #23). `core-testbed` checks that its PDF
  embeds every font.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.10"` when convenient (the patch block is
  unchanged).

## 0.1.9 (2026-11-24)

### Added

- `jcj-features`: `verify` (`words`, `paragraph_words`, `document_words`, `compare`, `Difference`,
  `Check`, `CONTEXT`): the word-for-word check of an import or export against its source, the
  first difference reported with the words around it. Promoted from Wordsmith Manuscript (core
  issue #10). `core-testbed` checks that every format it exports and imports keeps every word.
- `jcj-features`: `wordiff` (`tokens`, `paragraph`, `paragraphs`, `documents`, `Diff`, `Segment`,
  `Change`): two versions of a text compared paragraph by paragraph, then word by word, beside
  the paragraph-level `compare`. Promoted from Wordsmith Manuscript (core issue #17).
  `core-testbed` shows a changed word.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.9"` when convenient (the patch block is
  unchanged). Apps with their own copies can move to these in follow-up changes.

## 0.1.8 (2026-11-24)

### Added

- `jcj-shell`: `tree` (`tree`, `TreeRow`, `TreeState`, `TreeEvent`, `Zone`, `zone`): a tree of rows
  with selection, open and closed rows, rename in place, drag and drop before, after or inside a
  row, and a context menu event. The caller passes the visible rows and gets events back; only
  rows in view are drawn, and each is named as a tree item for screen readers. Promoted from
  Wordsmith Manuscript's binder for a second app's outline (core issue #6, C5 of Manuscript's
  plan, C3 of Book's). `core-testbed` shows the sample document's headings in a tree panel.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.8"` when convenient (the patch block is
  unchanged). An app with its own tree can move to this one in a follow-up change.

## 0.1.7 (2026-11-24)

### Added

- `jcj-doc`: `section` (`SectionStart`, `NumFormat`, `HeaderSet`, `Columns`, `NoteProps`,
  `NoteOptions`, `NotePos`, `NoteRestart`, `ChapterSep`, `PageVAlign`, `LineNumbering`,
  `LineNumberRestart`, `DocGrid`, `DocGridType`): the types of `SectionProps`' fields, so apps can
  start a section on an odd or even page, restart and format page numbers and set header and
  footer stories. `para::NoteKind`, so apps can add footnote and endnote references (core issue
  #24). `jcj-layout` tests that an odd-page start is by page number and that a footnote sits at
  the foot of its reference's page; `core-testbed` lays out both.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.7"` when convenient (the patch block is
  unchanged).

## 0.1.6 (2026-11-24)

### Fixed

- Shared xtask (`template/xtask/src/sync.rs`): `cargo xtask sync` and `sync --check` locate the
  core release the app's own workspace members depend on directly (the newest, should they name
  several), not the first `jcj-doc` in the graph. An app that depends on another app's crates
  pinning an older core builds both releases, and sync used to pick the older one (core issue
  #16). With one core in the graph nothing changes. New `sync::choose_core`, tested in the core's
  xtask.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.6"` when convenient and run
  `cargo xtask sync` to take the fixed module (the patch block is unchanged).

## 0.1.5 (2026-11-24)

### Changed

- `deny.toml`: `allow-git` also lists `https://github.com/jamescjfoundation/wordsmith-manuscript`,
  so an app may depend on Wordsmith Manuscript's published crates at a tag (core issue #15). No
  other source changes; the core's own `cargo deny check sources` passes.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.5"` when convenient and run
  `cargo xtask sync` to take the new `deny.toml` (the patch block is unchanged).

## 0.1.4 (2026-11-24)

### Added

- `jcj-entries` (new crate, layer 2): entries kept beside documents (a story bible, a cast list):
  `Entry<Id, B>` generic over the app's id and body types, `Template`, `TemplateField`,
  `FieldType` (entry types as data), `name_list`, `find_by_name`, `find_mentions` and `mentions`
  (whole words, matching case when a name starts with a capital), `backlinks` over `Document`s.
  Promoted from Wordsmith Manuscript's story bible for Wordsmith Script's characters to share
  (core issue #14, C13). `core-testbed` finds an entry by name in its sample document.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.4"` when convenient (the patch block is
  unchanged). An app that wants entries adds `jcj-entries` at the same tag.

## 0.1.3 (2026-11-24)

### Added

- `jcj-shell`: `split` (`Split`, `SplitState`, `split`, `panes`, `tabs`, `TabEvent`): two panes
  side by side or stacked with a draggable divider, and a strip of document tabs. Promoted for
  Wordsmith Manuscript's split editor (core issue #4).
- `jcj-interchange`: `epub_read`, and `import(Format::Epub, ..)` now reads an EPUB's chapters in
  spine order (core issue #13). `Format::Epub.can_import()` still returns false, so apps that list
  importable formats show the same list; an app that wants EPUB import asks for it by name.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.3"` when convenient (the patch block is
  unchanged).

## 0.1.2 (2026-11-24)

### Added

- `jcj-shell`: `start` (`TemplateSpec`, `chooser`, `start_window`): templates an app lists with
  the new `App::templates` show on the New page; with the new `ShellOptions::start_window` the
  shell shows templates, Open and recent files in place of the document while the new
  `App::has_document` is false. `ids::NEW_FROM_TEMPLATE` (`file.newFromTemplate`). Promoted from
  Wordsmith Script for Wordsmith Manuscript (core issue #2).
- `jcj-shell`: `writing` (`Writing`, `History`, `Sprint`, `Day`, `words_per_day`,
  `progress_bar`): a writing session's net words and target, a sprint timer, words per day,
  deadlines over writing days. Promoted from Wordsmith Script (core issue #3).

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.2"` when convenient (the patch block is
  unchanged). Apps that keep their own start window, templates or writing statistics can move to
  these in follow-up changes.

## 0.1.1 (2026-11-24)

### Added

- `jcj-output`: `read` (`read_pages`, `ReadLimits`, `PdfPage`, `PdfGlyph`, `GlyphMode`,
  `ReadError`): a PDF's pages and where each glyph lands, with its text, for importing PDFs and
  checking exported ones. Bounded for files that are not trusted (size, pages, glyphs a page).
- `jcj-interchange`: `export_rtf_direct`: RTF with each paragraph's resolved indents, spacing,
  alignment and page break, and each run's font, size, bold, italic and underline, written out
  directly, for apps that lay out their own paragraphs. `export(Format::Rtf, ..)` is unchanged.
- `jcj-engine`: `file_access::current` and `unconfined` (which folders are in force; run code with
  none).
- `jcj-proof`: `dictionary_generation` (changes when the user dictionary changes).

### Upstream

- craft-libs' `craft-pdf` reader (same commit as before) and its patched Hayro snapshot
  (`hayro-syntax`, `hayro-interpret`, `hayro-cmap`, `hayro-postscript`, `hayro-jbig2`), in the
  patch block (`docs/UPSTREAM.md`).

### What apps must do

- Move every `jcj-` dependency to `tag = "v0.1.1"`, then run `cargo xtask sync` to take the new
  patch block.

## 0.1.0 (2026-11-24)

The first release of open-source-core, built from the review of Wordsmith Script at `29ed4bf`
(`docs/history/script-29ed4bf.md`) on WordCraft v0.18.1.

### Upstream

- WordCraft v0.18.1 through our fork's `jcj` branch (`73717786`), carrying one patch: extension
  data on documents, paragraphs and runs, kept by resetting and clearing paragraph formatting.
- craft-libs at the commits WordCraft v0.18.1 pins; egui 0.37 and winit on storytold's forks.
- Pins, patches and known issues: `docs/UPSTREAM.md`.

### Crates

- `jcj-upstream`: the one crate that names upstream code (apps never use it).
- `jcj-doc`: the document model and typed extension keys (`ExtKey`, `HasExt`).
- `jcj-layout`: pages, page boxes, hit testing, measuring, page plans, the `PaginationPass` hook,
  bundled fonts.
- `jcj-engine`: sessions, undo, the registry (`registry_with`), recovery, native JSON, and general
  commands the engine lacked: find and replace, plain/reset/capitals formatting, page break,
  delete paragraph, paragraph selection, spelling with a per-document word list.
- `jcj-output`: deterministic PDF, page images, printing (`print` feature).
- `jcj-proof`: spelling, suggestions, user words, hyphenation.
- `jcj-interchange`: DOCX, ODT, RTF, Markdown, HTML, text import and export; an EPUB 3 writer;
  emphasis markers.
- `jcj-project`: the project container (one document or many, assets, snapshots, the app's data
  and migrations), atomic saves, backups, recovery files, auto-save timing.
- `jcj-features`: bookmarks, notes, revision sets, change tracking, page locks, watermarks,
  headers and footers, comparison, with their commands.
- `jcj-theme`: design tokens (`tokens.json`) to egui themes; default interface fonts from
  WordCraft's bundled OFL faces.
- `jcj-board`: cards, corkboard, canvas.
- `jcj-shell`: ribbon, panels, File page, palette, dialogs, preferences, keys, auto-save, updates,
  icons, offscreen pictures.
- `jcj-agents`: an app's commands as MCP tools.
- `core-testbed`, `contract-tests`, `template/` (a new app's starting point and the files every
  app keeps in step with `cargo xtask sync`).

### Tooling

- `cargo xtask ci` (fmt, lint-roots, layers, pins, template, privacy, licenses, clippy, tests),
  `ci --changed` for affected packages only, git hooks, `template-check`, `vendor`,
  `third-party-licenses`; cargo-deny; CI on GitHub-hosted runners (decision 0003).

### Known gaps

No EPUB import, no PDF/X; the page planner does not plan footnotes, floats or columns and keeps
tables whole; spelling and hyphenation tests run without the optional craft-i18n language data.

### What apps must do

Start from `template/` and follow `docs/history/porting-script.md` (for Wordsmith Script).
