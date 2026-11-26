# Changelog

Each release says what changed for apps. While at 0.x, a patch release never breaks an app; a
minor release may, and then **What apps must do** says exactly what (decision 0011).

## Unreleased

### Added

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
  - `EpubOptions::default().note_numbering(NoteNumbering::ThroughBook)`: notes are numbered once
    through the book instead of from 1 in each chapter file (`PerChapter`, the default);
  - `.note_backlink(..)`: the link under each note is `NoteBacklink::Text("…")` in the book's
    language, `NoteBacklink::Symbol { label }` (a return arrow with a name for screen readers,
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
    release tag (`tag = "vX.Y.Z"`), all of them the same; the lines that differ are listed.
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
