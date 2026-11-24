# 0006: Extension data on documents, paragraphs and runs

Date: 2026-11-24 · Status: accepted · Replaces Wordsmith Script decision 0008 (`app_data`) and the
nine screenplay fields Script added to WordCraft's model

## Context

Each app keeps its own data in the document: Script's scene numbers, omitted scenes, beats,
bookmarks, scene ids and dual-dialogue flags on paragraphs, and note, revision and page-lock marks
on text runs; Manuscript's labels and status; Book's chapter roles. Script added nine fields to its
copy of WordCraft's model plus `Document::app_data`. Upstream models cannot carry fields for every
app.

## Decision

- **One general carrier, patched into our WordCraft fork:** `wordcraft_doc::Ext`, a map from a
  namespaced key (`"app.name"`) to a JSON value, on `Document`, `ParaProps` and `CharProps`
  (`ext` fields). The model copies, compares, undoes and saves it without understanding it. It is
  left out of the native JSON when empty, so existing documents read and write unchanged.
  Formats that cannot hold it (DOCX, RTF, ...) drop it.
- **Run data is a mark, not formatting** (as Script's note, revised and page-lock fields were):
  Clear Formatting keeps it, `CharProps::formatting` leaves it out, `minus`/`overlay` work key by
  key. **Paragraph data is never inherited from a style** (as Script's scene number was).
- **Apps use typed keys** from `jcj-doc`: `const SCENE_NUMBER: ExtKey<String> =
  ExtKey::new("script.scene_number");` then `SCENE_NUMBER.get(&para)` / `.set(&mut para, &v)`.
  A value of the wrong shape is an error value, never a panic. Setting a paragraph's value marks it
  changed for layout caches.
- **Namespaces** are the app's id (`script.`, `manuscript.`, `book.`) or `core.` for the core's
  own data. An app never writes another app's namespace; when one app opens another's document
  (Manuscript to Book), it reads the other namespace through that app's published keys.
- **Offered upstream** as a general feature ("like custom XML in Word"); carried until taken.

## Consequences for Script (M4)

Script's `.wss` gets a format version bump: the nine fields move into `script.*` keys and
`app_data` entries into document `ext`, with a migration that opens every old file
(`docs/porting-script.md`).
