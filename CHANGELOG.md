# Changelog

Each release says what changed for apps. While at 0.x, a patch release never breaks an app; a
minor release may, and then **What apps must do** says exactly what (decision 0011).

## Unreleased

### Added

- **The core's rules reach every app by `cargo xtask sync`, not by hand** (decision 0003,
  amended). Until now the rules every app carries were about 115 lines copied by hand into five
  files, and `SECURITY.md`, `CONTRIBUTING.md` and the code of conduct were written separately
  in each repository; nothing noticed a copy that had fallen behind.
  - A marked block is the text from a line `<!-- BEGIN open-source-core: NAME -->` to a line
    `<!-- END open-source-core: NAME -->`. Its source is the same block in a file under
    `template/`. The text around a block is the app's own.
  - Five blocks (`BLOCKS` in `template/xtask/src/sync.rs`): in `AGENTS.md`, `app` ("Before you
    commit", "House rules for code"), `public` ("What is public") and `app-and-core` ("Building
    between an app and the core"); in `SECURITY.md`, `security` ("Reporting a problem", "How we
    handle a report"); in `CONTRIBUTING.md`, `contributing` ("Rules for a public repository",
    "Commits, pushes and pull requests").
  - `cargo xtask sync` replaces a block where its two marker lines are. A file an app does not
    have yet is written whole from `template/`. A file an app has without the marker lines is
    left alone and named in the error, after everything else is written: `sync` never chooses a
    place for a block.
  - `cargo xtask sync --check`, a task of every app's gate, names each file or block that is
    missing or differs from the pinned core release.
  - `CODE_OF_CONDUCT.md` is a whole managed file, worded for any project
    (`template/CODE_OF_CONDUCT.md`).
  - `template/SECURITY.md` and `template/CONTRIBUTING.md` are new: the core's block, and
    placeholders for the app's own parts.
- In the core: `cargo xtask template --write` copies the blocks from `template/` into the core's
  own `AGENTS.md`, `SECURITY.md` and `CONTRIBUTING.md`, and the code of conduct. The `template`
  task of the gate fails when one of them differs. The core carries every block but `app`: it has
  its own "Before you commit" and house rules for code.

### Changed

- A record of the core is cited as "core decision NNNN" in everything an app carries: linked in
  the marked blocks, and with the address of the core's `docs/decisions/` folder in `deny.toml`,
  `rust-toolchain.toml`, the shared xtask modules and the two workflows. A bare "decision 0003"
  in a copied file pointed at an app's own record of that number.
- The wording of the carried rules, so that one text is true in the core and in an app: "the
  core's `docs/decisions/`" for "here", "an app" for "this app". Three sentences say more than
  before: the brand "lives in the app's repository or in its project's brand kit, never in the
  core"; a security problem in the shared parts may be reported in the core or in the app, and
  the report is moved; "Things agents must not do" names editing a file or block the core owns.
- `template/AGENTS.md` no longer tells an agent to read the core's `AGENTS.md` as well: the
  blocks are that text.
- `README.md` and the opening of `AGENTS.md` say where a project built on the core keeps its map,
  and link the foundation's own.

### What apps must do

An app that carries the rules as hand copies does this once, after moving its pin to this
release.

1. Run `cargo xtask sync`. It brings the new `sync.rs` and says to run it again.
2. In `AGENTS.md`, find the hand copy of the core's rules (from "## Before you commit" to the end
   of "## House rules for code"). Delete it and put these six lines in its place, in this order,
   each pair with one empty line after it:

   ```
   <!-- BEGIN open-source-core: app -->
   <!-- END open-source-core: app -->

   <!-- BEGIN open-source-core: public -->
   <!-- END open-source-core: public -->

   <!-- BEGIN open-source-core: app-and-core -->
   <!-- END open-source-core: app-and-core -->
   ```

   Keep the app's opening lines, a project block if the app has one, and the app's own rules.
   Remove "Read the core's AGENTS.md too" from the opening.
3. In `SECURITY.md`, delete the app's "## Reporting a problem" and "## How we handle a report"
   and put the pair `<!-- BEGIN open-source-core: security -->`,
   `<!-- END open-source-core: security -->` under the title. Keep "## What counts" below it.
4. In `CONTRIBUTING.md`, delete the app's "## Rules for a public repository" and its commit and
   pull request section, and put the pair `<!-- BEGIN open-source-core: contributing -->`,
   `<!-- END open-source-core: contributing -->` there. Move any rule that is the app's own
   (a naming rule, a commit subject form, a progress file) to a section below the pair; do not
   keep a second copy of a rule the block states.
5. Run `cargo xtask sync` again. It fills the five blocks and replaces `CODE_OF_CONDUCT.md` with
   the core's. If it names a block it could not place, the marker lines for that block are
   missing or misspelled; each must be a line of its own.
6. Read the three files once through: text outside a block must not contradict the block, and a
   bare "decision NNNN" in the app's own text that means a core record becomes "core decision
   NNNN".
7. `cargo xtask sync --check`, then `cargo xtask ci`.

Nothing changes in an app's `xtask/src/main.rs`, and no code changes.

## 0.1.16 (2026-11-26)

### Added

- Shared xtask (`template/xtask/src/corepin.rs`, new, managed): `cargo xtask core-pin` checks
  that an app builds on one core release (decision 0011). Until now nothing did: an app could
  name two tags, hold two cores in its graph, or sit many releases behind unnoticed.
  - **One tag.** Every line of the root `Cargo.toml` that names the core's git address carries a
    release tag (`tag = "vX.Y.Z"`), all of them the same; the lines that differ are listed.
  - **One core in the graph.** `Cargo.lock` holds packages from one ref of the core's address.
    A second one is named with the packages built from it and the packages that bring them in
    (another app's crates that pin a different core, for example; decision 0015).
  - **Distance.** `cargo xtask core-pin --behind` lists the core's release tags
    (`git ls-remote`, so it needs the network), orders them as versions and prints, for example,
    `core pin v0.1.4 is 11 releases behind v0.1.15`, then `pinned=`, `newest=` and `behind=`
    lines for a workflow to read. `--max-behind <n>` fails when the pin is more than `n` behind.
  - The first two read two files and are a task of the template's `ci` (`core-pin`, after
    `unlinked`); the distance is never part of `ci`. A workspace with no core pin passes.
  - In the core, which has no pin of its own, `cargo xtask core-pin <app dir> [--behind]
    [--max-behind <n>]` looks at an app checkout, as `cargo xtask sync <app dir>` does.
- `template/.github/workflows/core-pin.yml` (new, managed): every Monday, and on demand, an app
  runs `cargo xtask core-pin --max-behind 3`. When the pin is more than three releases behind,
  the workflow opens an issue in the app, "Move to open-source-core vX.Y.Z", with the distance, a
  link to this changelog and the steps. While such an issue is open no second one is opened: it
  is renamed and rewritten when the core has released again. It uses only the run's own token
  (`issues: write`). CI stays green for a release an app does not need yet.
- Shared xtask (`template/xtask/src/sync.rs`): `cargo xtask sync` says when `sync.rs` itself
  changed, because the run that brings a longer list of managed files cannot yet copy the files
  the list adds; and `sync` and `sync --check` print a note while the app's own
  `xtask/src/main.rs` does not run the core-pin check.
- `cargo xtask template-check` runs the template's `core-pin` task.

### Changed

- The rules every app carries (`template/AGENTS.md`, from "Before you commit" to the end) follow
  the decisions recorded with this release: how a change lands and what "no product name" covers
  (0013), who cuts a release and how far behind an app may be (0011), upstream pins (0004,
  accepted), reviews and working notes (0012), apps that depend on apps (0015, new), and the
  entries and translation crates (0016, new). The port plan and the first review moved to
  `docs/history/`.

### What apps must do

- Move every `jcj-` dependency to `tag = "v0.1.16"`, then run `cargo xtask sync` **twice**: the
  first run brings the new list of managed files, the second the two files it adds
  (`xtask/src/corepin.rs` and `.github/workflows/core-pin.yml`). `cargo xtask sync --check` passes
  after the second. The patch block is unchanged.
- Add the check to the app's own `xtask/src/main.rs` (four lines; `template/xtask/src/main.rs`
  shows them in place). Nothing fails without them, but then nothing is checked, and the new
  workflow's run fails because the task is unknown:
  1. With the other `mod` lines: `mod corepin;`
  2. In `TASKS`, after the `unlinked` entry:
     `("core-pin", "every jcj- dependency names one core release tag, and Cargo.lock holds one core"),`
  3. In `main`, among the arms of `match task`, before the `name =>` arm:
     `"core-pin" => corepin::task(&root, rest),`
  4. In `run_task`, among the arms of `match name`: `"core-pin" => corepin::task(root, &[]),`
- The workflow arrives with the sync and needs no setting. It opens issues, so issues must be
  turned on in the repository.
- Replace the shared rules in the app's `AGENTS.md` (from "Before you commit" to the end of
  "House rules for code") with the same part of `template/AGENTS.md`. This is still a copy by
  hand; nothing checks it yet.
- An app whose lock file holds two cores fails `cargo xtask ci` once it has the task. That is the
  point of the check: move the dependency that brings the second core to a release built on the
  app's own core release (decision 0015) in the same change, or first.

## 0.1.15 (2026-11-25)

### Added

- `jcj-output`: `conformance` (`declare`, `Conformance`, `ConformanceError`, `srgb_profile`):
  PDF/A-2b and PDF/UA-1 declared on a tagged export as an incremental update: the
  identifications in the XMP metadata (PDF/UA's schema declared for PDF/A), an output intent
  with an sRGB profile made in code, `DisplayDocTitle`, and the language and author kept the
  same in the XMP and the information dictionary (core issue #27). Checked with the standard
  validator where it is installed (`JCJ_PDF_VALIDATOR`): both standards pass. `core-testbed`
  declares both on its sample.

### What apps must do

- Nothing. Move every `jcj-` dependency to `tag = "v0.1.15"` when convenient (the patch block is
  unchanged).

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
(`docs/history/script-29ed4bf.md`) on WordCraft v0.18.1.

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

Start from `template/` and follow `docs/history/porting-script.md` (for Wordsmith Script).
