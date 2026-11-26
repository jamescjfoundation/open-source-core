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
```

`cargo xtask help` lists every task. Install `cargo-nextest` for faster test runs; the gate uses it
when present.

## Where things are

| Path | What it is |
| --- | --- |
| `crates/jcj-upstream` | The one crate that names WordCraft and craft-libs |
| `crates/jcj-doc` | Document model interface and typed extension data |
| `crates/jcj-entries`, `jcj-i18n` | Entries kept beside documents; interface translation catalogs |
| `crates/jcj-layout`, `jcj-engine`, `jcj-output`, `jcj-proof`, `jcj-agents` | Adapters: pages, editing sessions, PDF and printing, spelling, agent tools |
| `crates/jcj-project`, `jcj-interchange` | Project files, saving and backups; format import and export |
| `crates/jcj-features` | Document features every app shares: bookmarks, notes, revisions, page locks |
| `crates/jcj-theme`, `jcj-board`, `jcj-shell` | Theming from tokens, cards and boards, the app shell |
| `apps/core-testbed` | A small app that uses every crate |
| `contract-tests` | Behavior apps rely on, run against every upstream candidate |
| `template` | Starter files for a new app repository, and the source of what `cargo xtask sync` keeps identical in every app: the shared xtask modules, the two files that call the shared workflows, the Dependabot settings, hooks, code of conduct and the marked blocks |
| `xtask` | The core's own gate and maintenance tasks |
| `.github/workflows` | The core's own workflows, and the two shared workflows every app's CI calls; its `README.md` says what each runs and when, and how a change to a shared one reaches the apps |
| `docs/decisions` | Why things are the way they are |
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

- While no app built on the core has a public release, a push to `main` is allowed once the
  repository's gate passes (`cargo xtask ci` where it has `cargo xtask`). A pull request is
  optional; its author may merge it once its checks pass
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

The rules every app carries are the marked blocks under `template/` (`BLOCKS` in
`template/xtask/src/sync.rs`). Change a block there, never in this repository's own copy, then run
`cargo xtask template --write`; the `template` task of the gate fails when a copy here differs. A
changed block reaches the apps with the next release, so keep it true for every app and for the
core, and say in the changelog what an app must do.

### The shared workflows

`shared-ci.yml` and `shared-core-pin.yml` in `.github/workflows/` are the jobs of every app's CI.
Apps call them at the tag `workflows-v1`, so a change to one reaches the apps when that tag is
moved: not when the change lands, and not with a release. A person or an agent moves it, to a
commit of `main` where `ci.yml` has passed. The steps, and the list of what these workflows may
ask of an app, are in `.github/workflows/README.md`; a change that asks more gets a new tag
(decision 0003).

## Releasing

A person or an agent cuts a release, on a commit whose full gate has passed (decision 0011).

1. Write the release's section in `CHANGELOG.md`: what changed for apps, and under **What apps
   must do** the exact steps an app follows, starting with the pin move. Someone holding only
   that text must be able to follow it.
2. Make the release commit, "open-source-core X.Y.Z". It changes four files:
   - `Cargo.toml`: `version` under `[workspace.package]`;
   - `Cargo.lock`: run `cargo metadata --format-version 1 >/dev/null`, which brings the core's
     own crates to the new version and moves nothing else;
   - `CHANGELOG.md`: the section's heading, `## X.Y.Z (date)`;
   - `template/Cargo.toml`: the tag on every `jcj-` line, `vX.Y.Z`.
3. Run `cargo xtask ci` and push to `main`.
4. Run the cross-platform gate on that commit and wait for it:
   `gh workflow run nightly.yml --ref main`. It is the full gate on Linux, macOS and Windows,
   the minimum Rust version, cargo-deny and the app template.
5. When it is green, tag that commit and push the one tag:
   `git tag -a vX.Y.Z <commit> -m "open-source-core X.Y.Z"`, then `git push origin vX.Y.Z`.
   The release workflow runs the gate again, checks that the tag is `Cargo.toml`'s version and
   that `CHANGELOG.md` has its section, and publishes the GitHub release with the vendored
   sources, the third-party license texts and checksums.
6. Apps name the tag and their lock files record its commit, so a pushed tag is not moved or
   deleted. A mistake is fixed in the next version.

The order of steps 4 and 5 is kept by hand today: nothing refuses a tag on a commit whose
cross-platform run has not passed. The release workflow would fail on it, but the tag would
already exist.

## Conduct

Follow `CODE_OF_CONDUCT.md`.
