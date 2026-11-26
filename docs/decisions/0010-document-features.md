# 0010: Document features every app shares (`jcj-features`)

Date: 2026-11-24 · Status: accepted · From Wordsmith Script decisions 0021 (headers), 0023
(notes), 0024 (revisions), 0026 (page locking), 0031 (bookmarks) and 0035 (production additions)

## Context

Script built bookmarks, notes, revision sets, change tracking, page locks, a watermark, headers
and footers and version comparison for screenplays, but almost none of it is about screenplays
(`docs/history/script-29ed4bf.md`: 0 to 3 % screenplay terms, comparison aside). Any foundation app
that writes long documents wants them. Script kept their state in fields it added to its copy of
WordCraft's model (`ParaProps::bookmark`, `CharProps::note`, `revised`, `page_lock`) and in
`Document::app_data`.

The plan first put them in "`jcj-doc` extensions and `jcj-output`". They need commands, the
engine's typing and the pagination primitives, which sit above the model, so they get a crate of
their own.

## Decision

- **One crate, `jcj-features`, layer 4.** It depends on `jcj-doc`, `jcj-engine` and `jcj-layout`
  only (never on the shell, theme or board). One module per feature, each with its data, its
  commands (`commands()`, passed with the app's own to `jcj_engine::registry_with`) and its tests;
  `jcj_features::commands()` is all of them.
- **All data in extension data, `core.` namespace** (decision 0006), through typed keys:

  | Feature | Data |
  | --- | --- |
  | Bookmarks | paragraph `core.bookmark` (id); document `core.bookmarks` (names, label flag) |
  | Notes | run mark `core.note` (id); document `core.notes`, `core.note_types` |
  | Revisions | run and paragraph mark `core.revised` (set index); document `core.revisions` |
  | Change tracking | the model's own tracked changes and `settings.track_changes` |
  | Page locks | run mark `core.page_lock` (lock id); document `core.page_locks` |
  | Watermark | document `core.watermark` |
  | Headers and footers | document `core.header_footer` |

  A value that does not read gives the feature's defaults; each group bounds what it read.
- **Typing keeps the marks right.** The engine's typing copies the marks before the caret. The
  crate registers `text.insert`, `text.newParagraph`, `text.backspace` and `text.delete` (same
  labels and keys) that run the engine's and then: text typed at a note's edge is not in the note;
  revision mode marks typed text and deleted-in paragraphs; a page lock stays on its page's first
  character; a split paragraph keeps its bookmark on the first part only.
- **Pages through the pagination hook.** `locks::LockPass` wraps an app's `PaginationPass`: a lock
  on a paragraph's first character starts a page there; a lock inside a paragraph lets it split
  only there. Commands that need pages ask `pages::planned_pages`, which an app points at its own
  pagination with `pages::set_pages`. Placeholders for deleted locked pages are data
  (`PageLabels::placeholders`): the pass cannot add empty pages.
- **Output helpers work on the paginated output** (a pass's `finish`): `watermark::apply` (text
  boxes behind the page; a drawing call in `jcj-output` was not needed), `header_footer::apply`
  (one section per labelled page) and `revisions::decorate_output` (set text styles, page colors).
- **What apps supply.** Header and footer fields beyond `{page}`, `{date}`, `{title}`,
  `{revision}` and `{collated}` come from a `FieldSource` (a `{section}` field, say). Comparison
  groups paragraphs through a `Grouping` (`WholeDocument`, `ByStyle`, or the app's own units, with
  its own moved-note text). The revision color cycle is a setting; empty means the default
  production cycle (White, Blue, Pink, ...). Note types default to a general list. Writers'
  colors take an app palette.

## What came from where

| Script decision | Module | Kept | Changed or left in Script |
| --- | --- | --- | --- |
| 0021 Title page and headers | `header_footer` | Bands, slots, fields, first page, start page, labelled pages | `{scene}` became an app field (`FieldSource`); the title page stays in Script |
| 0023 Notes | `notes` | Anchors as run marks, detached notes, fields, types, stepping | Default types are general; the listing gives paths, not scenes |
| 0024 Revisions | `revisions` | Sets, cycle and rounds, mode, marks, rollback, configure | The cycle is a setting with the old one as default; margin marks stay with each app's pagination |
| 0026 Page locking | `locks` | Labels and letters, levels, placeholders, every lock command | Pagination through `LockPass` on the core's pass; placeholders are data |
| 0031 Editing additions (bookmarks) | `bookmarks` | Ids on paragraphs, labels, stepping | Pages from `planned_pages` |
| 0035 Production | `watermark`, `tracking`, `compare` (and locks) | Watermark settings and drawing; tracked-change listing, settling, stepping | Tracking uses the model's switch the engine already follows; comparison groups through `Grouping` (scene matching stays in Script, on top); dual dialogue stays |

Script's `dates` and `data` helpers came along as `dates` and `data`.

## Consequences

- Script (M4) reads its old fields into these keys (`docs/history/script-29ed4bf.md`, section 4) and
  composes `LockPass` with its own pass.
- Pasted text keeps the marks it was copied with (a pasted noted word is in the same note).
- `track.*` settles changes to text runs; tracked paragraph marks, moves and formatting changes
  are the engine's `review.*` commands' work.

## Revisit when

A second app needs a lock inside a paragraph to force a break (a pagination hook in `jcj-layout`
for breaks at a byte), or placeholder pages in the output.
