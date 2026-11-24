# Changelog

Each release says what changed for apps. While at 0.x, a patch release never breaks an app; a
minor release may, and then **What apps must do** says exactly what (decision 0011).

## Unreleased

Nothing yet.

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
