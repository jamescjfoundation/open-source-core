# 0018: A project is a folder of plain files

Date: 2026-11-26 · Status: accepted · Adds to decision 0009 (the container) · Core issue #45

## Context

Decision 0009 said "one container for every app": a zip holding a manifest and each document as
WordCraft's native JSON. Two apps were built after it, and neither uses it. Wordsmith Manuscript
(its decision 0001) and Wordsmith Book (its decision 0002) each keep a project as a folder of
plain files, because a writer must be able to read the work without the app, compare two
versions, sync it and keep it under version control, and because a long project with pictures
should not be rewritten whole on every save.

Each app then wrote the same layer around its own files:

| | Wordsmith Manuscript (`crates/manuscript-format`) | Wordsmith Book (`crates/book-format`) |
| --- | --- | --- |
| The folder | `folder.rs`: `Folder::{open, create, save, save_to}`, `Sink`, `Disk`, an index of what each file held | `folder.rs`: the same names, an index of one hash a file |
| The manifest | `project.toml` with `schema`; a newer one refused | `book.toml` with `schema`; a newer one refused, the nesting of the TOML bounded first |
| One file | `pack.rs`: `pack_files`, `unpack`, `unpack_into`, limits, names checked | `pack.rs`: `pack`, `unpack`, `unpack_into`, limits, names checked |
| Recovery | `.recovery/unsaved.wsmz`: `write_recovery`, `recovered`, `discard_recovery`, `save_safely` | `.recovery/unsaved.wsbkz`: the same four |
| Backups | `backups`, `backup`: a dated packed copy, the oldest removed | the same two, with a number for a second backup in one second |
| Changes made elsewhere | `outside_changes`, `accept`, a copy under `conflicts/` before a save writes over one | none |

About 1,100 lines in one and 730 in the other. The limits, the zip's shape and the order of a
save were chosen twice and already differ in small ways: which names a packed file may hold,
whether a second backup in one second replaces the first, which files count as a project's
backups. The core's container is used by its own test app and by the app template.

## Decision

- **The core's standard for an app's project is a folder of plain files.** The container of
  decision 0009 stays, for a document that is one file: a single text, a file sent to someone,
  an app with nothing a person would open by hand. An app chooses one; neither is removed.
- **`jcj_project::folder` is the layer every folder project shares.** It is in `jcj-project`
  beside the container, not a crate of its own: the two share the atomic writes, the file
  gateway that keeps an agent to its folders, the backup naming and the zip library, and an app
  that offers both forms depends on one crate.
- **What a folder project is** (`crates/jcj-project/FOLDER.md` is the specification):
  - a **manifest** at the folder's root, TOML, with `schema = <number>` at its top. A folder
    without it is not a project; one with a higher schema than the app reads is refused; a
    lower one is the app's to migrate;
  - **plain files** the app renders from its model: text, and whatever it keeps as files;
  - **assets** the app may carry without reading (pictures, media);
  - `.recovery/`, where unsaved work is kept, and optionally a folder for conflict copies;
  - any other file in the folder is the user's: never read, packed, changed or removed.
- **An app describes its folder with data** (`folder::Format`): the manifest's name and schema,
  the files written last, which paths belong to the project, the packed form's `mimetype` and
  extension, which endings are stored uncompressed, whether conflict copies are kept and how
  they are stamped, and the limits. Everything else is the same code for every app.
- **The app reads, the layer remembers.** The layer does not know what a file means, so it
  loads no model. The app reads the files it wants, each within a size it names, through
  `Source`, which a folder on disk and files in memory both are: one reader serves the folder,
  a packed project and a recovery copy. A folder remembers what each file held.
- **A save writes what changed, and only that.** The app renders its project to a map of path
  to bytes. A file is written when its bytes differ from what the model rendered it to when it
  was read or last saved, so text typed by hand in another form stays byte for byte until the
  app changes it. Each file is written atomically; the files named last (the manifest by
  default) come after every other, so an interrupted save leaves the old ones naming whole
  files. A file is removed only when the app held it (a save wrote it, or the app read it and
  said what it renders to) and no longer renders it; a file that was only looked at never is.
- **Changes made elsewhere are found by asking, not by watching.** `Folder::outside_changes`
  compares the files with what was read. There is no file-system watcher: notifications are
  unreliable on synced and network folders, which is where such changes come from, and a
  watcher is a dependency and a thread for a question an app asks when its window gains focus
  and before it saves. A save never writes over or removes a file changed elsewhere without
  keeping a copy first, when the format has a folder for them.
- **The packed form is a zip any tool opens**: a stored `mimetype` entry first, then every file
  in name order with a fixed time and permissions, so the same files give the same bytes and
  two backups compare. Reading never trusts it: the file's size, the number of entries, each
  entry's size counted while reading, and the total are bounded; every name must be a plain
  path the format allows; nothing is extracted to disk before all of it passed, and the app's
  own reader has read it from memory.
- **Symbolic links are never followed**, in reading, listing or packing. A project that came
  from somewhere else must not be able to name a file outside itself.
- **Recovery** is a packed copy of the unsaved state at `.recovery/unsaved.<extension>` inside
  the folder, written before a save and while editing, removed after a good save, never packed.
- **Backups** are dated packed copies, named as decision 0009 names them
  (`<name> <YYYY-MM-DD HH.MM.SS> UTC.<extension>`, a number for a second in one second, never
  replacing a file), the oldest removed beyond the number kept. Only a file with exactly that
  name is a project's backup: a file named by hand, or another project's whose name starts the
  same, is never counted or removed.
- **What stays the app's**: what its files hold and how they are parsed and written; the rest
  of the manifest; migration from an older schema; which files it reads and the size it allows
  each kind; the counts it allows (items, assets); its messages, which name the product.
  Errors are `jcj_project::Error`; an app turns them into its own.

## What each app changes to use it

Neither app is moved by this decision. Each is listed here, with what its files on disk would
do: nothing, in both. That was checked, not argued: each app's format crate was built beside
the layer and the two were compared on the same projects. The packed project, a new folder,
the folder after a save, the recovery copy and a backup came out the same bytes under the same
names from both, and each read what the other wrote.

**Wordsmith Manuscript.** A `Format`: `project.toml`, schema 3; `project.toml` and
`binder.toml` written last; a path belongs when it is `binder.toml` or under `text/`, `bible/`,
`media/`, `scripts/`, `snapshots/`, `conflicts/` or `drafts/`; `mimetype`
`application/x-wordsmith-manuscript-packed`, extension `wsmz`; nothing stored uncompressed;
conflict copies in `conflicts/`, stamped with its own snapshot stamp; its four packed limits.

- `folder.rs` keeps `load`, the reader, written over the core's `Source` and calling
  `rendered` where it computes a file's model hash today. `Sink`, `Disk`, `Stamp`, `Saved`,
  `OutsideChange`, `Source`, `DiskSource`, `MemorySource`, `read_text`, `hash`, `conflict_name`
  and `Folder` with its six methods are the core's.
- `pack.rs` keeps `pack_project`, `pack_current` and `read_packed` as a few lines each over the
  core. `allowed_name`, `pack_files`, `folder_files`, `pack_folder`, `PackLimits`, `unpack`,
  `unpack_with`, `unpack_into`, the four recovery functions, `backups` and `backup` are the
  core's.
- `files.rs` (what each file holds), `limits.rs` and the dialect are untouched.
- On disk: the project's files are written by the same atomic writer with the same bytes, and
  a packed project has the same entries in the same order with the same options, so the same
  bytes.
- What behaves differently, none of it a format change: the text files of one save are written
  in name order, not the binder's (the manifest and the binder still last); a second backup in
  one second gets a number where today it replaces the first; a file in the backups folder
  that is not named as a backup is no longer listed or removed, and `backups` gives the oldest
  first; the `.recovery` folder is removed after a save only when it is empty; a text file
  that is a symbolic link is read as missing, where today it is followed when read and left
  out when packed.

**Wordsmith Book.** A `Format`: `book.toml`, its model's schema, written last alone; a path
belongs when no part of it is hidden and it holds no control character, names at most 200
bytes; `mimetype` `application/x-wordsmith-book-packed`, extension `wsbkz`; `.wsmz`, `.png`
and `.jpg` stored uncompressed; no conflict copies; its four packed limits.

- `folder.rs` keeps `Files` with `render` and `from_map`, `safe_name`, `safe_id` and the three
  folder names. `Sink`, `Disk`, `Saved`, `Folder`, `read_folder` (three calls of `read_tree`)
  and `read_bounded`, which today tells "too large" from other errors by reading the message,
  are the core's.
- `pack.rs` keeps `Packed` and `gather`, which add the owned source's files and the linked
  source's packed copy to the project's own. The zip writing and reading, `unpack_into`,
  `write_file`, the four recovery functions, `backups` and `backup` are the core's.
- `toml_file.rs` keeps the migration and the checks of the project; its `nesting` is the
  core's `toml_nesting`.
- On disk: the same files with the same bytes, and the same packed bytes.
- What behaves differently: backups are listed by their exact name, as above, and two made in
  the same second are listed in the order they were made, where today the numbered second one
  sorts before the first and would be removed first; a folder is read 16 levels deep, not 8.
  Conflict copies stay off until Book wants them: a new folder in the project is Book's own
  format decision.

## Why

Two apps reached the same design alone, which is the evidence decision 0013 asks for before a
thing is general. A third app gets a project folder with tested limits, recovery and backups
for a hundred lines of its own, and a fix to a limit or to the zip reader reaches every app.

Keeping the container costs nothing: it is small, tested, and right for a document that is one
file.

## Revisit when

- An app needs a file-system watcher after all (an editor that must follow another program's
  writes as they happen).
- A project folder must be read lazily in parts the layer cannot see as files (a database
  inside the folder).
- A manifest in another syntax than TOML is wanted; `schema_of` is the only part that reads it.
