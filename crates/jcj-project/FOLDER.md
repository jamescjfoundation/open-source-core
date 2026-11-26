# The project folder

This is the specification of a project kept as a folder of plain files, as
`jcj_project::folder` reads and writes it (decision 0018). What the files hold is each app's
own format, described in that app's repository; what is here is the same for every app.
`FORMAT.md` beside this file is the other form, the single-file container.

## Layout

```text
My Project.<the app's extension>/
  <manifest>                   a TOML file the app names, with `schema = <number>`
  <the app's root files>       any others the app's format names
  <the app's folders>/...      text files, assets
  <conflicts folder>/...       copies of files changed elsewhere (if the app keeps them)
  .recovery/unsaved.<ext>      unsaved work, as a packed project (while the app runs)
  anything else                the user's own: never read, packed, changed or removed
```

- **The manifest** is TOML and holds the whole number `schema` at its top level: the version
  of the app's files in this folder. The rest of it is the app's. A folder without the
  manifest, or whose manifest has no such number, is not a project. The manifest is at most
  4 MiB unless the app says otherwise, and its tables and arrays nest at most 32 deep.
- **A newer schema is refused**: a project that declares a higher schema than the app reads is
  not opened, with a message that says so. A lower one is opened and the app migrates it.
- **Paths** inside a project are written with `/` between their parts, on every system. A path
  is *plain* when it is not empty, is no longer than the app's limit (512 bytes unless it says
  otherwise), does not start with `/`, holds no backslash, colon or NUL, and none of its parts
  is empty, `.` or `..`. Only a plain path that the app's format says belongs is a file of the
  project. `mimetype` at the root and anything under `.recovery/` never are.
- **Symbolic links** are never followed: a link is not read, not listed and not packed.

## Saving

The app renders its project to a set of files, path to bytes, and a save brings the folder to
it:

1. A file is written when its bytes differ from what the app rendered it to when it was read
   or last saved. A file the app read and would render differently from its bytes on disk
   (text typed by hand in another form) is therefore left as it is until the app changes it.
2. Every file is written atomically: to a temporary file beside it, flushed, then renamed. An
   interrupted save leaves each file either as it was or as it is now.
3. The files the format names last are written after every other, in the format's order; by
   default that is the manifest alone. An interrupted save therefore leaves the old manifest,
   which names files that are whole.
4. A file is removed when the app held it and no longer renders it. The app holds a file a
   save wrote, and a file it read and rendered. A file it only read is never removed.
5. Before a file is written over or removed, if the bytes on disk are neither what was last
   read or written nor what is about to be written, and the format has a conflicts folder,
   the file is first copied there as `<time> <path with / as ->`. The time is
   `YYYY-MM-DD HH.MM.SS UTC` unless the app has its own.

## The packed form

One file that holds the folder, with the extension the app gives it. It is a zip archive
(APPNOTE 6.3, Deflate or Stored entries, no encryption):

```text
mimetype                       first entry, Stored, exactly the app's packed type
<path>                         every file of the project, in the order of their paths' bytes
```

- Every entry has the time 1980-01-01 00:00:00 and the permissions `0644`, and no entry is a
  folder, so the same files always give the same bytes.
- A file whose name ends in one of the endings the app's format names (pictures, an archive
  inside) is Stored; every other is Deflate.
- `.recovery/` is never packed; nor is a file that is not the project's, nor a symbolic link.

Reading never trusts the file. It is refused when it is larger than 2 GiB; when it is not a
zip, is empty, or does not start with a `mimetype` entry holding the app's packed type; when
it has more than 50,000 files; when an entry is larger than 512 MiB (counted while reading,
whatever its header says) or all of them together are larger than 4 GiB; when a name is not a
plain path the format allows; when a name appears twice. The four numbers are the defaults,
and an app's format may set its own. Entries are read one by one into memory. Nothing is
written to disk until all of them passed and the app has read the project from memory.

## Recovery

While a project has unsaved changes, the app keeps them as a packed project at
`.recovery/unsaved.<packed extension>` inside the folder, written atomically. It holds the
files the app renders, not the assets it only carries. A save writes this copy first, then
the folder, then removes the copy; the `.recovery` folder is removed with it when it is
empty. A copy found when a project is opened is what a run that did not end cleanly left, and
the app offers it.

## Backups

A backup is the packed form, written into a folder the app chooses as
`<name> <YYYY-MM-DD HH.MM.SS> UTC.<packed extension>`: the project's name, the time in UTC so
names sort and never repeat across a clock change, then, for a second backup in the same
second, a space and a number from 2. An existing file is never replaced. After a backup the
oldest of the same project beyond the number kept are removed. Only a file with exactly this
name is one of the project's backups.
