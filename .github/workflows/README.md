# CI

Decision 0003. GitHub-hosted runners only; built so nobody waits on CI.

## This repository's workflows

| Workflow | When | What |
| --- | --- | --- |
| `ci.yml` | Every push to `main` and every pull request, a change to Markdown alone included | Calls `shared-ci.yml` from the same commit. Three jobs in parallel: **checks** (every task of the gate but clippy and test: fmt, crate roots, layers, pins, template, docs, privacy of every file and commit identity, licenses; no product build), **deny** (cargo-deny), **build** (clippy and tests for the packages changed since the last green commit on `main`, and their dependents; it ends at once when the change affects no package). A newer push cancels an older run; nothing is skipped, because the base is the last *green* commit. |
| `nightly.yml` | Daily, on demand, and before a release | The full gate on Linux, macOS and Windows; the minimum Rust version; cargo-deny with today's advisories; the app template built against this checkout. |
| `nightly-failed.yml` | When a scheduled `nightly.yml` run has failed | Opens the issue "Nightly run failed", or adds a comment to the open one. A run started by hand or by a release is not reported: someone is watching it. |
| `upstream-train.yml` | Every other Monday | Opens an issue with the train checklist when WordCraft has a newer release than the pin (`cargo xtask upstream-status`). |
| `release.yml` | Pushing a `v*` tag, which a person or an agent does once the nightly run has passed on that commit (decision 0011) | The nightly run again, a check that the tag is `Cargo.toml`'s version and that `CHANGELOG.md` has its section, then a GitHub release with the changelog section, vendored sources, third-party licenses and checksums. |

## The shared workflows, which every app calls

The jobs of an app's CI are not copied into the app. They are two workflows here, and each app
carries two short files that call them (`template/.github/workflows/`, kept identical in every
app by `cargo xtask sync`). Those files say only when a workflow runs and which token its jobs
get.

| Shared workflow | Called by | What |
| --- | --- | --- |
| `shared-ci.yml` | `ci.yml` here (from the same commit) and `ci.yml` in every app (at `workflows-v1`) | The three jobs above. In an app the checks include `sync --check` (the shared files, the patch block and the marked blocks match the pinned core release), `core-pin`, `unlinked`, `app-layers`, `docs`, `assets`, and every task the app added to its own gate. The list is the app's pinned core release's (`gate.rs`), not this file's. On a pull request whose branch pins the core at a commit instead of a release, the jobs build against that commit and `core-pin` passes with a warning shown on the checks page; the same pin fails on `main` (decision 0013). |
| `shared-core-pin.yml` | `core-pin.yml` in every app (at `workflows-v1`), Mondays and on demand | `cargo xtask core-pin --max-behind 3`: when the app's core pin is more than three releases behind the newest, opens an issue in the app ("Move to open-source-core vX.Y.Z"), or brings the open one up to date (decision 0011). |

A called workflow runs in the repository that calls it: that repository's code, commit and
token. The caller grants the token (`contents: read` and `actions: read` for CI; `contents: read`
and `issues: write` for the core pin) and the shared workflow cannot ask for more.

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

A change that needs more than this (a new task, a new flag, a new file in the app) does not go
to `workflows-v1`. It gets a new tag, `workflows-v2`: the release that brings what the workflow
needs also changes the two files under `template/.github/workflows/` to name the new tag, and
the tag is created before that release is cut. `workflows-v1` then stays where it is, for apps
that have not moved yet. This is what keeps a change in the core from breaking an app's CI
unasked.

## Scheduled workflows can be switched off without notice

GitHub switches a scheduled workflow off when its repository has had no activity for 60 days,
and tells nobody. Here that is `nightly.yml` and `upstream-train.yml`; in an app it is
`core-pin.yml`, in a repository that may well be quiet for that long. To look and to switch one
back on:

```sh
gh workflow list --all --repo jamescjfoundation/<repository>    # shows it as disabled_inactivity
gh workflow enable core-pin.yml --repo jamescjfoundation/<repository>
```

## Before a release

`nightly.yml` is started by hand on the release commit (`gh workflow run nightly.yml --ref main`)
and the tag is pushed only when it is green. Nothing enforces that order yet; the steps are in
`CONTRIBUTING.md`, "Releasing".

## Why it rarely makes anyone wait

1. **The same gate runs locally first.** `cargo xtask install-hooks` makes `git push` run
   `cargo xtask ci --changed` against what the remote has. CI repeats a check that already passed.
   A clone or worktree without the hooks is told so each time `cargo xtask ci` runs there.
2. **Only what changed is built and tested on pushes**, with a warm cache saved from `main`.
   A changed file belongs to the package whose folder holds it and to every package whose
   sources name it: a design or a font under `assets/`, a fixture or a golden file under
   `tests/`, a page of `docs/` that a test compares. A file under the root `assets/` or `tests/`
   that no package names means everything, and so do manifests, the lock file and the toolchain
   (`template/xtask/src/changed.rs`; `cargo xtask affected` says what a change affects, and why).
3. **Cheap checks finish in about a minute**, in parallel with the build, so the common mistakes
   (format, layers, privacy, licenses) report first. They run for every change, Markdown
   included: the marked blocks and the links live in Markdown.
4. **The slow, wide checks run on a schedule**, not on every push.

Run any job's work locally: `cargo xtask ci`, `cargo xtask ci --changed --base <ref>`,
`cargo xtask deny`, `cargo xtask template-check`.
