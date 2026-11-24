# Changelog

Each release says what changed for apps. While at 0.x, a patch release never breaks an app; a
minor release may, and then **What apps must do** says exactly what (decision 0011).

## Unreleased

Nothing yet.

## 0.1.6 (2026-11-24)

### Fixed

- Shared xtask (`template/xtask/src/sync.rs`): `cargo xtask sync` and `sync --check` locate the
  core release the app's own workspace members depend on directly (the newest, should they name
  several), not the first `jcj-doc` in the graph. An app that depends on another app's crates
  pinning an older core builds both releases, and sync used to pick the older one (core issue
  #16). With one core in the graph nothing changes. New `sync::choose_core`, tested in the core's
  xtask.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.6"` when convenient and run
  `cargo xtask sync` to take the fixed module (the patch block is unchanged).

## 0.1.5 (2026-11-24)

### Changed

- `deny.toml`: `allow-git` also lists `https://github.com/jamescjfoundation/wordsmith-manuscript`,
  so an app may depend on Wordsmith Manuscript's published crates at a tag (core issue #15). No
  other source changes; the core's own `cargo deny check sources` passes.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.5"` when convenient and run
  `cargo xtask sync` to take the new `deny.toml` (the patch block is unchanged).

## 0.1.4 (2026-11-24)

### Added

- `jcj-entries` (new crate, layer 2): entries kept beside documents (a story bible, a cast list):
  `Entry<Id, B>` generic over the app's id and body types, `Template`, `TemplateField`,
  `FieldType` (entry types as data), `name_list`, `find_by_name`, `find_mentions` and `mentions`
  (whole words, matching case when a name starts with a capital), `backlinks` over `Document`s.
  Promoted from Wordsmith Manuscript's story bible for Wordsmith Script's characters to share
  (core issue #14, C13). `core-testbed` finds an entry by name in its sample document.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.4"` when convenient (the patch block is
  unchanged). An app that wants entries adds `jcj-entries` at the same tag.

## 0.1.3 (2026-11-24)

### Added

- `jcj-shell`: `split` (`Split`, `SplitState`, `split`, `panes`, `tabs`, `TabEvent`): two panes
  side by side or stacked with a draggable divider, and a strip of document tabs. Promoted for
  Wordsmith Manuscript's split editor (core issue #4).
- `jcj-interchange`: `epub_read`, and `import(Format::Epub, ..)` now reads an EPUB's chapters in
  spine order (core issue #13). `Format::Epub.can_import()` still returns false, so apps that list
  importable formats show the same list; an app that wants EPUB import asks for it by name.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.3"` when convenient (the patch block is
  unchanged).

## 0.1.2 (2026-11-24)

### Added

- `jcj-shell`: `start` (`TemplateSpec`, `chooser`, `start_window`): templates an app lists with
  the new `App::templates` show on the New page; with the new `ShellOptions::start_window` the
  shell shows templates, Open and recent files in place of the document while the new
  `App::has_document` is false. `ids::NEW_FROM_TEMPLATE` (`file.newFromTemplate`). Promoted from
  Wordsmith Script for Wordsmith Manuscript (core issue #2).
- `jcj-shell`: `writing` (`Writing`, `History`, `Sprint`, `Day`, `words_per_day`,
  `progress_bar`): a writing session's net words and target, a sprint timer, words per day,
  deadlines over writing days. Promoted from Wordsmith Script (core issue #3).

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.2"` when convenient (the patch block is
  unchanged). Apps that keep their own start window, templates or writing statistics can move to
  these in follow-up changes.

## 0.1.1 (2026-11-24)

### Added

- `jcj-output`: `read` (`read_pages`, `ReadLimits`, `PdfPage`, `PdfGlyph`, `GlyphMode`,
  `ReadError`): a PDF's pages and where each glyph lands, with its text, for importing PDFs and
  checking exported ones. Bounded for files that are not trusted (size, pages, glyphs a page).
- `jcj-interchange`: `export_rtf_direct`: RTF with each paragraph's resolved indents, spacing,
  alignment and page break, and each run's font, size, bold, italic and underline, written out
  directly, for apps that lay out their own paragraphs. `export(Format::Rtf, ..)` is unchanged.
- `jcj-engine`: `file_access::current` and `unconfined` (which folders are in force; run code with
  none).
- `jcj-proof`: `dictionary_generation` (changes when the user dictionary changes).

### Upstream

- craft-libs' `craft-pdf` reader (same commit as before) and its patched Hayro snapshot
  (`hayro-syntax`, `hayro-interpret`, `hayro-cmap`, `hayro-postscript`, `hayro-jbig2`), in the
  patch block (`docs/UPSTREAM.md`).

### What apps must do

- Move every `jcj-` dependency to `tag = "v0.1.1"`, then run `cargo xtask sync` to take the new
  patch block.

## 0.1.0 (2026-11-24)

The first release of open-source-core, built from the review of Wordsmith Script at `29ed4bf`
(`docs/review/script-29ed4bf.md`) on WordCraft v0.18.1.

### Upstream

- WordCraft v0.18.1 through our fork's `jcj` branch (`73717786`), carrying one patch: extension
  data on documents, paragraphs and runs, kept by resetting and clearing paragraph formatting.
- craft-libs at the commits WordCraft v0.18.1 pins; egui 0.37 and winit on storytold's forks.
- Pins, patches and known issues: `docs/UPSTREAM.md`.

### Crates

- `jcj-upstream`: the one crate that names upstream code (apps never use it).
- `jcj-doc`: the document model and typed extension keys (`ExtKey`, `HasExt`).
- `jcj-layout`: pages, page boxes, hit testing, measuring, page plans, the `PaginationPass` hook,
  bundled fonts.
- `jcj-engine`: sessions, undo, the registry (`registry_with`), recovery, native JSON, and general
  commands the engine lacked: find and replace, plain/reset/capitals formatting, page break,
  delete paragraph, paragraph selection, spelling with a per-document word list.
- `jcj-output`: deterministic PDF, page images, printing (`print` feature).
- `jcj-proof`: spelling, suggestions, user words, hyphenation.
- `jcj-interchange`: DOCX, ODT, RTF, Markdown, HTML, text import and export; an EPUB 3 writer;
  emphasis markers.
- `jcj-project`: the project container (one document or many, assets, snapshots, the app's data
  and migrations), atomic saves, backups, recovery files, auto-save timing.
- `jcj-features`: bookmarks, notes, revision sets, change tracking, page locks, watermarks,
  headers and footers, comparison, with their commands.
- `jcj-theme`: design tokens (`tokens.json`) to egui themes; default interface fonts from
  WordCraft's bundled OFL faces.
- `jcj-board`: cards, corkboard, canvas.
- `jcj-shell`: ribbon, panels, File page, palette, dialogs, preferences, keys, auto-save, updates,
  icons, offscreen pictures.
- `jcj-agents`: an app's commands as MCP tools.
- `core-testbed`, `contract-tests`, `template/` (a new app's starting point and the files every
  app keeps in step with `cargo xtask sync`).

### Tooling

- `cargo xtask ci` (fmt, lint-roots, layers, pins, template, privacy, licenses, clippy, tests),
  `ci --changed` for affected packages only, git hooks, `template-check`, `vendor`,
  `third-party-licenses`; cargo-deny; CI on GitHub-hosted runners (decision 0003).

### Known gaps

No EPUB import, no PDF/X; the page planner does not plan footnotes, floats or columns and keeps
tables whole; spelling and hyphenation tests run without the optional craft-i18n language data.

### What apps must do

Start from `template/` and follow `docs/porting-script.md` (for Wordsmith Script).
