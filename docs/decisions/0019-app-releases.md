# 0019: One release contract and one release pipeline for every app

Date: 2026-11-26 · Status: accepted · Extends decisions 0003 (shared workflows) and 0011 (the
gate before the tag)

## Context

Three apps built on the core had each written a release: a workflow, a way to package, names
for the files. None had run. Read side by side on 2026-11-26:

| | Wordsmith Script | Wordsmith Manuscript | Wordsmith Book |
| --- | --- | --- | --- |
| Started by | A push to a `release` branch that did not exist | A pushed `v*` tag | A pushed `v*` tag |
| Published | As a draft, by hand | At once | At once, through an environment said to wait for an approval and set up with none |
| The gate | On four systems, in the release | On three | On Linux |
| macOS file | `…-macos-aarch64.dmg` | `…-macos-aarch64.zip` | `…-macos-arm64.dmg` |
| Windows file | `…-windows-x64-portable.zip` | `…-windows-x86_64.zip` | `…-windows-x64-portable.zip` |
| Checksum file | `SHA256SUMS.txt` | `SHA256SUMS.txt`, names written `./name` | none |
| Third-party license texts | In every package | In every package | In none |
| Command-line program | `<id>-cli` | `manuscript-cli` | `book-cli` |
| Packaging | An xtask module | Shell in the workflow | A shell script |

The update check every app carries (`jcj-shell`, over craft-update) reads a release by its file
names and its checksum file. At the core's pin it takes `-portable.zip`, `.dmg` and `.tar.gz`
with the system written `windows-x64`, `macos-aarch64` and `linux-x86_64`, and checksum lines
with a bare name. So an installed copy of the second app would have found no Windows or macOS
file and no checksum, and the third had no checksum to check. Two of the apps named in
`shipped` a program where the license task looks for a package, so their third-party license
file could not be written at all. A launcher that installs the apps from their releases, and a
website that links the downloads, would each have needed a rule for every app.

## Decision

`docs/app-releases.md` is the contract. What it decides, and why:

- **The files are named `<id>-<version>-<system>-<processor><kind>`**, one for Linux x86_64
  (`.tar.gz`), macOS on Apple silicon (`.dmg`) and Windows x64 (`-portable.zip`), in the
  spelling craft-update reads. The updater is not changed to read more: a name it already
  takes costs nothing, and every reader after it (a launcher, a site) then has one pattern.
  A test in `jcj-shell` holds the names and the checksum file to craft-update's own reader,
  so a move of the pin that changes what it reads is red in the core.
- **One checksum file, `SHA256SUMS.txt`, in the form `sha256sum` writes**, with bare names.
  It is the one form every tool and the updater read.
- **Every package carries the app's license files and `THIRD-PARTY-LICENSES.txt`**, at one
  place for each system, and is not made without them.
- **The desktop program is named by the release id, every other program `<id>-<name>`.** The
  updater looks for `bin/<id>` and `<id>.exe`, and it counts a program beside the installed
  one as the app's own by that prefix; two apps installed into one `bin` folder then share
  no name.
- **A version is `X.Y.Z`.** The update check offers full releases only. A pre-release would be
  published and offered to nobody; a dry run tries a release without publishing one.
- **The release's text is the changelog section**, so what a release says is written once,
  reviewed with the change, and the same on GitHub and in the repository.
- **Published at once, by the workflow, after the gate: the core's own order** (decision
  0011). A workflow started by hand checks the release commit, runs the whole gate on the
  three systems, builds and checks every file, and only then makes the tag and publishes. A
  person or an agent starts it. No draft waits for a hand: the files go up while the release
  is a draft and it is published in the same step, so a release is never seen half filled.
- **Signing is built in and waits for certificates.** The steps run when their secrets exist
  and say so when they do not. Until then macOS apps are signed ad hoc and Windows programs
  are unsigned.
- **One pipeline, owned by the core.** The packaging is a task of the shared xtask
  (`cargo xtask package`), driven by data in the app's `xtask/src/main.rs`; the workflow is
  `shared-release.yml`, which each app calls at `workflows-v1` through a managed file, as its
  CI is called. An app holds no release workflow and no packaging script.
- **A dry run is a mode of the same workflow**, on demand and once a week in every app, and
  the core packages its template app on the three systems every night. The first real
  release is then not the first run of anything.

## Why the packaging is Rust in the xtask

Of the three ways in use, the xtask module was taken as the base and made general.

- It runs the same on a laptop and in the workflow (`cargo xtask package`), like the gate. The
  two shell versions ran only inside a workflow, on a runner.
- It is tested. The names, the layout of each package, the menu entry, the property list, the
  icon file and the checksum file are checked by unit tests that run in every app's gate; a
  shell script inside a workflow is tested by a release.
- It needs no tool beyond what each system has: `tar` on Linux, `hdiutil` and `codesign` on
  macOS, Windows' own `tar` for the zip. The others needed `7z`, `zip`, `iconutil`, `sips` and
  Python on the runner.
- It reads the app's data where the gate already reads it, so the programs a package holds
  are the programs the license file is written for.

The xtask has no dependencies (decision 0003), so it carries its own SHA-256 and writes the
macOS icon file itself; both are small and are tested against the standard's examples and the
file's layout.

## What was considered and not chosen

- **A draft that a person publishes.** One app had it. It makes a release wait on one person
  for a step that checks nothing the workflow has not checked, and the core dropped the same
  rule for the same reason (decision 0011).
- **Starting on a pushed tag.** A tag is never moved or deleted, so a tag pushed before the
  gate cannot be taken back when the gate fails, and installed copies and a launcher can read
  it meanwhile.
- **Installers (an `.msi`, a `.deb`, an AppImage).** Each needs a tool that is not GitHub's own
  action, a signing identity to be worth having, or both. The three kinds here are the ones
  the updater installs with nothing else on the system; an installer can be added as a
  further file later without changing these.
- **Intel macOS and Linux on ARM.** Not built: a release has a file for the three systems
  named. Adding one is a new row in the contract and in the workflow.
- **Changing craft-update to read each app's names.** It would have kept three schemes alive
  and put the cost on every later reader.

## What it leaves open

- Windows signing with a certificate whose key cannot be exported (a hardware token, a
  signing service). The step here signs with a `.pfx` file; the other kind is added when a
  certificate is chosen.
- In-place updates on macOS, which craft-update does only for its own project's bundle id.

## Revisit when

A fourth system is wanted; a certificate is bought; craft-update's reader changes; or an app
needs a kind of package the three do not cover.
