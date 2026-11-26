# CI

Decision 0003. GitHub-hosted runners only; built so nobody waits on CI.

| Workflow | When | What |
| --- | --- | --- |
| `ci.yml` | Every push to `main` and pull request (not documentation-only changes) | Three jobs in parallel: **checks** (fmt, crate roots, layers, pins, template, privacy of every file and commit identity, licenses; no product build), **deny** (cargo-deny), **build** (clippy and tests for the packages changed since the last green commit on `main`, and their dependents). A newer push cancels an older run; nothing is skipped, because the base is the last *green* commit. |
| `nightly.yml` | Daily, on demand, and before a release | The full gate on Linux, macOS and Windows; the minimum Rust version; cargo-deny with today's advisories; the app template built against this checkout. |
| `upstream-train.yml` | Every other Monday | Opens an issue with the train checklist when WordCraft has a newer release than the pin (`cargo xtask upstream-status`). |
| `release.yml` | Pushing a `v*` tag (a person's decision) | The nightly run, then a GitHub release with the changelog section, vendored sources, third-party licenses and checksums. |

The core also owns the workflows every app runs. They live in `template/.github/workflows/` and
`cargo xtask sync` keeps them identical in each app; none of them runs here.

| Workflow | When | What |
| --- | --- | --- |
| `ci.yml` | Every push to `main` and pull request in an app (not documentation-only changes) | The app's gate, built the same way as the core's: cheap checks, cargo-deny, and clippy and tests for what changed. The cheap checks include `sync --check`: the shared files, the patch block and the marked blocks in `AGENTS.md`, `SECURITY.md` and `CONTRIBUTING.md` match the pinned core release. |
| `core-pin.yml` | Mondays, and on demand | `cargo xtask core-pin --max-behind 3`: when the app's core pin is more than three releases behind the newest, opens an issue in the app ("Move to open-source-core vX.Y.Z"), or brings the open one up to date (decision 0011). |

A push that changes only Markdown runs no workflow, here or in an app. The marked blocks live in
Markdown files, so for such a push the check is the pre-push hook (`cargo xtask ci --changed` runs
the `template` task here and the `sync` task in an app whatever changed), and then the next push
that is not documentation only.

Why it rarely makes anyone wait:

1. **The same gate runs locally first.** `cargo xtask install-hooks` makes `git push` run
   `cargo xtask ci --changed` against what the remote has. CI repeats a check that already passed.
2. **Only what changed is built and tested on pushes**, with a warm cache saved from `main`.
   Manifests, the lock file or the toolchain changing means everything.
3. **Cheap checks finish in about a minute**, in parallel with the build, so the common mistakes
   (format, layers, privacy, licenses) report first.
4. **The slow, wide checks run on a schedule**, not on every push.

Run any job's work locally: `cargo xtask ci`, `cargo xtask ci --changed --base <ref>`,
`cargo xtask deny`, `cargo xtask template-check`.
