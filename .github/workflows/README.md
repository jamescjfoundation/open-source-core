# CI

Decision 0003. GitHub-hosted runners only; built so nobody waits on CI.

## This repository's workflows

| Workflow | When | What |
| --- | --- | --- |
| `ci.yml` | Every push to `main` and every pull request, a change to Markdown alone included | Calls `shared-ci.yml` from the same commit. Three jobs in parallel: **checks** (every task of the gate but clippy and test: fmt, crate roots, layers, pins, template, workflows, docs, privacy of every file and commit identity, licenses; no product build), **deny** (cargo-deny), **build** (clippy and tests for the packages changed since the last green commit on `main`, and their dependents; it ends at once when the change affects no package). A newer push cancels an older run; nothing is skipped, because the base is the last *green* commit. |
| `nightly.yml` | Daily, on demand, and as the gate of a release | The full gate on Linux, macOS and Windows; the minimum Rust version; cargo-deny with today's advisories; the app template built against this checkout; the template app's release package made on Linux, macOS and Windows (`cargo xtask template-check --package`; "An app's release" below); and each app in `.github/apps.json` built against this checkout with its own gate ("The apps, built against a core commit" below). |
| `nightly-failed.yml` | When a scheduled `nightly.yml` run has failed | Opens the issue "Nightly run failed", or adds a comment to the open one. A run started by hand or by a release is not reported: someone is watching it. |
| `upstream-train.yml` | Every other Monday | Opens an issue with the train checklist when WordCraft has a newer release than the pin (`cargo xtask upstream-status`). |
| `release.yml` | Started by a person or an agent for a release commit on `main` (decision 0011): `gh workflow run release.yml --ref main` | Checks that the commit is a release commit and not released yet; runs `nightly.yml` on it, unless a run of it has already passed on that commit; and only then makes the annotated tag and the GitHub release, with the changelog section, vendored sources, third-party licenses and checksums ("A release" below). |

## The shared workflows, which every app calls

The jobs of an app's CI and of its release are not copied into the app. They are three
workflows here, and each app carries three short files that call them
(`template/.github/workflows/`, kept identical in every app by `cargo xtask sync`). Those files
say only when a workflow runs, which token its jobs get and which secrets they are handed.

| Shared workflow | Called by | What |
| --- | --- | --- |
| `shared-ci.yml` | `ci.yml` here (from the same commit) and `ci.yml` in every app (at `workflows-v1`) | The three jobs above. In an app the checks include `sync --check` (the shared files, the patch block and the marked blocks match the pinned core release), `core-pin`, `unlinked`, `app-layers`, `docs`, `assets`, and every task the app added to its own gate. The list is the app's pinned core release's (`gate.rs`), not this file's. On a pull request whose branch pins the core at a commit instead of a release, the jobs build against that commit and `core-pin` passes with a warning shown on the checks page; the same pin fails on `main` (decision 0013). |
| `shared-core-pin.yml` | `core-pin.yml` in every app (at `workflows-v1`), Mondays and on demand | `cargo xtask core-pin --max-behind 3`: when the app's core pin is more than three releases behind the newest, opens an issue in the app ("Move to open-source-core vX.Y.Z"), or brings the open one up to date (decision 0011). |
| `shared-release.yml` | `release.yml` in every app (at `workflows-v1`): started by hand for a release or a dry run, and by itself on Tuesdays as a dry run | The app's release (decision 0018, `docs/app-releases.md`): the release commit checked, the whole gate on Linux, macOS and Windows with cargo-deny, the package of each system, `SHA256SUMS.txt`, an attestation, and only then the tag and the published release. A dry run builds and checks the files and stops ("An app's release" below). |

A called workflow runs in the repository that calls it: that repository's code, commit and
token. The caller grants the token (`contents: read` and `actions: read` for CI; `contents: read`
and `issues: write` for the core pin; for the release also `contents: write`, `id-token: write`
and `attestations: write`, of which each job takes only its part) and the shared workflow
cannot ask for more. A secret reaches a shared workflow only when the caller names it.

### The tag `workflows-v1`

An app calls the shared workflows at the tag `workflows-v1`. The tag is meant to move: it is the
one step that carries a CI fix to every app, with no release of the crates and no change in an
app. A change to a shared workflow on `main` reaches no app until the tag is moved, so a mistake
there is red in this repository first.

To move it, as a person or an agent:

1. The change is on `main`, and `ci.yml` has passed there on the commit to move to. That run
   went through the same `shared-ci.yml`.

   ```sh
   gh run list --workflow ci.yml --branch main --status success --limit 1 --json headSha --jq '.[0].headSha'
   ```

2. Note where the tag is now (`git ls-remote origin refs/tags/workflows-v1`), then move it:

   ```sh
   git tag -f workflows-v1 <commit>
   git push --force origin refs/tags/workflows-v1
   ```

3. Try it in one app: `gh workflow run ci.yml --repo jamescjfoundation/<app>`, and after a
   change to `shared-core-pin.yml` also `gh workflow run core-pin.yml --repo jamescjfoundation/<app>`
   (this repository never runs that one itself). If a run fails because of the move, move the
   tag back to the commit noted in step 2.

The rule that a pushed tag is never moved is for release tags (`v*`), which apps pin and lock
files record. `workflows-v1` is not a release and names no crate.

### What `workflows-v1` may ask of an app

Apps on different core releases call the same tag, so the shared workflows may use only what
every core release from 0.1.19 on gives an app:

- `cargo xtask ci --skip clippy,test`, and `cargo xtask ci [--changed --base <commit>] --only clippy,test`;
- `cargo xtask affected --base <commit>`, which prints `all`, package names or an empty line;
- `cargo xtask core-pin --max-behind <n>`, which prints `pinned=`, `newest=` and `behind=` lines;
- `cargo deny --locked check`, a `rust-toolchain.toml` and a `Cargo.lock` at the root.

It may also try something a newer core gives and do without it where it is missing, as long as
the run is right either way. There is one such thing: `cargo xtask prune-target <lock file>`
(from 0.1.21), which removes stale build output after a dependency moved. An app on an older
core keeps that output, a few hundred megabytes at most, until it moves.

The tests run under cargo-nextest where the app's gate uses it, which is from 0.1.21 on. The
workflow puts the program there for every app; an older gate does not look for it.

`shared-release.yml` asks for more, of the apps that can call it: the tasks `release`,
`release-data` and `package` of the shared xtask and the app's package data. The file that
calls it reaches an app with the same core release as those tasks, through `cargo xtask sync`,
so no app calls it without having them. A later change to it may use only
what that release gives.

A change that needs more than this (a new task, a new flag, a new file in the app) does not go
to `workflows-v1`. It gets a new tag, `workflows-v2`: the release that brings what the workflow
needs also changes the files under `template/.github/workflows/` to name the new tag, and
the tag is created before that release is cut. `workflows-v1` then stays where it is, for apps
that have not moved yet. This is what keeps a change in the core from breaking an app's CI
unasked.

## Scheduled workflows can be switched off without notice

GitHub switches a scheduled workflow off when its repository has had no activity for 60 days,
and tells nobody. Here that is `nightly.yml` and `upstream-train.yml`; in an app it is
`core-pin.yml` and the weekly dry run of `release.yml`, in a repository that may well be quiet
for that long. To look and to switch one back on:

```sh
gh workflow list --all --repo jamescjfoundation/<repository>    # shows it as disabled_inactivity
gh workflow enable core-pin.yml --repo jamescjfoundation/<repository>
```

## The apps, built against a core commit

"A patch release never breaks an app" (decision 0011) is checked, not hoped for. The job
`apps` of `nightly.yml` runs once for each repository in `.github/apps.json`, on Linux:

1. It checks out the core at the commit under test, and the app at the newest commit of its
   `main` that passed the app's own CI (the newest commit of `main` when none is found). An app
   whose `main` is red for a reason of its own therefore does not stop a core release.
2. In the app: `cargo xtask dev-link <the core checkout>`, the app's own task, which changes no
   file Git tracks; then `cargo xtask sync`, which takes the core checkout's shared files, as
   the app will when it moves to the release.
3. `cargo xtask ci --linked`: the app's whole gate against the core commit. Its checks, clippy,
   and every test, golden files included. A golden file that differs is a failure of that app.

What a green job shows: the app, as it is on its `main`, builds and passes its gate with this
core in place of the release it pins, after nothing more than the pin move and `sync`.

What it cannot show, and says in the run's summary:

- **An app that builds on another app's crates** (decision 0015) takes them at the tag it
  names. Those crates are built against this core too, because the link replaces the core for
  the whole build, whatever core that tag was cut on. The other app's `main`, from which its
  next tag will be cut, is not tried with this one: that is between the two apps, and the
  provider's own row tries the provider.
- macOS and Windows: an app's gate runs on Linux here, as in the app's own CI.
- An app on a core older than 0.1.20 has no link that leaves its files alone and no
  `ci --linked`. Its job fails and says so; the app moves first.
- The app after a step that "What apps must do" asks beyond the pin move and `sync`. A release
  that asks for one may leave an app red on purpose; see the next section.

To add or remove an app, edit `.github/apps.json`: the `apps` list holds each repository as
`"owner/name"`. It must be public; the job reads it with no token of its own.
`cargo xtask workflows` checks that the file can be read.

### When an app is red

A red app fails the nightly run and stops a release. Fix the core. When the core is right and
the app is the one that must change (a minor release that breaks on purpose, an output that
changes on purpose and a golden file that must follow), whoever cuts the release goes ahead by
writing the reason where every app's maintainer will read it: the newest section of
`CHANGELOG.md`, `## Unreleased` or the release's own, carries one line for each such app, on a
line of its own or as an item of a list:

```text
**App gate override, <owner>/<repository>:** <the reason, a sentence or more>
```

For example: `**App gate override, example/some-app:** the EPUB writer now marks notes as a
list; the app updates its golden file when it moves.` The job then passes with a warning that
shows the reason, and the line is part of the release's text. A line names one app; no line
covers them all. The reason says what the app must do; it is not an apology.
`cargo xtask release --override <owner>/<repository>` prints the reason the checkout gives for
an app, and fails when it gives none.

## A release

The steps a person or an agent follows are in `CONTRIBUTING.md`, "Releasing". What the
workflows hold:

- **The gate runs before the tag exists.** `release.yml` is started for the release commit. Its
  `check` job refuses a commit that is not on `main`, is not a release commit
  (`cargo xtask release --check`: the version in `Cargo.toml`, the tags in
  `template/Cargo.toml` and the newest section of `CHANGELOG.md` agree, and the section says
  what apps must do), or is released already. Its `gate` job is `nightly.yml` on that commit.
  Its `publish` job needs the gate, and is the only thing that makes the tag.
- **Linux, macOS and Windows pass before every release**, a patch release included, with the
  minimum Rust version, cargo-deny, the template and the apps.
- **The gate runs once for a commit.** When a run of `nightly.yml` has already passed on the
  very commit (the scheduled run, or one started by hand), `check` finds it and `gate` is
  skipped: the same workflow file ran on the same code. `run-gate-again` runs it anyway. When
  only `publish` failed, run the failed job again (`gh run rerun <run> --failed`): the gate's
  result is kept, and the step that tags finds a tag it already made.
- **The tag is annotated and made by the run's own token**, through the API, with
  `contents: write` in that one job. The repository's rule for `v*` tags lets anyone with
  write access create one and nobody move or delete one.
- **Nothing runs when a tag is pushed by hand.** Such a tag would get no release and no gate,
  and it cannot be taken back. The rule is not to push one; what would make it impossible is a
  repository rule that lets only this workflow create `v*` tags, which is a setting, not a
  file.

## An app's release

`docs/app-releases.md` is the contract and has the steps a person or an agent follows. What
`shared-release.yml` holds:

- **The gate runs before the tag exists**, as for the core. `check` refuses a commit that is
  not on `main`, is not a release commit (`cargo xtask release --check` in the app: the version
  in `Cargo.toml`, a dated changelog section for it, package data that is sound), is released
  already, or pins a core that is not the newest (`older-core: true` goes ahead with a
  warning). `gate` is `cargo xtask ci`, whole, on Linux, macOS and Windows, and `deny` is
  cargo-deny. `publish` needs all of it and is the only job that can write to the repository.
- **Every file is built and checked before the tag.** `package` runs the app's own
  `cargo xtask package` on each system; `assemble` puts the three files in one folder, refuses
  a missing or a foreign one, writes `SHA256SUMS.txt` and reads it back with `sha256sum`.
  `attest` signs a statement of where the files were built. A failure anywhere leaves no tag.
- **Signing waits for its secrets.** The macOS certificate goes into a keychain of the run's
  own; the Windows certificate into a file of the run's own; the notary key likewise. A step
  whose secret is missing does not run, and the run's summary says how each package was
  signed. The secrets are named in `docs/app-releases.md`. No secret is read in a dry run.
- **A dry run** (`dry-run: true`; always, when the run was started by the schedule) does
  `check` without the release-commit rules, `package` and `assemble`, and stops. Its files are
  kept with the run for a week. A scheduled dry run that fails opens the issue "The release
  dry run failed" in the app, or adds to the open one.
- **The Linux package is built on a named image** (`ubuntu-24.04`), not the newest, because it
  needs that image's C library or a newer one and that must not move by itself. Moving it is a
  change to the contract's "Inside the Linux package".
- **Nothing is kept between runs** for a release build. A release is rare, and its build
  output would push out the archives CI reads every day.
- **The core tries the packaging first.** `nightly.yml` makes the template app's package on
  the three systems with the same task, every night and before a core release, so a change to
  the packaging code is red here before an app's weekly dry run or its release finds it.

To try a change to `shared-release.yml` before `workflows-v1` moves: push it to a branch here,
and in a branch of one app change the `uses:` line of `release.yml` to that branch
(`...shared-release.yml@<branch>`), then
`gh workflow run release.yml --repo jamescjfoundation/<app> --ref <app branch> -f dry-run=true`.
The app's gate fails on that branch (`sync --check`), which is right: it is never merged.

## What CI keeps between runs

GitHub keeps 10 GB of cache for a repository and drops what was used longest ago. So every
archive here is one a later run will read, and none is saved twice.

| Archive | Holds | Saved |
| --- | --- | --- |
| `cargo-home-<lock file>` | The crates and git repositories Cargo downloaded | By a build on `main`, when the lock file has none yet |
| `deps-<system>-<toolchain and settings>-<dependencies>` | `target/`, with the build output of the dependencies only | By `ci.yml` (Linux) and `nightly.yml` (Linux, macOS, Windows) on `main`, when these dependencies have none yet |
| `msrv-<version>-...`, `app-<owner>/<name>-...` | The same, for the build on the minimum Rust version and for each app built against the core | By `nightly.yml` on `main`, likewise |

- **Dependencies only.** Cargo builds a package that comes from a folder again whenever its
  files are newer than its output, and a fresh checkout is always newer. So the workspace's own
  packages were rebuilt on every run whatever was kept; keeping them only made the archive
  several times larger and slower to move. Before an archive is saved, every package built from
  a folder is taken out of `target/` (`cargo clean --package`), with the tests' scratch folders.
- **The name says what the content depends on**, and nothing else: the toolchain, Cargo's
  settings, the locked packages that come from elsewhere, and the manifests without the
  workspace's own version. No commit is in it. A push that moves no dependency finds the
  archive and saves nothing; a release of the workspace itself, which moves only its version,
  keeps its archive too.
- **Saved from `main` only**, and never from a tag or a pull request, whose archives no other
  run could read. A release runs on `main` and restores what `main` saved.
- **Saved whole.** On `main`, a run that finds no archive for its dependencies builds
  everything, not only what changed, so that what it saves serves every later run.
- **After a dependency moves**, the run starts from the newest archive of the same toolchain,
  and first removes the output of every package that moved and of everything built on one
  (`cargo xtask prune-target`, from the copy of the lock file kept inside the archive). Most of
  the output is still good, and what is not would otherwise be saved again for good.
- The full gate on Linux and `ci.yml` build the same things and share one archive. The
  template job restores it and saves nothing.

To see what a repository keeps and what a run cost:

```sh
gh api repos/jamescjfoundation/<repository>/actions/cache/usage
gh api "repos/jamescjfoundation/<repository>/actions/caches?per_page=100" \
  --jq '.actions_caches[] | "\(.size_in_bytes / 1048576 | floor) MB  \(.ref)  \(.key)"'
gh run list --repo jamescjfoundation/<repository> --workflow ci.yml --limit 10
gh run view <run> --repo jamescjfoundation/<repository> --json jobs \
  --jq '.jobs[] | .name, (.steps[] | "  \(.name): \(.startedAt) to \(.completedAt)")'
```

## Tools the workflows download

cargo-nextest and cargo-deny are not compiled in CI. Each is downloaded as the file its project
publishes with a release, and the file's SHA-256 checksum is checked against the one written in
the workflow's `env`, so a file that changed after we chose it is refused. The organisation
allows only GitHub's own actions, each named by a full commit; `cargo xtask workflows` checks
that, and that `shared-ci.yml` and `nightly.yml` name the same version and checksums.

To move to a newer version, change the version and each checksum in both files:

```sh
gh release view cargo-nextest-<version> --repo nextest-rs/nextest --json assets \
  --jq '.assets[] | select(.name | endswith(".tar.gz")) | "\(.digest)  \(.name)"'
gh release view <version> --repo EmbarkStudios/cargo-deny --json assets \
  --jq '.assets[] | select(.name | endswith(".tar.gz")) | "\(.digest)  \(.name)"'
```

A change to `shared-ci.yml` reaches the apps when `workflows-v1` is moved.

## Why it rarely makes anyone wait

1. **The same gate runs locally first.** `cargo xtask install-hooks` makes `git push` run
   `cargo xtask ci --changed` against what the remote has. CI repeats a check that already passed.
   A clone or worktree without the hooks is told so each time `cargo xtask ci` runs there.
2. **Only what changed is built and tested on pushes**, on top of the dependencies' build output
   kept from `main`.
   A changed file belongs to the package whose folder holds it and to every package whose
   sources name it: a design or a font under `assets/`, a fixture or a golden file under
   `tests/`, a page of `docs/` that a test compares. A file under the root `assets/` or `tests/`
   that no package names means everything, and so do manifests, the lock file and the toolchain
   (`template/xtask/src/changed.rs`; `cargo xtask affected` says what a change affects, and why).
3. **Every test still runs, and the test programs run at once.** The `test` task uses
   cargo-nextest when it is installed, in CI and on a laptop alike: each test in a process of
   its own, as many at a time as the machine has cores. `cargo test` runs one test program after
   another, which is most of the wait in a repository with long end-to-end tests. The tests in
   documentation, which cargo-nextest does not run, follow with `cargo test --doc`. Without
   cargo-nextest the task runs `cargo test`, and says which of the two ran. A test that asserts
   a time needs the machine to itself: the repository says so in its own `.config/nextest.toml`
   (`threads-required = "num-test-threads"` for that test), and cargo-nextest runs nothing
   beside it.
4. **Cheap checks finish in about half a minute**, in parallel with the build, so the common
   mistakes (format, layers, privacy, licenses) report first. They run for every change,
   Markdown included: the marked blocks and the links live in Markdown.
5. **The slow, wide checks run on a schedule**, not on every push.

Run any job's work locally: `cargo xtask ci`, `cargo xtask ci --changed --base <ref>`,
`cargo xtask deny`, `cargo xtask template-check`, `cargo xtask template-check --package`. For the apps job, in a checkout of the app:
`cargo xtask dev-link <this checkout>`, `cargo xtask sync`, `cargo xtask ci --linked`. Afterwards
`cargo xtask dev-unlink`, and put back the shared files `sync` wrote from this checkout
(`git status` in the app lists them).
