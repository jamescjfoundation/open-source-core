# Contributing

Thank you for helping. This repository is public: everything you commit, push, or write in an
issue, pull request or review can be read by anyone and stays in the history for good.

## Building and checking

```sh
cargo xtask install-hooks        # once: pre-commit and pre-push hooks
cargo xtask ci                   # the full gate (CI runs the same)
cargo xtask ci --changed         # only the packages your change affects, and their dependents
cargo xtask affected             # which packages those are, and why
cargo xtask docs                 # links, decision references (part of the gate; quick after a docs change)
cargo xtask docs <folder>        # the same on another checkout: an app, or a repository without Cargo
cargo run -p core-testbed        # the test app that exercises every crate
cargo xtask deny                 # licenses, advisories and sources (needs cargo-deny)
cargo xtask template --write     # after changing a block or the code of conduct under template/
cargo xtask template-check       # builds the app template against this checkout
cargo xtask template-check --package   # its release package on this system, as an app's release makes it
cargo xtask release <version>    # the changes of a release commit ("Releasing" below)
```

`cargo xtask help` lists every task. Install
[cargo-nextest](https://nexte.st/docs/installation/pre-built-binaries/) for faster test runs: the
`test` task uses it when it is installed, here and in every app, and CI always does. It runs the
same tests and says which of the two ran them.

## Where things are

| Path | What it is |
| --- | --- |
| `crates/jcj-upstream` | The one crate that names WordCraft and craft-libs |
| `crates/jcj-doc` | Document model interface, typed extension data, the word list and the word-level comparison |
| `crates/jcj-entries`, `jcj-i18n` | Entries kept beside documents; interface translation catalogs |
| `crates/jcj-layout`, `jcj-engine`, `jcj-output`, `jcj-proof`, `jcj-agents` | Adapters: pages, editing sessions, PDF and printing, spelling, agent tools (with the adapter for an app's own commands and the shared parts of a command line) |
| `crates/jcj-project`, `jcj-interchange` | Projects on disk (the folder of plain files and the single-file container), saving, recovery and backups; format import and export |
| `crates/jcj-features` | Document features every app shares: bookmarks, notes, revisions, page locks |
| `crates/jcj-theme`, `jcj-board`, `jcj-shell` | Theming from tokens and brand loading, cards and boards, the app shell and an app's start-up |
| `apps/core-testbed` | A small app that uses every crate |
| `contract-tests` | Behavior apps rely on, run against every upstream candidate |
| `template` | Starter files for a new app repository, and the source of what `cargo xtask sync` keeps identical in every app: the shared xtask modules (the gate every app runs is `template/xtask/src/gate.rs`; an app's release package and release commit are `package` and `release` in `shared.rs`), the four files that call the shared workflows, the Dependabot settings, hooks, code of conduct and the marked blocks of rules and settings |
| `xtask` | The core's own gate and maintenance tasks |
| `.github/workflows` | The core's own workflows, and the four shared workflows every app calls (its CI, its core pin, its gate on macOS and Windows, its release); its `README.md` says what each runs and when, how a change to a shared one reaches the apps, what CI keeps between runs, and how a release is gated |
| `.github/apps.json` | The app repositories that are built against a core commit every night and before a release |
| `docs/decisions` | Why things are the way they are |
| `docs/command-lines-and-agent-tools.md` | The convention every app's command line and agent tools follow: exit codes, `mcp`, tool names |
| `docs/app-releases.md` | The contract of an app's public release: tag, file names, the checksum file, what each package holds, signing, how a release is cut and tried; `docs/app-release-example/` is the example the tests hold it to |
| `docs/UPSTREAM.md` | Pins, carried patches, follow / hold / own |
| `docs/history/` | Finished plans and published reviews, kept as written and not maintained |
| `CHANGELOG.md` | What each release changes for apps, with "What apps must do" |
| `NOTICE`, `ATTRIBUTION.md` | The credit to upstream; the source and license of every asset and copied block of code we did not write |

<!-- BEGIN open-source-core: contributing -->
<!-- Owned by open-source-core (template/CONTRIBUTING.md). Change it there, never in a copy: its tooling rewrites the copies. -->
## Rules for a public repository

### Personal information

Never commit, or paste into an issue or pull request: anyone's name, email address or account
(other than your commit author line); local paths, user names, host names or machine details;
files from a real machine (preferences, recovery, auto-save or backup files, logs, desktop
screenshots); anyone's private writing; passwords, tokens or keys. Test fixtures are
public-domain texts or written for the test. The repository's privacy check
(`cargo xtask privacy` where it has `cargo xtask`) checks files and commit identities.

### Security problems

Follow `SECURITY.md`: report privately and keep the problem out of public places until a release
fixes it.

### Private working notes

Reviews, audits and working notes go in `.private/` (ignored by Git). One is published only when
a maintainer says so, under `docs/history/` where the repository has a `docs/` folder, with a
first line saying it is history
([core decision 0012](https://github.com/jamescjfoundation/open-source-core/blob/main/docs/decisions/0012-working-in-the-open.md)).
Write the public part as a decision record or changelog entry, giving the reason itself rather
than "as discussed".

## Commits, pushes and pull requests

- Every change lands on `main` through a pull request whose required checks pass; GitHub
  refuses a direct push. Its author, a person or an agent, merges it once they pass, and no
  approval is required. Run the repository's gate (`cargo xtask ci` where it has
  `cargo xtask`) before you push the branch
  ([core decision 0013](https://github.com/jamescjfoundation/open-source-core/blob/main/docs/decisions/0013-app-and-core.md)).
- A release is cut, and in the core an upstream pin is moved, only on a commit whose full gate
  has passed; a person or an agent may do either (core decisions
  [0004](https://github.com/jamescjfoundation/open-source-core/blob/main/docs/decisions/0004-follow-hold-own.md)
  and
  [0011](https://github.com/jamescjfoundation/open-source-core/blob/main/docs/decisions/0011-releases-and-versioning.md)).
- Contributions are licensed as the repository's license files say (MIT OR Apache-2.0 for
  code), with no extra terms. No CLA.
- Use your GitHub no-reply address (`git config user.email <id>+<login>@users.noreply.github.com`).
- One concern per commit or pull request; say what changed, why, and how you checked it.
- A pull request opened by Dependabot is merged by anyone with write access once its checks
  pass.
- Update the changelog and the decision records, where the repository keeps them, when behavior
  changes. A core change that apps must act on says so under "What apps must do".
- AI-assisted contributions are welcome; you are responsible for every line.
<!-- END open-source-core: contributing -->

## Rules of this repository

### Upstream code and names

- WordCraft and craft-libs are dependencies through our forks. Changes to their code go on a topic
  branch of the fork and are recorded in `docs/UPSTREAM.md`.
- Refer to upstream issues as `storytold/wordcraft#123`, never `#123`.
- No ArtCraft or WordCraft name, logo or icon in anything users see; the credit lives in `NOTICE`.
- No product's name, color, logo, or screenplay, manuscript or book concept in the core's code,
  identifiers, interface text or anything under `template/`. The records of history
  (`CHANGELOG.md`, the decision records, `docs/history/`) may say which app a thing came from,
  and `README.md` and the opening of `AGENTS.md` may link the map of a project built on the core
  (decision 0013).

### Dependencies and assets

Every dependency's license must be on the allowlist (decision 0001). Fonts, images and data must
be under an allowed license and listed in `ATTRIBUTION.md`.

### The marked blocks

The rules and settings every app carries are the marked blocks under `template/` (`BLOCKS` in
`template/xtask/src/sync.rs`): rules in the Markdown files, and in `Cargo.toml`,
`.cargo/config.toml`, `.gitignore` and `.gitattributes` the settings every app shares. Change a
block there, never in this repository's own copy, then run `cargo xtask template --write`; the
`template` task of the gate fails when a copy here differs. A changed block reaches the apps with
the next release, so keep it true for every app and for the core, and say in the changelog what
an app must do.

### The shared gate

An app's gate is this repository's code: `template/xtask/src/gate.rs` and the modules beside it,
listed in `template/xtask/src/shared.rs`. An app's `xtask/src/main.rs` is the app's own and holds
only its data and its own tasks. Keep a release from asking for an edit there: a new check goes
in `CHECKS`, a new module in `shared.rs` and in `MANAGED`, and a new field of `App` or of
`Package` gets a value in its `DEFAULT`. Until issue #42 is fixed, a release that adds a module
cannot be taken by an app's own `cargo xtask sync`: put new code in a module apps already have,
or give the step that works under "What apps must do". The `release` and `package` modules are
written inside `shared.rs` for that reason, and move to files of their own once it is fixed.
`cargo xtask template-check` builds the template against this checkout and runs its checks.

### The shared workflows

`shared-ci.yml`, `shared-core-pin.yml`, `shared-platforms.yml` and `shared-release.yml` in
`.github/workflows/` are the jobs of every app's CI, of its weekly core-pin check, of its
nightly gate on macOS and Windows and of its release. Apps call them at the
tag `workflows-v1`, so a change to one reaches the apps when that tag is moved: not when the
change lands, and not with a release. A person or an agent moves it, to a commit of `main` where
`ci.yml` has passed. The steps, and the list of what these workflows may ask of an app, are in
`.github/workflows/README.md`; a change that asks more gets a new tag (decision 0003).

### App releases

What an app's public release is made of is `docs/app-releases.md` (decision 0019), and the code
that keeps it is here: `cargo xtask package` and `cargo xtask release` in the shared xtask,
`shared-release.yml`, and the managed file `template/.github/workflows/release.yml`. An app
gives data and nothing else. So a change to a file name, to the checksum file or to where a
program sits in a package is a change to the contract: copies already installed read all three,
and so do a launcher and a website. Change the document, the code and
`docs/app-release-example/` together; the tests in `crates/jcj-shell/src/updates/desktop.rs`
hold the example to the updater's own reader, and the core's xtask holds the packaging code to
the example. `cargo xtask template-check --package` makes the template app's package on this
system; the nightly run does that on Linux, macOS and Windows.

## Releasing

A person or an agent cuts a release, on a commit whose full gate has passed (decision 0011). The
gate runs first and the tag is made by a workflow, so a tag cannot land on a commit that failed.

1. Write what the release changes for apps under `## Unreleased` at the top of `CHANGELOG.md`,
   and under **What apps must do** the exact steps an app follows, starting with the pin move.
   Someone holding only that text must be able to follow it. When the pin move and
   `cargo xtask sync` are all an app does, say exactly that.
2. Run `cargo xtask release X.Y.Z`. It changes the four files of a release commit and nothing
   else:
   - `Cargo.toml`: `version` under `[workspace.package]`;
   - `Cargo.lock`: the core's own crates at the new version (it runs
     `cargo metadata --format-version 1`, which moves nothing else);
   - `CHANGELOG.md`: `## Unreleased` becomes `## X.Y.Z (date)`, with today's date in UTC
     (`--date YYYY-MM-DD` for another);
   - `template/Cargo.toml`: the tag on every `jcj-` line, `vX.Y.Z`.

   It refuses a version that is not newer than the present one, a changelog whose newest
   section is not `## Unreleased`, and a section with no **What apps must do** or nothing
   under it. A refusal changes no file.
3. Run `cargo xtask ci`, commit the four files as "open-source-core X.Y.Z" on a branch, open a
   pull request, and merge it once its checks pass.
4. Start the release for that commit:

   ```sh
   gh workflow run release.yml --ref main
   gh run watch "$(gh run list --workflow release.yml --limit 1 --json databaseId --jq '.[0].databaseId')"
   ```

   It checks that the commit is a release commit and not released yet. It runs the full gate
   on Linux, macOS and Windows, the minimum Rust version, cargo-deny, the app template, and
   each app in `.github/apps.json` built against the commit with its own gate. Only when all
   of that has passed does it make the annotated tag `vX.Y.Z` and publish the GitHub release,
   with the changelog section, the vendored sources, the third-party license texts and
   checksums. When a run of the gate (`nightly.yml`) has already passed on that very commit, it
   uses that run and does not run the gate again. `-f commit=<commit>` releases an older commit
   of `main` than its newest.
5. A red job stops the release, and no tag exists. Land the fix on `main`, with a line in the
   release's changelog section when apps should know, and start the workflow again: the
   version was never tagged, so the commit with the fix is the release commit now. When only a
   runner or the network was at fault, run the failed jobs again:
   `gh run rerun <run> --failed`.
6. **When an app is red** and the core is right, the release goes ahead only with the reason
   written in the release's changelog section, one line for each such app:

   ```text
   **App gate override, <owner>/<repository>:** <the reason, a sentence or more>
   ```

   The line is part of the release commit, so it is in the release's text, where the app's
   maintainers read what they must do. `.github/workflows/README.md`, "When an app is red",
   has the rule.
7. Do not make or push a `v*` tag by hand: no workflow runs for it, and it cannot be taken
   back. Apps name the tag and their lock files record its commit, so a tag is never moved or
   deleted. A mistake is fixed in the next version.

## Conduct

Follow `CODE_OF_CONDUCT.md`.
