# 0009: The project container, saving, backups and recovery

Date: 2026-11-24 · Status: accepted · Amended 2026-11-26 (the container and the folder) ·
General parts of Wordsmith Script decisions 0017 and 0020

Amended 2026-11-26: the container below is no longer the one form for every app. The standard
for an app's project is a folder of plain files (decision 0018, `jcj_project::folder`), which
the two apps built since this record both chose. The container stays as it is, for a document
that is one file; nothing here is removed, and an app picks one form. The two share this
record's atomic saves, its backup names and its recovery idea, and a folder project's packed
form follows this container's shape (a stored `mimetype` entry first, the same bytes for the
same content, reading that never trusts the file).

## Decision

- **One container** (`jcj-project`; "for every app" until the 2026-11-26 amendment): a zip holding `mimetype` (first, stored),
  `manifest.json` (container version, app id, app format version, document list),
  optional `app.json` (the app's project-wide data), `documents/<id>.json` (WordCraft's native
  JSON, so extension data survives), `snapshots/<id>.json` and `assets/<key>`. The app picks the
  file extension. The layout is specified in `crates/jcj-project/FORMAT.md`.
- **A project holds one document or many**, each with a stable id, in order; assets keyed by
  content hash or by the app; snapshots are named, dated copies of one document.
- **Versions.** The container has its own version (2). Each app declares its id and data version;
  a project from another app or from a newer version is refused with a clear error; an older one
  runs the app's migration hook once. Version 1 is an unreleased draft layout kept to test the
  migration path.
- **Same bytes for the same project**, so saving twice changes nothing and backups compare.
- **Reading never trusts the file**: sizes, entry counts and nesting are capped; the zip is read
  entry by entry into memory (never `extract`, decision 0001); damaged runs are repaired.
- **Saving** is atomic through craft-atomic; after a successful save the recovery file is
  removed. **Backups** are time-stamped copies (`<name> <YYYY-MM-DD HH.MM.SS> UTC.<ext>`, a number added for a second in the same second) in a folder the app chooses, the oldest removed beyond the number the app keeps. **Recovery** files are
  written beside the project while there are unsaved changes and offered at the next start.
- **Auto-save timing** (Script 0020: after the first change, once the interval has passed and the
  user has paused; a failed save waits another interval) is `jcj_project::AutoSave`, on a clock
  the caller passes in; `jcj-shell` drives it.
