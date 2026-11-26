# Contributing

Thank you for helping. This repository is public: everything you commit, push, or write in an
issue, pull request or review can be read by anyone and stays in the history for good.

## Building and checking

```sh
cargo xtask install-hooks        # once: pre-commit and pre-push hooks
cargo xtask ci                   # the full gate (CI runs the same)
cargo xtask ci --changed         # only the packages your change affects, and their dependents
cargo run -p core-testbed        # the test app that exercises every crate
cargo xtask deny                 # licenses, advisories and sources (needs cargo-deny)
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
| `template` | Starter files for a new app repository, and the files `cargo xtask sync` keeps identical in every app |
| `xtask` | The gate and maintenance tasks |
| `docs/decisions` | Why things are the way they are |
| `docs/UPSTREAM.md` | Pins, carried patches, follow / hold / own |
| `docs/history/` | Finished plans and published reviews, kept as written and not maintained |

## Rules for a public repository

### Personal information

Never commit, or paste into an issue or pull request: anyone's name, email address or account
(other than your commit author line); local paths, user names, host names or machine details;
files from a real machine (preferences, recovery, auto-save or backup files, logs, desktop
screenshots); anyone's private writing; passwords, tokens or keys. `cargo xtask privacy` checks
files and commit identities.

### Security problems

Follow `SECURITY.md`: report privately and keep the problem out of public places until a release
fixes it.

### Private working notes

Reviews, audits and working notes go in `.private/` (ignored by Git). One is published only when
a maintainer says so, under `docs/history/`, with a first line saying it is history (decision
0012). Write the public part as a decision record or changelog entry, giving the reason itself
rather than "as discussed".

### Upstream code and names

- WordCraft and craft-libs are dependencies through our forks. Changes to their code go on a topic
  branch of the fork and are recorded in `docs/UPSTREAM.md`.
- Refer to upstream issues as `storytold/wordcraft#123`, never `#123`.
- No ArtCraft or WordCraft name, logo or icon in anything users see; the credit lives in `NOTICE`.
- No product name, color, logo, or screenplay, manuscript or book concept in the core.

### Dependencies and assets

Every dependency's license must be on the allowlist (decision 0001). Fonts, images and data must
be under an allowed license and listed in `ATTRIBUTION.md`.

## Commits, pushes and pull requests

- While no app built on the core has a public release, a push to `main` is allowed once
  `cargo xtask ci` passes. A pull request is optional; its author may merge it once its checks
  pass (decision 0013).
- A release is cut, and an upstream pin is moved, only on a commit whose full gate has passed; a
  person or an agent may do either (decisions 0004 and 0011).
- Contributions are licensed as the project is, MIT OR Apache-2.0, with no extra terms. No CLA.
- Use your GitHub no-reply address (`git config user.email <id>+<login>@users.noreply.github.com`).
- One concern per commit or pull request; say what changed, why, and how you checked it.
- Update `CHANGELOG.md` (with "What apps must do" for anything apps must change) and the decision
  records when behavior changes.
- AI-assisted contributions are welcome; you are responsible for every line.

## Conduct

Follow `CODE_OF_CONDUCT.md`.
