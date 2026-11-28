# open-source-core

[![CI status](https://img.shields.io/github/actions/workflow/status/jamescjfoundation/open-source-core/ci.yml?branch=main&label=CI&style=flat-square&labelColor=1C2340)](https://github.com/jamescjfoundation/open-source-core/actions/workflows/ci.yml)
[![nightly status](https://img.shields.io/github/actions/workflow/status/jamescjfoundation/open-source-core/nightly.yml?branch=main&label=nightly&style=flat-square&labelColor=1C2340)](https://github.com/jamescjfoundation/open-source-core/actions/workflows/nightly.yml)
[![Latest release](https://img.shields.io/github/v/release/jamescjfoundation/open-source-core?label=release&color=3B4FA8&style=flat-square&labelColor=1C2340)](https://github.com/jamescjfoundation/open-source-core/releases/latest)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-3B4FA8?style=flat-square&labelColor=1C2340)](#license)
[![Written in Rust](https://img.shields.io/badge/written%20in-Rust-5A6080?logo=rust&style=flat-square&labelColor=1C2340)](https://www.rust-lang.org)

The James C. J. Foundation's shared base for its open-source desktop apps. It holds the code
every app needs and none should write twice — the app shell, project files, format conversion,
cards and boards, theming, agent control — and it is the only place that talks to the open-source
projects the apps build on: [WordCraft](https://github.com/storytold/wordcraft) and
[craft-libs](https://github.com/storytold/craft-libs) by the ArtCraft team, used at pinned
releases through the foundation's forks.

> **A note from the founder**
>
> A lot of work has already gone into laying a solid foundation. The apps aren't ready for everyday use yet, but the idea is sound and the pieces are in place. From here, the plan is slow, steady development and testing.
>
> I've been a wannabe developer for years. AI has finally given me the tools to bring an idea like this together, but my time and resources are limited. Ideas, feedback, bug reports, fixes and feature improvements are all welcome. If you care about this as much as we do, we're also open to deeper partnerships.
>
> I'll keep improving the ecosystem where I can while building out the rest of the foundation.
>
> — Dylan J.

It carries no product name in its code, its interface text or the files apps copy from it.
Wordsmith Script, Wordsmith Manuscript and Wordsmith Book are its first apps; any later
foundation app, or anyone else's, builds on it the same way. A project built on the core keeps its
own map of its repositories, outside the core. The foundation's own is the
[Wordsmith project map](https://github.com/jamescjfoundation/wordsmith/blob/main/project/README.md),
which says how the Wordsmith repositories depend on this one and gives the steps for a change that
crosses them.

## The crates

| Crate | What apps get from it |
| --- | --- |
| `jcj-doc` | The document model (documents, paragraphs, runs, styles, notes, revisions), typed extension data each app attaches to it, and a text's words: the word list, the word-for-word check and the word-level comparison of two versions |
| `jcj-layout` | Pages: lay out a document, page boxes, hit testing, the primitives a pagination pass needs, fonts |
| `jcj-engine` | Editing sessions, undo and redo, the command registry, recovery, general editing commands |
| `jcj-output` | PDF, page images, printing |
| `jcj-interchange` | DOCX, ODT, RTF, Markdown, HTML, EPUB and plain text |
| `jcj-proof` | Spelling and hyphenation |
| `jcj-entries` | Entries beside documents: typed records, templates, names, mentions, backlinks |
| `jcj-i18n` | Interface translation catalogs: texts keyed by their English, plurals, placeholders, layered lookup |
| `jcj-project` | Projects on disk. A folder of plain files (the standard for an app's project): what changed is saved atomically, changes made elsewhere are found, the folder packs to one file, with recovery and backups. And the single-file container: one document or many, assets, snapshots |
| `jcj-features` | Document features every app shares: bookmarks, notes, revision sets, change tracking, page locks, watermarks, headers and footers, comparison |
| `jcj-theme` | A product's design tokens turned into the shell's theme, and its brand files loaded: tokens, the window's icon, a mark painted from a mask |
| `jcj-board` | Cards, corkboard and canvas |
| `jcj-shell` | Window, ribbon, panels, File page, dialogs, the system's file dialogs, preferences, keys, auto-save, updates, the start window and template chooser, writing statistics, the door to interface translations, and an app's whole start-up from its data |
| `jcj-agents` | An app's commands as MCP tools for agents, on a document or on the app's own state, and the parts of a command line every app shares (exit codes, `mcp` arguments) |
| `jcj-upstream` | Internal: the one crate that names upstream code. Apps never use it |

`apps/core-testbed` uses every crate; `contract-tests` checks the behavior apps rely on against
every upstream candidate; `template/` starts a new app repository.

## Using it in an app

```toml
[workspace.dependencies]
jcj-doc = { git = "https://github.com/jamescjfoundation/open-source-core", tag = "vX.Y.Z" }
jcj-shell = { git = "https://github.com/jamescjfoundation/open-source-core", tag = "vX.Y.Z" }
# ... the other jcj- crates you use, all at the same tag
```

`vX.Y.Z` is a release tag; [CHANGELOG.md](CHANGELOG.md) lists them, newest first, with what each
asks of an app.

Copy `template/` to start an app ([template/README.md](template/README.md) has the steps);
`cargo xtask sync` (in the app) keeps the toolchain, lint,
cargo-deny settings, shared xtask modules (the gate among them), workflows, code of conduct, the
core's `[patch]` block and the core's rules and settings (marked blocks in the app's `AGENTS.md`,
`SECURITY.md`, `CONTRIBUTING.md`, `Cargo.toml`, `.cargo/config.toml`, `.gitignore` and
`.gitattributes`) identical to the pinned core release, and the app's gate checks they stay so.
To work on the core and an app together, `cargo xtask dev-link ../open-source-core` in the app:
the link lives in a folder Git ignores and changes no tracked file.

The rules for deciding whether a change belongs in an app or the core are in
[AGENTS.md](AGENTS.md), and the reasons in [docs/decisions/](docs/decisions/README.md). Finished
plans and published reviews are kept under [docs/history/](docs/history/); they are not maintained.

## Building

```sh
cargo run -p core-testbed     # the test app
cargo xtask ci                # the full gate; `cargo xtask ci --changed` for just your change
cargo xtask install-hooks     # once: pre-commit (fmt, privacy, no dev-link in a tracked file) and pre-push (ci --changed) hooks
```

Rust is pinned in `rust-toolchain.toml` (1.100.0; minimum 1.95). [CONTRIBUTING.md](CONTRIBUTING.md) says where things are and how a release is cut.

## Upstream

Pins, carried patches and the follow / hold / own state of every upstream source are in
[docs/UPSTREAM.md](docs/UPSTREAM.md). The core moves to WordCraft's newest release every two weeks,
on our schedule; our apps never wait on anyone else.

## License

MIT OR Apache-2.0, at your option. Based on WordCraft by the ArtCraft team (see [NOTICE](NOTICE)).
