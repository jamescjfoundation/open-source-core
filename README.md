# open-source-core

The J. C. J. Foundation's shared base for its open-source desktop apps. It holds the code every
app needs and none should write twice — the app shell, project files, format conversion, cards and
boards, theming, agent control — and it is the only place that talks to the open-source projects
the apps build on: [WordCraft](https://github.com/storytold/wordcraft) and
[craft-libs](https://github.com/storytold/craft-libs) by the ArtCraft team, used at pinned
releases through the foundation's forks.

It carries no product name. Wordsmith Script, Wordsmith Manuscript and Wordsmith Book are its first
apps; any later foundation app builds on it the same way.

## The crates

| Crate | What apps get from it |
| --- | --- |
| `jcj-doc` | The document model (documents, paragraphs, runs, styles, notes, revisions) and typed extension data each app attaches to it |
| `jcj-layout` | Pages: lay out a document, page boxes, hit testing, the primitives a pagination pass needs, fonts |
| `jcj-engine` | Editing sessions, undo and redo, the command registry, recovery, general editing commands |
| `jcj-output` | PDF, page images, printing |
| `jcj-interchange` | DOCX, ODT, RTF, Markdown, HTML, EPUB and plain text |
| `jcj-proof` | Spelling and hyphenation |
| `jcj-entries` | Entries beside documents: typed records, templates, names, mentions, backlinks |
| `jcj-i18n` | Interface translation catalogs: texts keyed by their English, plurals, placeholders, layered lookup |
| `jcj-project` | The project file: one document or many, assets, atomic saves, backups, recovery, snapshots |
| `jcj-features` | Document features every app shares: bookmarks, notes, revision sets, change tracking, page locks, watermarks, headers and footers, comparison |
| `jcj-theme` | A product's design tokens turned into the shell's theme |
| `jcj-board` | Cards, corkboard and canvas |
| `jcj-shell` | Window, ribbon, panels, File page, dialogs, preferences, auto-save, updates |
| `jcj-agents` | An app's commands as MCP tools for agents |
| `jcj-upstream` | Internal: the one crate that names upstream code. Apps never use it |

`apps/core-testbed` uses every crate; `contract-tests` checks the behavior apps rely on against
every upstream candidate; `template/` starts a new app repository.

## Using it in an app

```toml
[workspace.dependencies]
jcj-doc = { git = "https://github.com/jamescjfoundation/open-source-core", tag = "v0.1.0" }
jcj-shell = { git = "https://github.com/jamescjfoundation/open-source-core", tag = "v0.1.0" }
# ... the other jcj- crates you use, all at the same tag
```

Copy `template/` to start an app; `cargo xtask sync` (in the app) keeps the toolchain, lint,
cargo-deny settings, shared xtask modules, CI workflow and the core's `[patch]` block identical to
the pinned core release, and the app's CI checks they stay so. To work on the core and an app
together, `cargo xtask dev-link ../open-source-core` in the app.

The rules for deciding whether a change belongs in an app or the core are in
[AGENTS.md](AGENTS.md), and the reasons in [docs/decisions/](docs/decisions/README.md). Finished
plans and published reviews are kept under [docs/history/](docs/history/); they are not maintained.

## Building

```sh
cargo run -p core-testbed     # the test app
cargo xtask ci                # the full gate; `cargo xtask ci --changed` for just your change
cargo xtask install-hooks     # pre-commit and pre-push hooks running the same checks
```

Rust is pinned in `rust-toolchain.toml` (1.100.0; minimum 1.95). See [CONTRIBUTING.md](CONTRIBUTING.md).

## Upstream

Pins, carried patches and the follow / hold / own state of every upstream source are in
[docs/UPSTREAM.md](docs/UPSTREAM.md). The core moves to WordCraft's newest release every two weeks,
on our schedule; our apps never wait on anyone else.

## License

MIT OR Apache-2.0, at your option. Based on WordCraft by the ArtCraft team (see [NOTICE](NOTICE)).
