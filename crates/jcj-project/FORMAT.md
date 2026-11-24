# The project file

This is the specification of the file `jcj-project` reads and writes. A project holds one
document or many, the media they use, snapshots of documents and the app's own data. The file
extension is the app's choice (`.notes`, `.proj` ...); the content is the same for every app.

Container version: **2** (`jcj_project::CONTAINER_VERSION`).

## Layout

A project file is a zip archive (APPNOTE 6.3, Deflate or Stored entries, no encryption):

```text
mimetype                     first entry, Stored (uncompressed), no extra field
manifest.json                what the project holds (below)
app.json                     the app's own data for the project (optional)
documents/<id>.json          each document, WordCraft native JSON
snapshots/<id>.json          each snapshot's copy of a document, WordCraft native JSON
assets/<key>                 media by key, raw bytes (Stored)
```

- **`mimetype`** holds exactly `application/vnd.jcj.project+zip` (ASCII, no newline). It comes
  first and uncompressed, as in ODF and EPUB, so a project is recognized from its first 70 bytes.
  A file without it is not a project.
- **Documents** are `jcj_doc::Document` serialized as WordCraft's native JSON (the same JSON
  WordCraft saves as `.json`). It is the only format that keeps every app's extension data on
  documents, paragraphs and runs, and everything else the model holds (styles, numbering,
  sections, comments, revisions, embedded pictures as base64).
- **Assets** are project-level media (a cover picture, research images) the app refers to by
  key. Pictures inside a document stay in that document's JSON.
- Entries not listed here are ignored by readers (and dropped when the project is saved again),
  so later versions may add entries without a version change.

Writing is deterministic: entry times are fixed (1980-01-01 00:00), JSON objects have their keys
in sorted order and are written compactly, and entries come in the order above (documents in
project order, snapshots oldest first, assets by key). Saving a project that did not change gives
the same bytes.

## manifest.json

```json
{
  "app": { "format": 3, "id": "org.example.notes" },
  "assets": ["3f2a...e1.png", "cover.jpeg"],
  "documents": [
    { "id": 1, "name": "Chapter One" },
    { "id": 2, "name": "Notes" }
  ],
  "format": 2,
  "nextId": 4,
  "snapshots": [
    { "document": 1, "id": 3, "name": "First draft", "taken": "2026-11-19T15:03:22Z" }
  ]
}
```

| Field | Meaning |
| --- | --- |
| `format` | The container version. Required. |
| `app.id` | The app the project belongs to. A key (see below); reverse-domain style is suggested. |
| `app.format` | The version of the app's own data in the project (see Migration). |
| `nextId` | The next free id. Documents and snapshots share one id counter and ids are never reused. |
| `documents` | The documents in project order: a numeric `id` (its entry is `documents/<id>.json`) and a display `name` (any text, at most 1,024 bytes; names need not be unique). |
| `snapshots` | Named, dated copies of documents: `id` (entry `snapshots/<id>.json`), the `document` it copies, a `name`, and `taken`, RFC 3339 in UTC. |
| `assets` | Asset keys; each is stored at `assets/<key>`. |

**Keys** (app ids and asset keys) are 1 to 128 bytes of ASCII letters, digits, `.`, `_` and `-`,
not starting with `.`, so they are always safe entry names.

## app.json

A JSON object of the app's own data for the whole project, by key (apps namespace their keys:
`notes.tags`). Left out when empty. Data that belongs to one document, paragraph or run goes in
that item's extension data instead, so it is copied, compared, undone and snapshotted with it.

## Reading limits

Readers bound everything a hostile file could inflate (`jcj_project::Limits`, defaults below) and
answer a damaged file with an error, never a crash:

| Bound | Default |
| --- | ---: |
| File size | 1 GiB |
| One entry, uncompressed (measured while inflating, not taken from the header) | 256 MiB |
| All entries read, uncompressed | 2 GiB |
| Zip entries (counted from the end record before the archive is parsed) | 40,000 |
| Documents / snapshots / assets | 10,000 / 20,000 / 10,000 |
| `manifest.json`, `app.json` | 16 MiB each |
| JSON nesting | 128 levels (serde_json's limit) |

A manifest with duplicate ids, a snapshot of a document that is not listed, an invalid key, or a
listed entry that is missing or does not parse is damaged, and the file is refused. Runs whose
lengths do not fit their paragraph's text are repaired as WordCraft repairs them.

Saving checks the same counts and entry sizes, so the core never writes a file it would refuse.

## Versions and migration

There are two version numbers, owned by two parties:

- **The container version** (`format`), owned by the core. A reader refuses a newer container
  with a clear message and migrates an older one step by step (1 → 2 → ...), each step a function,
  so every older file stays readable. A project is always saved in the current version.
- **The app's data version** (`app.format`), owned by the app (`jcj_project::App`). When a
  project saved by an older version of the app is opened, the core calls the app's migration hook
  (`App::with_migration`) once with the old version and the whole project (documents with their
  extension data, snapshots, assets, app data); the hook updates what it must, and the project is
  then marked with the app's current version. Without a hook the project opens as it is. A
  project saved by a newer version of the app, or by another app, is refused.

### Version history

- **1** (the draft layout from the core's first development milestone; never released, kept
  readable as the worked example of a container migration and to keep the migration path
  tested): `manifest.json` was
  `{"format": 1, "app": "<app id>", "documents": ["<name>", ...], "assets": ["<key>", ...]}`,
  documents were stored by position (`documents/0.json`, `documents/1.json`, ...), and there were
  no ids, snapshots, app data version or `app.json`.
  *Migration to 2:* documents get ids 1, 2, ... in order, `nextId` follows them, the app's data
  version is 0, and there are no snapshots.
- **2** (current): ids, snapshots, the app's data version, `app.json`.

## On disk

These are behaviours of `jcj_project::files`, not of the format, listed here so all apps behave
alike:

- **Saving is atomic** (craft-atomic): the new file is written beside the old one, flushed, and
  renamed over it; a failed or interrupted save leaves the old file whole.
- **Backups** are copies of the file as saved, in a folder the app chooses, named
  `<name> <YYYY-MM-DD HH.MM.SS> UTC.<ext>` (a number is added for a second backup in the same
  second; an existing file is never replaced). The oldest beyond the number to keep (1,000 by
  default, at most 10,000) are removed.
- **The recovery file** is a full project file beside the project, named
  `.<file name>.recovery`. Auto-saves may write it instead of the project file; a successful save
  removes it; on opening, a recovery file newer than the project means unsaved changes from a
  session that did not end cleanly.
