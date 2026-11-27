# Notes for coding agents (and people)

This app is built on [open-source-core](https://github.com/jamescjfoundation/open-source-core),
The James C. J. Foundation's shared base. The core holds what every app needs; this repository
holds only what is about this app.

The blocks below marked `open-source-core` are the core's rules, which every app carries word for
word. `cargo xtask sync` copies them from the pinned core release and `cargo xtask sync --check`
fails when a copy differs, so change them in the core (`template/AGENTS.md`), never here.
Everything outside the marked blocks is this app's own. A project with several repositories may
put its own block, with its map of them, above the core's; the core does not manage that one.

<!-- BEGIN open-source-core: app -->
<!-- Owned by open-source-core (template/AGENTS.md). Change it there, never in a copy: its tooling rewrites the copies. -->
## Before you commit

```sh
cargo xtask install-hooks   # once: pre-commit (fmt, privacy, no dev-link in a tracked file) and pre-push (ci --changed)
cargo xtask ci --changed    # what CI will run for your change; `cargo xtask ci` for everything
cargo xtask sync            # after moving to a new core release: shared files, blocks and patch block
```

What the core owns is changed in the core, never here:

- whole files: `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`, `deny.toml` (but for the
  app's own sources in it, which `sync` writes from `xtask/src/main.rs`), `CODE_OF_CONDUCT.md`,
  `xtask/Cargo.toml`, every file in `xtask/src/` but `main.rs` and the modules the app adds,
  `.github/workflows/ci.yml` and `core-pin.yml`, `.github/pull_request_template.md`,
  `.github/dependabot.yml` and `hooks/`;
- the blocks marked `open-source-core` in files that are otherwise the app's: in `AGENTS.md`,
  `SECURITY.md` and `CONTRIBUTING.md`, and in `Cargo.toml` (lints and build profiles),
  `.cargo/config.toml`, `.gitignore` and `.gitattributes`;
- the patch block in `Cargo.toml`.

`cargo xtask sync --check` fails if any of them differs from the pinned core release.

`xtask/src/main.rs` is the app's own and holds only what is the app's: its data (the product's
name, the programs it ships, the repositories it builds on besides the core) and its own tasks.
The gate, its list of checks and how each is run are the core's (`xtask/src/gate.rs`), so a
check the core adds runs here after `cargo xtask sync`, with no edit. The app's layers are the
table marked `layer table` in this file: `cargo xtask app-layers` reads that table, so the table
is the rule, and a layer is changed there in the same change as the code. The two workflow
files only say when CI runs: the jobs are the core's shared workflows, which they call, so a fix
to CI is made in the core and reaches the app without a change here.

## House rules for code

- Every crate root: an SPDX line, `#![forbid(unsafe_code)]`, and `#![deny(...)]` of the six
  panicking lints. Product code returns errors; it never unwraps or panics.
- Depend only on the core's `jcj-` crates and, under a contract, another app's crates
  ([core decision 0015](https://github.com/jamescjfoundation/open-source-core/blob/main/docs/decisions/0015-apps-that-depend-on-apps.md));
  never on `jcj-upstream`, `wordcraft-*` or `craft-*` (`cargo xtask layers`).
- The app's brand (its name, logo, icons, fonts and colors) lives in the app's repository, or in
  its project's brand kit with copies in the app, never in the core. The app's interface theme
  is its own `tokens.json`, which the core's theming code reads.
<!-- END open-source-core: app -->

<!-- BEGIN open-source-core: public -->
<!-- Owned by open-source-core (template/AGENTS.md). Change it there, never in a copy: its tooling rewrites the copies. -->
## What is public

This repository is public, history included, forever
([core decision 0012](https://github.com/jamescjfoundation/open-source-core/blob/main/docs/decisions/0012-working-in-the-open.md)).
Before you write a commit, branch name, issue, pull request or comment, check it holds none of:

- anyone's name, email, account names, or details of their machine (local paths such as a home
  folder, user names, host names, IP addresses);
- what the user told you in conversation, quoted or summarised; write the reason for a change as
  its own argument;
- an open security problem (follow `SECURITY.md`: keep it in `.private/` and tell the user);
- secrets, or files from a real machine (preferences, recovery, auto-save, backups, logs,
  screenshots of a desktop);
- anyone's private writing, even as test data. Fixtures are public-domain or written for the test.

Commit with a GitHub no-reply address or a foundation bot identity. Where a repository has
`cargo xtask`, `cargo xtask privacy` checks its files and every commit. Never push with `--all`
or `--mirror`.

The Foundation's name is always written "The James C. J. Foundation", with its capital "The"
wherever it stands in a sentence, never in a shorter form.
<!-- END open-source-core: public -->

<!-- BEGIN open-source-core: app-and-core -->
<!-- Owned by open-source-core (template/AGENTS.md). Change it there, never in a copy: its tooling rewrites the copies. -->
## Building between an app and the core

"The core" is [open-source-core](https://github.com/jamescjfoundation/open-source-core); "an app"
is any repository built on it. A record of the core is cited from anywhere as "core decision
0011", with its repository named, never as a bare number: an app has records with the same
numbers.

### Step 1: decide where the change belongs

Ask in order; the first "yes" decides.

1. **Does it change how we use WordCraft, craft-libs or another upstream project?** The core's
   adapters (`jcj-upstream` and the `jcj-` crate that wraps it). If upstream code itself must
   change, it goes in our fork first (the core's `docs/UPSTREAM.md` and
   [core decision 0004](https://github.com/jamescjfoundation/open-source-core/blob/main/docs/decisions/0004-follow-hold-own.md)).
2. **Would it look or work the same in any foundation app?** The window, ribbon, panels, dialogs,
   File page, preferences, auto-save, updates, saving and backups, spelling, find and replace,
   undo, format import and export, PDF and printing, cards and boards, theming, entries kept
   beside documents, interface translations, agent control: the core.
3. **Does a second app already need it, or have a written plan to use it?** The core.
4. **Otherwise** the app. Screenplay elements, Fountain, scene numbers, (MORE)/(CONT'D), a
   manuscript binder's compile rules and a book's themes are app code.

When unsure, put it in the app: moving code into the core later is cheap; taking a wrong general
interface back out is not.

| Example | Where | Why |
| --- | --- | --- |
| Redesigned ribbon, new panel docking | Core (`jcj-shell`) | Every app has a ribbon and panels |
| A new app-specific ribbon tab | The app | Only the tab's contents are the app's |
| Faster auto-save, better backups | Core (`jcj-project`, `jcj-shell`) | Same for every app |
| Index cards a second app also wants | Core (`jcj-board`) once it needs them | Second app |
| DOCX import fix | Core (`jcj-interchange`), offered upstream too | General |
| Colors, fonts, logo | The app's repository or its project's brand kit | The core holds theming code, never a brand |

### Step 2: build it

- **How a change lands**
  ([core decision 0013](https://github.com/jamescjfoundation/open-source-core/blob/main/docs/decisions/0013-app-and-core.md)):
  while no app has a public release, a push to `main` is allowed once the gate passes; a pull
  request is optional and its author merges it when its checks pass.
- **App-only change:** one change in the app.
- **Core change an app needs:**
  1. Open a core issue describing the general need, not the app's feature.
  2. Check out both side by side. In the app: `cargo xtask dev-link ../open-source-core`
     (`cargo xtask dev-unlink` removes it). The link changes no file Git tracks: it is a folder
     Git ignores, `.cargo/dev-link/`, and `Cargo.lock` is left alone. While linked, builds,
     tests and the editor use the local core, and `cargo xtask ci --linked` runs the gate
     against it. The plain gate refuses to run linked, because CI builds the pinned release.
  3. Build the general part in the core with its own tests and a use in `apps/core-testbed`, so it
     is proven without any product.
  4. Build the app's use of it in the app.
  5. To have CI check the app's half before the core is released, give the app's branch a
     temporary pin. Push the core change (a branch will do). On the app's branch, on every
     `jcj-` line, write `rev = "<that core commit, all 40 characters>"` in place of the tag; then
     `cargo metadata --format-version 1 >/dev/null` and `cargo xtask sync`. The gate passes on
     that branch and in its pull request, with a warning. It fails on `main`, on a tag and in a
     release, so the pull request is not merged while it holds the pin.
  6. Land the core change first, with the core's gate green. Then release the core.
  7. Move the app's pin to that release's tag (`cargo xtask sync`), then land the app change,
     which names the core issue it needed.
- **Prototype first, promote later.** If the general shape is not clear yet, build it in the app,
  mark it `// core-candidate: open-source-core#N` and open that core issue, labelled
  `core-candidate`. Move it once it works and a second app wants it. A promotion is finished only
  when the app the code came from uses the core's copy and its own copy is deleted: the core
  issue stays open until then, and the marker on the app's copy names it. Never copy code
  between app repositories.
- **Something one app needs from another app**
  ([core decision 0015](https://github.com/jamescjfoundation/open-source-core/blob/main/docs/decisions/0015-apps-that-depend-on-apps.md)):
  open an issue in the providing app's repository. The provider's change lands and is tagged
  there first; the consumer then moves its pin. Never edit the provider as part of the consumer's
  change. To try the two together, the consumer names the provider in its `xtask/src/main.rs`
  (`providers`) and links a checkout of it like the core: `cargo xtask dev-link <name>=<folder>`.

### Step 3: let it reach the other apps safely

- Everything in the core is shared by default.
- A change that alters what users see in other apps goes behind an option (a shell setting or a
  Cargo feature) that defaults to the old behavior; each app turns it on when ready.
- Versioning
  ([core decision 0011](https://github.com/jamescjfoundation/open-source-core/blob/main/docs/decisions/0011-releases-and-versioning.md)):
  while at 0.x a patch release only adds or fixes; a minor release may break and its changelog
  says exactly what each app must do. A breaking change to a public `jcj-` interface says so in
  the title of its commit or pull request, and every app is updated in follow-up changes.
- A core change that breaks another app is fixed in the core, not worked around in the app.
- An app moves to the newest core before new work starts on it and before its own release. Every
  `jcj-` line names one tag. `cargo xtask core-pin` says how far behind an app is, and a
  scheduled check opens an issue when it is more than three releases behind.

### Releasing an app

One contract and one workflow for every app
([core decision 0019](https://github.com/jamescjfoundation/open-source-core/blob/main/docs/decisions/0019-app-releases.md);
the contract, with what each release file is named and holds, is the core's
[`docs/app-releases.md`](https://github.com/jamescjfoundation/open-source-core/blob/main/docs/app-releases.md)).
The gate runs first and the workflow makes the tag, as for the core.

1. Move the app to the newest core. Write what the release changes under `## Unreleased` of
   `CHANGELOG.md`.
2. `cargo xtask release X.Y.Z` makes the changes of the release commit: the version in
   `Cargo.toml` and `Cargo.lock`, and the changelog heading with the date. Run `cargo xtask ci`,
   commit, push to `main`.
3. `gh workflow run release.yml --ref main`. It checks the release commit, runs the whole gate
   on Linux, macOS and Windows, builds and checks the package of each system and
   `SHA256SUMS.txt`, and only then makes the tag `vX.Y.Z` and publishes. A red job leaves no
   tag: fix the cause on `main` and start it again.
4. To try it without releasing: `gh workflow run release.yml -f dry-run=true`. The same dry run
   starts by itself every week. `cargo xtask package` makes this computer's package in `dist/`.

What a release carries is data in `xtask/src/main.rs` (`package`). The packaging and the
workflow are the core's: an app has no release workflow and no packaging script of its own.

### Things agents must not do

- Name WordCraft or craft-libs types in an app repository. Use the `jcj-` interfaces; if one is
  missing, add it to the core.
- Put a product name, color, logo, or screenplay, manuscript or book concept into the core's
  code, interface text or `template/`.
- Change a core interface from a change made in an app repository.
- Copy code from one app repository into another.
- Release the core, or move an upstream pin, on a commit whose full gate has not passed. A
  person or an agent may do either once it has (core decisions
  [0004](https://github.com/jamescjfoundation/open-source-core/blob/main/docs/decisions/0004-follow-hold-own.md)
  and
  [0011](https://github.com/jamescjfoundation/open-source-core/blob/main/docs/decisions/0011-releases-and-versioning.md)).
- Push a `v*` tag, or create or edit a release, by hand: in the core and in an app the release
  workflow makes both, after the gate. A tag cannot be taken back.
- Edit WordCraft or craft-libs code anywhere but our forks' topic branches.
- Edit a file or a marked block the core owns anywhere but in the core.
- Commit personal details, private files or secrets.

### Where each kind of record goes

| Record | Lives in |
| --- | --- |
| Decisions about shared interfaces, upstream, tooling | The core's `docs/decisions/` |
| Decisions about one app's behavior | That app's `docs/decisions/` |
| Pins, carried patches, follow / hold / own | The core's `docs/UPSTREAM.md` |
| What changed for apps in each core release | The core's `CHANGELOG.md`, with "What apps must do" |
| Ideas waiting to be promoted to the core | Core issues labelled `core-candidate` |
| A request to another app | An issue in that app's repository |
| Reviews, audits, working notes | `.private/`, or `docs/history/` once a maintainer publishes one |
<!-- END open-source-core: app-and-core -->

## This app's own rules

Write here what holds only in this app: its layers, its formats, its names. Nothing here may
contradict the blocks above; if a rule above is wrong for every app, change it in the core.

### Layers

`cargo xtask app-layers` reads this table, so it is the rule itself. A row is a layer, lowest
first. A package depends only on packages of earlier rows, and uses a core crate (or any other
crate the table names) only when its own row or an earlier one names it in backticks;
`(only here)` after a name keeps that crate to one row. Every package of the workspace but
`xtask` is in one row. Add a row, or a name to a row, in the same change as the code.

<!-- BEGIN layer table (cargo xtask app-layers reads it) -->
| Layer | Packages | Holds | May use |
| --- | --- | --- | --- |
| 1 | `app` | The program: its window, its commands and its files | `jcj-doc`, `jcj-layout`, `jcj-engine`, `jcj-output`, `jcj-project`, `jcj-features`, `jcj-shell` |
<!-- END layer table -->
