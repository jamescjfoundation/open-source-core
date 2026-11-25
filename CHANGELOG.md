# Changelog

Each release says what changed for apps. While at 0.x, a patch release never breaks an app; a
minor release may, and then **What apps must do** says exactly what (decision 0011).

## Unreleased

Nothing yet.

## 0.1.14 (2026-11-25)

### Fixed

- `jcj-shell`: the Preferences page's auto-save interval and idle-time fields are named, for
  screen readers, by the words beside them (core issue #26). Before, they were unnamed spin
  buttons. A shell test and a `core-testbed` test check that no control on the File pages or the
  testbed's window lacks a name.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.14"` when convenient (the patch block is
  unchanged).

## 0.1.13 (2026-11-25)

### Added

- `jcj-i18n` (new crate): interface translation catalogs (core issue #18). One TOML file per
  language maps each text, keyed by its English, to the text shown; plurals take the language's
  forms (`trn`), `{name}` placeholders are filled (`tr_with`); catalogs are layered, the first
  that has a text giving it; a text no catalog has stays English. `texts_in`, `singulars_in` and
  `compare` let a test check that every text is in every catalog and none is unused.
- `jcj-shell`: `i18n` (`LANGUAGES`, `catalog`, `set_language`): the shell's own texts in Spanish,
  and a language turned on with the app's catalogs layered over the shell's. The shell shows its
  texts, and the labels, titles, tooltips, tab and group names apps give it, through the
  catalogs. `core-testbed` switches its window to Spanish (`CORE_TESTBED_LANG=es`).

### What apps must do

- Nothing. Until an app calls `jcj_shell::i18n::set_language`, every text is shown in English
  exactly as before. Move every `jcj-` dependency to `tag = "v0.1.13"` when convenient (the patch
  block is unchanged).

## 0.1.12 (2026-11-25)

### Fixed

- `jcj-interchange`: the EPUB writer keeps a document's footnotes and endnotes, as numbered
  endnotes at each chapter's end (custom marks kept), each linked from its reference and back,
  with `noteref` and `endnote` semantics. Before, they were left out (core issue #21).
  `core-testbed` checks that a footnote survives EPUB export and import.

### What apps must do

- Nothing. Apps that wrote notes into their EPUBs by hand to work around this can stop when
  convenient. Move every `jcj-` dependency to `tag = "v0.1.12"` (the patch block is unchanged).

## 0.1.11 (2026-11-25)

### Added

- `jcj-output`: `boxes` (`set_page_boxes`, `Boxes`, `BoxError`): trim and bleed boxes set on an
  exported PDF's pages as an incremental update (the original bytes kept), deterministic and
  bounded (core issue #25). `core-testbed` sets trim boxes and reads them back with `read_facts`.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.11"` when convenient (the patch block is
  unchanged).

## 0.1.10 (2026-11-24)

### Added

- `jcj-output`: `facts` (`read_facts`, `PdfFacts`, `PageFacts`, `PageBox`, `PdfImage`, `PdfFont`):
  a PDF's page boxes (media, crop, trim, bleed), rotation, annotations, images with their pixel
  and placed size (so their resolution), fonts with whether each is embedded, and bookmarks,
  read with the same bounds as `read_pages` (core issue #23). `core-testbed` checks that its PDF
  embeds every font.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.10"` when convenient (the patch block is
  unchanged).

## 0.1.9 (2026-11-24)

### Added

- `jcj-features`: `verify` (`words`, `paragraph_words`, `document_words`, `compare`, `Difference`,
  `Check`, `CONTEXT`): the word-for-word check of an import or export against its source, the
  first difference reported with the words around it. Promoted from Wordsmith Manuscript (core
  issue #10). `core-testbed` checks that every format it exports and imports keeps every word.
- `jcj-features`: `wordiff` (`tokens`, `paragraph`, `paragraphs`, `documents`, `Diff`, `Segment`,
  `Change`): two versions of a text compared paragraph by paragraph, then word by word, beside
  the paragraph-level `compare`. Promoted from Wordsmith Manuscript (core issue #17).
  `core-testbed` shows a changed word.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.9"` when convenient (the patch block is
  unchanged). Apps with their own copies can move to these in follow-up changes.

## 0.1.8 (2026-11-24)

### Added

- `jcj-shell`: `tree` (`tree`, `TreeRow`, `TreeState`, `TreeEvent`, `Zone`, `zone`): a tree of rows
  with selection, open and closed rows, rename in place, drag and drop before, after or inside a
  row, and a context menu event. The caller passes the visible rows and gets events back; only
  rows in view are drawn, and each is named as a tree item for screen readers. Promoted from
  Wordsmith Manuscript's binder for a second app's outline (core issue #6, C5 of Manuscript's
  plan, C3 of Book's). `core-testbed` shows the sample document's headings in a tree panel.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.8"` when convenient (the patch block is
  unchanged). An app with its own tree can move to this one in a follow-up change.

## 0.1.7 (2026-11-24)

### Added

- `jcj-doc`: `section` (`SectionStart`, `NumFormat`, `HeaderSet`, `Columns`, `NoteProps`,
  `NoteOptions`, `NotePos`, `NoteRestart`, `ChapterSep`, `PageVAlign`, `LineNumbering`,
  `LineNumberRestart`, `DocGrid`, `DocGridType`): the types of `SectionProps`' fields, so apps can
  start a section on an odd or even page, restart and format page numbers and set header and
  footer stories. `para::NoteKind`, so apps can add footnote and endnote references (core issue
  #24). `jcj-layout` tests that an odd-page start is by page number and that a footnote sits at
  the foot of its reference's page; `core-testbed` lays out both.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.7"` when convenient (the patch block is
  unchanged).

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
