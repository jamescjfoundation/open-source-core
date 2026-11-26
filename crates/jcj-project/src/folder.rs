// SPDX-License-Identifier: MIT OR Apache-2.0
//! A project as a folder of plain files (decision 0018; `FOLDER.md` beside this crate is the
//! specification).
//!
//! A project is a folder the user can read, copy, sync and put under version control: a
//! manifest file at its root that says which version of the app's files it holds, text files,
//! and assets. What the files mean is the app's; what is the same in every app is here:
//!
//! - **Opening.** [`Folder::open`] refuses a folder without the manifest and a project saved by
//!   a newer version of the app. The app then reads the files it wants through [`Source`], each
//!   within a size it names, and the folder remembers what every file held.
//! - **Saving.** The app renders its project to [`Files`] and [`Folder::save`] writes only the
//!   files that changed, each atomically, the manifest last, and removes the files that are
//!   gone. A file whose content the app did not change is never rewritten, so text written by
//!   hand stays byte for byte as it was ([`Source::rendered`]).
//! - **Changes made elsewhere.** [`Folder::outside_changes`] says which files another program,
//!   device or sync service changed since they were read, and a save never writes over such a
//!   change without keeping a copy of it ([`Format::conflicts`]).
//! - **One file.** [`pack`] makes the folder one file, the same bytes for the same files;
//!   [`unpack`] reads one back within limits, entry by entry, never trusting it.
//! - **Recovery.** [`write_recovery`] keeps unsaved work inside the folder, [`recovered`] finds
//!   what a run that did not end cleanly left, and [`Folder::save_safely`] puts the two around
//!   a save.
//! - **Backups.** [`backup`] writes a dated packed copy and removes the oldest beyond a number.
//!
//! An app describes its folder with a [`Format`], which is data:
//!
//! ```
//! use jcj_project::folder::{self, Files, Folder, Format, Source};
//!
//! fn belongs(path: &str) -> bool {
//!     path == "notes.toml" || path.starts_with("text/") || path.starts_with("assets/")
//! }
//! const NOTES: Format = Format { belongs, ..Format::new("notes.toml", 1, "application/x-example-notes-packed", "notesz") };
//!
//! let root = std::env::temp_dir().join(format!("jcj-project-folder-doc-{}", std::process::id()));
//! # let _ = std::fs::remove_dir_all(&root);
//! let mut files = Files::new();
//! files.insert("notes.toml".into(), b"schema = 1\ntitle = \"Notes\"\n".to_vec());
//! files.insert("text/one.md".into(), b"The first note.\n".to_vec());
//! let mut project = Folder::create(&root, &NOTES, &files).unwrap();
//!
//! // Only what changed is written.
//! files.insert("text/one.md".into(), b"The first note, revised.\n".to_vec());
//! assert_eq!(project.save(&files).unwrap().written, ["text/one.md"]);
//!
//! // The folder as one file, and back.
//! let packed = folder::pack_folder(&NOTES, &root).unwrap();
//! assert_eq!(folder::unpack(&NOTES, &packed).unwrap(), files);
//!
//! // Opened again, the app reads what it wants.
//! let mut again = Folder::open(&root, &NOTES).unwrap();
//! assert_eq!(again.read_text("text/one.md", 1 << 20).unwrap().as_deref(), Some("The first note, revised.\n"));
//! # let _ = std::fs::remove_dir_all(&root);
//! ```
//!
//! Every read and write passes the automation file gateway and every write is atomic, as in
//! [`crate::files`]. Nothing here reads a file without a bound or follows a symbolic link.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

use crate::{Error, Result, files};

/// A project's files: path inside the folder (parts joined with `/`) to bytes.
pub type Files = BTreeMap<String, Vec<u8>>;

/// A content hash (SHA-256).
pub type Hash = [u8; 32];

/// The hash of some bytes.
pub fn hash(bytes: &[u8]) -> Hash {
    Sha256::digest(bytes).into()
}

/// The folder inside a project where unsaved work is kept. It is never packed.
pub const RECOVERY_DIR: &str = ".recovery";

/// The zip entry that says what a packed project is: first, and stored.
const MIMETYPE_ENTRY: &str = "mimetype";

/// How deep folders are followed inside a project.
const MAX_DEPTH: usize = 16;

/// The deepest tables and arrays may nest in a manifest.
pub const MAX_MANIFEST_NESTING: usize = 32;

/// Bounds on what is read from a project folder or a packed project.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// The largest manifest, bytes.
    pub manifest: u64,
    /// The longest path inside the project, bytes.
    pub name: usize,
    /// The largest packed project, bytes.
    pub packed: u64,
    /// The largest single file in a packed project, bytes.
    pub entry: u64,
    /// The most files in a packed project or walked in a folder.
    pub entries: usize,
    /// The most bytes of a packed project once unpacked, or of a folder read to pack it.
    pub unpacked: u64,
}

impl Limits {
    /// 4 MiB of manifest, paths of 512 bytes, a 2 GiB packed file, 512 MiB a file, 50,000
    /// files, 4 GiB unpacked.
    pub const DEFAULT: Limits = Limits { manifest: 4 << 20, name: 512, packed: 2 << 30, entry: 512 << 20, entries: 50_000, unpacked: 4 << 30 };
}

impl Default for Limits {
    fn default() -> Self {
        Limits::DEFAULT
    }
}

/// Every plain path belongs ([`Format::belongs`]'s default).
pub fn any_path(_path: &str) -> bool {
    true
}

/// How one app's project folder is laid out. It is data: build one with [`Format::new`] and name
/// the fields that differ, so a field added later takes its default.
#[derive(Clone, Copy, Debug)]
pub struct Format {
    /// The manifest: a TOML file at the folder's root with `schema = <number>` at its top. A
    /// folder without it is not a project.
    pub manifest: &'static str,
    /// The newest schema this build of the app reads. A project that declares a higher one is
    /// refused ([`Error::NewerApp`]); a lower one is the app's to migrate.
    pub schema: u32,
    /// The files written after every other, in this order, so an interrupted save leaves the
    /// old ones naming whole files. Empty: the manifest alone.
    pub last: &'static [&'static str],
    /// Whether a path is one of the project's files (its root files, a file under one of its
    /// folders). The path is plain already ([`plain_path`]). Other files in the folder are the
    /// user's: never read, packed, changed or removed.
    pub belongs: fn(&str) -> bool,
    /// The `mimetype` entry of the packed form.
    pub mimetype: &'static str,
    /// The packed form's file extension, without the dot.
    pub packed_extension: &'static str,
    /// Endings of files stored in a packed project as they are, because they are compressed
    /// already (`".png"`, `".jpg"`).
    pub stored: &'static [&'static str],
    /// The folder inside the project where a save keeps a copy of a file changed elsewhere
    /// before writing over it. `None`: no copy is kept.
    pub conflicts: Option<&'static str>,
    /// The time in a conflict copy's name ([`files::stamp`] unless the app has its own).
    pub conflict_stamp: fn(SystemTime) -> String,
    /// The bounds.
    pub limits: Limits,
}

impl Format {
    /// A format with the manifest `manifest` at schema `schema`, packed as `mimetype` with the
    /// extension `packed_extension`: every plain path belongs, nothing is stored uncompressed,
    /// no conflict copies, the default limits.
    pub const fn new(manifest: &'static str, schema: u32, mimetype: &'static str, packed_extension: &'static str) -> Format {
        Format {
            manifest,
            schema,
            last: &[],
            belongs: any_path,
            mimetype,
            packed_extension,
            stored: &[],
            conflicts: None,
            conflict_stamp: files::stamp,
            limits: Limits::DEFAULT,
        }
    }

    /// Whether `path` may be a file of the project: a plain path ([`plain_path`]) that the
    /// format says belongs, not the recovery folder and not the packed form's own `mimetype`.
    pub fn allows(&self, path: &str) -> bool {
        plain_path(path, self.limits.name)
            && path != MIMETYPE_ENTRY
            && path.split('/').next() != Some(RECOVERY_DIR)
            && (path == self.manifest || (self.belongs)(path))
    }

    /// Where the files written last come in a save: after every other file, in their order.
    fn rank(&self, path: &str) -> usize {
        let last: &[&str] = if self.last.is_empty() { std::slice::from_ref(&self.manifest) } else { self.last };
        last.iter().position(|l| *l == path).map_or(0, |i| i + 1)
    }

    /// Where a copy of `path` is kept when a save would write over a change made elsewhere:
    /// `<conflicts>/<time> <path with slashes as dashes>`. `None` when the format keeps none.
    pub fn conflict_name(&self, path: &str, at: SystemTime) -> Option<String> {
        self.conflicts.map(|dir| format!("{dir}/{} {}", (self.conflict_stamp)(at), path.replace('/', "-")))
    }
}

/// Whether `path` is a plain path inside a folder: at most `max` bytes, parts joined with `/`,
/// none empty, `.` or `..`, no leading slash, and no backslash, colon or NUL (so it means the
/// same file on every system and cannot leave the folder).
pub fn plain_path(path: &str, max: usize) -> bool {
    !path.is_empty()
        && path.len() <= max
        && !path.starts_with('/')
        && !path.contains(['\\', ':', '\0'])
        && path.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
}

// ---- The manifest's schema ----------------------------------------------------------------------

/// How deep `[` and `{` nest in TOML `text`, outside strings and comments: a cheap guard before
/// parsing a file that is not trusted.
pub fn toml_nesting(text: &str) -> usize {
    let (mut depth, mut max) = (0usize, 0usize);
    let mut in_string: Option<char> = None;
    let (mut escape, mut comment) = (false, false);
    for c in text.chars() {
        if comment {
            comment = c != '\n';
            continue;
        }
        if let Some(quote) = in_string {
            if escape {
                escape = false;
            } else if c == '\\' && quote == '"' {
                escape = true;
            } else if c == quote || c == '\n' {
                in_string = None;
            }
            continue;
        }
        match c {
            '#' => comment = true,
            '"' | '\'' => in_string = Some(c),
            '[' | '{' => {
                depth += 1;
                max = max.max(depth);
            }
            ']' | '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    max
}

/// The schema a manifest declares: the whole number `schema` at the top of its TOML.
/// [`Error::NotAProject`] when it declares none; [`Error::Damaged`] when it is not TOML or
/// nests deeper than [`MAX_MANIFEST_NESTING`].
pub fn schema_of(text: &str) -> Result<u32> {
    if toml_nesting(text) > MAX_MANIFEST_NESTING {
        return Err(Error::Damaged(format!("the manifest nests deeper than {MAX_MANIFEST_NESTING} levels")));
    }
    let table: toml::Table = text.parse().map_err(|e: toml::de::Error| Error::Damaged(format!("the manifest: {}", e.message())))?;
    let schema = table.get("schema").and_then(toml::Value::as_integer).ok_or(Error::NotAProject)?;
    u32::try_from(schema).map_err(|_| Error::Damaged(format!("the manifest: schema {schema} is not a version")))
}

/// Read the manifest from `source` and check it against `format`: [`Error::NotAProject`]
/// without one, [`Error::NewerApp`] when it was saved by a newer version of the app. Gives the
/// manifest's text and the schema it declares, which the app migrates when it is older.
pub fn read_manifest(format: &Format, source: &mut dyn Source) -> Result<(String, u32)> {
    let text = source.read_text(format.manifest, format.limits.manifest)?.ok_or(Error::NotAProject)?;
    let found = schema_of(&text)?;
    if found > format.schema {
        return Err(Error::NewerApp { found, supported: format.schema });
    }
    Ok((text, found))
}

// ---- Reading ------------------------------------------------------------------------------------

/// Where a project's files are read from: a folder on disk ([`Folder`]) or files in memory
/// ([`Memory`]: a packed project, a recovery copy). An app writes its reader once, over this.
pub trait Source {
    /// A file's bytes, `None` when it is missing; [`Error::TooLarge`] over `limit` bytes.
    fn read(&mut self, path: &str, limit: u64) -> Result<Option<Vec<u8>>>;

    /// The names of the files directly in the folder `dir` inside the project, sorted; empty
    /// when the folder is missing. `""` is the project's root.
    fn list(&mut self, dir: &str) -> Result<Vec<String>>;

    /// Say what the app's model renders `path` to, once it has read the file. A source that
    /// remembers its files ([`Folder`]) then leaves the file alone on a save until the model
    /// changes, even when the bytes on disk are written differently (by hand, by an older
    /// version). Without this call a file is rewritten when its rendering differs from its
    /// bytes on disk.
    fn rendered(&mut self, path: &str, bytes: &[u8]) {
        let _ = (path, bytes);
    }

    /// A file as UTF-8 text; [`Error::Damaged`] when it is not.
    fn read_text(&mut self, path: &str, limit: u64) -> Result<Option<String>> {
        match self.read(path, limit)? {
            None => Ok(None),
            Some(bytes) => String::from_utf8(bytes).map(Some).map_err(|_| Error::Damaged(format!("{path}: not UTF-8 text"))),
        }
    }
}

/// Files in memory as a [`Source`].
#[derive(Clone, Copy, Debug)]
pub struct Memory<'a>(pub &'a Files);

impl Source for Memory<'_> {
    fn read(&mut self, path: &str, limit: u64) -> Result<Option<Vec<u8>>> {
        let Some(bytes) = self.0.get(path) else { return Ok(None) };
        if bytes.len() as u64 > limit {
            return Err(too_large(path, limit));
        }
        Ok(Some(bytes.clone()))
    }

    fn list(&mut self, dir: &str) -> Result<Vec<String>> {
        let prefix = if dir.is_empty() { String::new() } else { format!("{dir}/") };
        Ok(self.0.keys().filter_map(|k| k.strip_prefix(&prefix)).filter(|rest| !rest.is_empty() && !rest.contains('/')).map(str::to_owned).collect())
    }
}

fn too_large(what: &str, limit: u64) -> Error {
    Error::TooLarge(format!("{what} is larger than the limit of {limit} bytes"))
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> Error + '_ {
    move |source| Error::Io { path: path.to_path_buf(), source }
}

/// A file's bytes within `limit`, `None` when it is missing. A symbolic link is not followed.
fn read_file(root: &Path, path: &str, limit: u64) -> Result<Option<Vec<u8>>> {
    let full = root.join(path);
    match std::fs::symlink_metadata(&full) {
        Ok(meta) if meta.is_file() => {}
        Ok(_) => return Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io(&full)(e)),
    }
    match files::read_bounded(&full, limit) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(Error::TooLarge(_)) => Err(too_large(path, limit)),
        Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// The plain files directly in `dir`, by name, sorted: no folders, no symbolic links.
fn list_dir(dir: &Path) -> Result<Vec<String>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io(dir)(e)),
    };
    let mut out: Vec<String> =
        entries.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_file())).filter_map(|e| e.file_name().to_str().map(str::to_owned)).collect();
    out.sort();
    Ok(out)
}

/// The paths of every file under `root` that `format` allows, sorted: no symbolic links, not
/// the recovery folder, nothing deeper than [`MAX_DEPTH`] folders. `inside` keeps the walk to
/// one folder of the project (`""`: all of it).
fn walk(format: &Format, root: &Path, inside: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut stack = vec![(if inside.is_empty() { root.to_path_buf() } else { root.join(inside) }, inside.to_owned(), 0usize)];
    while let Some((dir, rel, depth)) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(io(&dir)(e)),
        };
        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else { continue };
            let path = if rel.is_empty() { name } else { format!("{rel}/{name}") };
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_dir() {
                if path != RECOVERY_DIR && depth < MAX_DEPTH {
                    stack.push((entry.path(), path, depth + 1));
                }
            } else if kind.is_file() && format.allows(&path) {
                out.push(path);
                if out.len() > format.limits.entries {
                    return Err(Error::TooLarge(format!("the project folder holds more than {} files", format.limits.entries)));
                }
            }
        }
    }
    out.sort();
    Ok(out)
}

// ---- The folder ---------------------------------------------------------------------------------

/// What is known about one file since it was read or saved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    /// The bytes on disk.
    pub disk: Hash,
    /// What the app's model rendered the file to when it was read or saved.
    pub model: Hash,
}

impl Stamp {
    /// A file whose bytes on disk are `disk` and whose model renders to `rendered`.
    pub fn of(disk: &[u8], rendered: &[u8]) -> Stamp {
        Stamp { disk: hash(disk), model: hash(rendered) }
    }
}

/// A file the folder knows: what it held, and whether it is the app's to remove.
#[derive(Clone, Copy, Debug)]
struct Known {
    stamp: Stamp,
    /// The app holds this file in its model: a save wrote it, or the app read it and said what
    /// it renders to. Only such a file is removed when the app no longer renders it.
    held: bool,
}

/// What a save did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Saved {
    /// Files written, in order.
    pub written: Vec<String>,
    /// Files removed.
    pub removed: Vec<String>,
    /// Copies kept of files that had changed elsewhere since they were read, each made before
    /// this save wrote over the file or removed it.
    pub conflicts: Vec<String>,
}

/// A file changed outside the app since it was read or last saved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutsideChange {
    /// The file, inside the project.
    pub path: String,
    /// What it holds now; `None` when it was removed.
    pub now: Option<Vec<u8>>,
}

/// Where a save's files go: the folder on disk ([`Disk`]), or in a test somewhere that fails on
/// cue, to prove what an interrupted save leaves.
pub trait Sink {
    /// Write a whole file, atomically.
    fn write(&mut self, path: &str, bytes: &[u8]) -> Result<()>;
    /// Remove a file; one already gone is not an error.
    fn remove(&mut self, path: &str) -> Result<()>;
}

/// A project folder on disk as a [`Sink`].
#[derive(Clone, Debug)]
pub struct Disk {
    root: PathBuf,
}

impl Disk {
    /// The folder at `root`.
    pub fn new(root: &Path) -> Disk {
        Disk { root: root.to_path_buf() }
    }
}

impl Sink for Disk {
    fn write(&mut self, path: &str, bytes: &[u8]) -> Result<()> {
        let full = self.root.join(path);
        if let Some(dir) = full.parent() {
            std::fs::create_dir_all(dir).map_err(io(dir))?;
        }
        files::write_atomic(&full, bytes)
    }

    fn remove(&mut self, path: &str) -> Result<()> {
        remove_file(&self.root.join(path))
    }
}

fn remove_file(path: &Path) -> Result<()> {
    files::check_write(path)?;
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io(path)(e)),
    }
}

/// An open project folder: where it is, how it is laid out, and what each of its files held
/// when it was read or last saved.
#[derive(Clone, Debug)]
pub struct Folder {
    root: PathBuf,
    format: Format,
    schema: u32,
    index: BTreeMap<String, Known>,
}

impl Folder {
    /// Open the project in the folder `root`: its manifest is read and checked
    /// ([`read_manifest`]), nothing else. The app reads the rest through [`Source`].
    pub fn open(root: &Path, format: &Format) -> Result<Folder> {
        let mut folder = Folder { root: root.to_path_buf(), format: *format, schema: format.schema, index: BTreeMap::new() };
        let (_, schema) = read_manifest(format, &mut folder)?;
        folder.schema = schema;
        Ok(folder)
    }

    /// Make a new project in the folder `root`, which must not hold one yet, and save `files`
    /// into it.
    pub fn create(root: &Path, format: &Format, files: &Files) -> Result<Folder> {
        if root.join(format.manifest).exists() {
            return Err(Error::Invalid(format!("{}: a project already exists here", root.display())));
        }
        if !files.contains_key(format.manifest) {
            return Err(Error::Invalid(format!("the project has no {}", format.manifest)));
        }
        files::check_write(root)?;
        std::fs::create_dir_all(root).map_err(io(root))?;
        let mut folder = Folder { root: root.to_path_buf(), format: *format, schema: format.schema, index: BTreeMap::new() };
        folder.save(files)?;
        Ok(folder)
    }

    /// The folder's path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The folder's layout.
    pub fn format(&self) -> &Format {
        &self.format
    }

    /// The schema the manifest declared when the folder was opened (the format's own for a new
    /// project).
    pub fn schema(&self) -> u32 {
        self.schema
    }

    /// What is known about the file `path`, when it was read or saved.
    pub fn stamp(&self, path: &str) -> Option<Stamp> {
        self.index.get(path).map(|k| k.stamp)
    }

    /// The files read or saved, in name order.
    pub fn known(&self) -> impl Iterator<Item = &str> {
        self.index.keys().map(String::as_str)
    }

    /// Record what a file holds now, on disk and as the model renders it, after the app took
    /// an outside change in. `None` forgets the file, so the next save writes it.
    pub fn accept(&mut self, path: &str, stamp: Option<Stamp>) {
        match stamp {
            Some(stamp) => {
                self.index.insert(path.to_owned(), Known { stamp, held: true });
            }
            None => {
                self.index.remove(path);
            }
        }
    }

    /// Read every file under the folder `dir` of the project that the format allows, each
    /// within `limit` bytes: for a folder the app holds whole in memory (designs, small
    /// assets). The app holds them from then on: a later save removes one the app no longer
    /// renders.
    pub fn read_tree(&mut self, dir: &str, limit: u64) -> Result<Files> {
        let mut out = Files::new();
        for path in walk(&self.format, &self.root, dir)? {
            if let Some(bytes) = self.read(&path, limit)? {
                if let Some(known) = self.index.get_mut(&path) {
                    known.held = true;
                }
                out.insert(path, bytes);
            }
        }
        Ok(out)
    }

    /// The files changed, added or removed outside the app since they were read or last saved,
    /// among those `watched` says the app wants to hear about. A file is read to answer, within
    /// the format's entry limit, so `watched` leaves large assets out.
    pub fn outside_changes(&self, watched: &dyn Fn(&str) -> bool) -> Result<Vec<OutsideChange>> {
        let mut out = Vec::new();
        for (path, known) in &self.index {
            if !watched(path) {
                continue;
            }
            match read_file(&self.root, path, self.format.limits.entry)? {
                Some(bytes) if hash(&bytes) != known.stamp.disk => out.push(OutsideChange { path: path.clone(), now: Some(bytes) }),
                Some(_) => {}
                None => out.push(OutsideChange { path: path.clone(), now: None }),
            }
        }
        for path in walk(&self.format, &self.root, "")? {
            if !self.index.contains_key(&path)
                && watched(&path)
                && let Some(bytes) = read_file(&self.root, &path, self.format.limits.entry)?
            {
                out.push(OutsideChange { path, now: Some(bytes) });
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }

    /// Save `files`, the project as the app renders it now, into the folder.
    pub fn save(&mut self, files: &Files) -> Result<Saved> {
        let mut disk = Disk::new(&self.root);
        self.save_to(files, &mut disk)
    }

    /// [`Folder::save`] through a sink. A file is written when its bytes differ from what the
    /// model rendered it to when it was read or last saved; the files of [`Format::last`] come
    /// after every other. A file the app held (one a save wrote, or one it read and said what it
    /// renders to) and `files` no longer holds is removed; a file that was only read never is.
    /// On an error the files written so far stay written and are remembered, so the next save
    /// picks up where this one stopped.
    pub fn save_to(&mut self, files: &Files, sink: &mut dyn Sink) -> Result<Saved> {
        if !files.contains_key(self.format.manifest) {
            return Err(Error::Invalid(format!("the project has no {}", self.format.manifest)));
        }
        let mut saved = Saved::default();
        let now = SystemTime::now();
        let mut order: Vec<&String> = files.keys().collect();
        order.sort_by_key(|path| self.format.rank(path));
        for path in order {
            let Some(bytes) = files.get(path) else { continue };
            if !self.format.allows(path) {
                return Err(Error::Invalid(format!("{path} is not a file of this project")));
            }
            let rendered = hash(bytes);
            if let Some(known) = self.index.get_mut(path)
                && known.stamp.model == rendered
            {
                // Unchanged since it was read: the file stays as it is, and is the app's now.
                known.held = true;
                continue;
            }
            self.keep_conflict(path, Some(rendered), now, sink, &mut saved)?;
            sink.write(path, bytes)?;
            self.index.insert(path.clone(), Known { stamp: Stamp { disk: rendered, model: rendered }, held: true });
            saved.written.push(path.clone());
        }
        let gone: Vec<String> =
            self.index.iter().filter(|(path, known)| known.held && !files.contains_key(*path)).map(|(path, _)| path.clone()).collect();
        for path in gone {
            self.keep_conflict(&path, None, now, sink, &mut saved)?;
            sink.remove(&path)?;
            self.index.remove(&path);
            saved.removed.push(path);
        }
        Ok(saved)
    }

    /// Before `path` is written over (with bytes hashing to `writing`) or removed: when the
    /// file on disk is not what the folder last knew, keep a copy of it.
    fn keep_conflict(&self, path: &str, writing: Option<Hash>, at: SystemTime, sink: &mut dyn Sink, saved: &mut Saved) -> Result<()> {
        let Some(copy) = self.format.conflict_name(path, at) else { return Ok(()) };
        // A file that cannot be read now is not kept: the save is not stopped for it.
        let Ok(Some(theirs)) = read_file(&self.root, path, self.format.limits.entry) else { return Ok(()) };
        let on_disk = hash(&theirs);
        let known = self.index.get(path).map(|k| k.stamp.disk);
        if Some(on_disk) != writing && known != Some(on_disk) {
            sink.write(&copy, &theirs)?;
            saved.conflicts.push(copy);
        }
        Ok(())
    }

    /// Save with recovery around it: the unsaved work goes to the recovery copy first, the
    /// folder is saved, and the copy is removed only after the save succeeded. If the app stops
    /// part way, the next open finds whole files and the recovery copy.
    pub fn save_safely(&mut self, files: &Files) -> Result<Saved> {
        write_recovery(&self.format, &self.root, files)?;
        let saved = self.save(files)?;
        discard_recovery(&self.format, &self.root)?;
        Ok(saved)
    }
}

impl Source for Folder {
    fn read(&mut self, path: &str, limit: u64) -> Result<Option<Vec<u8>>> {
        if !self.format.allows(path) {
            return Err(Error::Invalid(format!("{path} is not a file of this project")));
        }
        let bytes = read_file(&self.root, path, limit)?;
        if let Some(bytes) = &bytes {
            let h = hash(bytes);
            let held = self.index.get(path).is_some_and(|k| k.held);
            self.index.insert(path.to_owned(), Known { stamp: Stamp { disk: h, model: h }, held });
        }
        Ok(bytes)
    }

    fn list(&mut self, dir: &str) -> Result<Vec<String>> {
        if !dir.is_empty() && !plain_path(dir, self.format.limits.name) {
            return Err(Error::Invalid(format!("{dir} is not a folder of this project")));
        }
        list_dir(&if dir.is_empty() { self.root.clone() } else { self.root.join(dir) })
    }

    fn rendered(&mut self, path: &str, bytes: &[u8]) {
        if let Some(known) = self.index.get_mut(path) {
            known.stamp.model = hash(bytes);
            known.held = true;
        }
    }
}

// ---- One file -----------------------------------------------------------------------------------

fn zip_write(e: impl std::fmt::Display) -> Error {
    Error::Invalid(format!("the project could not be packed: {e}"))
}

fn zip_read(e: impl std::fmt::Display) -> Error {
    Error::Damaged(format!("the packed project: {e}"))
}

/// Pack `files` into one file: a zip with a stored `mimetype` entry first, then every file in
/// name order with fixed times and permissions, so the same files always give the same bytes.
/// A path the format does not allow is an error.
pub fn pack(format: &Format, files: &Files) -> Result<Vec<u8>> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let base = SimpleFileOptions::default().last_modified_time(DateTime::default()).unix_permissions(0o644);
    zip.start_file(MIMETYPE_ENTRY, base.compression_method(CompressionMethod::Stored)).map_err(zip_write)?;
    zip.write_all(format.mimetype.as_bytes()).map_err(zip_write)?;
    for (path, bytes) in files {
        if !format.allows(path) {
            return Err(Error::Invalid(format!("{path} is not a file of this project")));
        }
        let method = if format.stored.iter().any(|end| path.ends_with(end)) { CompressionMethod::Stored } else { CompressionMethod::Deflated };
        let options = base.compression_method(method).large_file(bytes.len() as u64 >= u64::from(u32::MAX));
        zip.start_file(path.as_str(), options).map_err(zip_write)?;
        zip.write_all(bytes).map_err(zip_write)?;
    }
    zip.finish().map(Cursor::into_inner).map_err(zip_write)
}

/// Every file of the project folder `root` that belongs in a packed copy: the files the format
/// allows, without the recovery folder and without symbolic links, within the format's limits.
pub fn folder_files(format: &Format, root: &Path) -> Result<Files> {
    let mut out = Files::new();
    let mut total = 0u64;
    for path in walk(format, root, "")? {
        let Some(bytes) = read_file(root, &path, format.limits.entry)? else { continue };
        total += bytes.len() as u64;
        if total > format.limits.unpacked {
            return Err(too_large("the project folder", format.limits.unpacked));
        }
        out.insert(path, bytes);
    }
    Ok(out)
}

/// Pack the project folder `root` as it is on disk.
pub fn pack_folder(format: &Format, root: &Path) -> Result<Vec<u8>> {
    pack(format, &folder_files(format, root)?)
}

/// Read a packed project's files into memory. The file is not trusted: its size, the number of
/// entries, each entry's size (counted while reading, whatever its header says) and the total
/// are bounded by the format's limits; every name must be one the format allows; a name twice
/// is an error. Nothing is extracted to disk.
pub fn unpack(format: &Format, bytes: &[u8]) -> Result<Files> {
    let limits = &format.limits;
    if bytes.len() as u64 > limits.packed {
        return Err(too_large("the packed project", limits.packed));
    }
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(|_| Error::NotAProject)?;
    if zip.is_empty() {
        return Err(Error::NotAProject);
    }
    // The mimetype entry is not one of the project's files.
    if zip.len() > limits.entries.saturating_add(1) {
        return Err(Error::TooLarge(format!("the packed project holds more than {} files", limits.entries)));
    }
    let mut out = Files::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(zip_read)?;
        let name = entry.name().to_owned();
        if i == 0 {
            let mut said = Vec::new();
            entry.take(256).read_to_end(&mut said).map_err(zip_read)?;
            if name != MIMETYPE_ENTRY || said != format.mimetype.as_bytes() {
                return Err(Error::NotAProject);
            }
            continue;
        }
        if entry.is_dir() {
            continue;
        }
        if !format.allows(&name) {
            return Err(Error::Damaged(format!(
                "the packed project holds a file that is not the project's: {}",
                name.chars().take(80).collect::<String>()
            )));
        }
        let mut data = Vec::new();
        entry.take(limits.entry.saturating_add(1)).read_to_end(&mut data).map_err(zip_read)?;
        if data.len() as u64 > limits.entry {
            return Err(too_large(&name, limits.entry));
        }
        total += data.len() as u64;
        if total > limits.unpacked {
            return Err(too_large("the packed project, unpacked,", limits.unpacked));
        }
        if out.insert(name.clone(), data).is_some() {
            return Err(Error::Damaged(format!("the packed project holds {name} twice")));
        }
    }
    Ok(out)
}

/// Unpack a packed project into the folder `dest`, which must not exist or be empty. The whole
/// file is read and checked in memory first, then `check` reads the project the way the app
/// does (through [`Memory`]); only when it passes is anything written, the files of
/// [`Format::last`] after every other. Gives the folder, opened as [`Folder::open`] opens any
/// folder (the app reads it next), and the files written.
pub fn unpack_into(format: &Format, bytes: &[u8], dest: &Path, check: impl FnOnce(&Files) -> Result<()>) -> Result<(Folder, Files)> {
    let files = unpack(format, bytes)?;
    read_manifest(format, &mut Memory(&files))?;
    check(&files)?;
    if dest.exists() && std::fs::read_dir(dest).map(|mut d| d.next().is_some()).unwrap_or(true) {
        return Err(Error::Invalid(format!("{}: the folder is not empty", dest.display())));
    }
    files::check_write(dest)?;
    std::fs::create_dir_all(dest).map_err(io(dest))?;
    let mut disk = Disk::new(dest);
    let mut order: Vec<&String> = files.keys().collect();
    order.sort_by_key(|path| format.rank(path));
    for path in order {
        if let Some(bytes) = files.get(path) {
            disk.write(path, bytes)?;
        }
    }
    Ok((Folder::open(dest, format)?, files))
}

// ---- Recovery -----------------------------------------------------------------------------------

/// Where unsaved work is kept for the project folder `root`:
/// `.recovery/unsaved.<packed extension>` inside it.
pub fn recovery_path(format: &Format, root: &Path) -> PathBuf {
    root.join(RECOVERY_DIR).join(format!("unsaved.{}", format.packed_extension))
}

/// Keep `files`, the project's unsaved state as the app renders it, as a packed copy inside the
/// folder, without touching the project's own files.
pub fn write_recovery(format: &Format, root: &Path, files: &Files) -> Result<()> {
    let bytes = pack(format, files)?;
    let path = recovery_path(format, root);
    if let Some(dir) = path.parent() {
        files::check_write(dir)?;
        std::fs::create_dir_all(dir).map_err(io(dir))?;
    }
    files::write_atomic(&path, &bytes)
}

/// The unsaved work a run that did not end cleanly left in the project folder `root`, if there
/// is any and it reads. The app reads it through [`Memory`] and offers it.
pub fn recovered(format: &Format, root: &Path) -> Option<Files> {
    let path = recovery_path(format, root);
    let bytes = read_file(path.parent()?, path.file_name()?.to_str()?, format.limits.packed).ok()??;
    unpack(format, &bytes).ok()
}

/// Remove the recovery copy (after a good save, or when the user declines it), and the recovery
/// folder when nothing else is in it. None there is not an error.
pub fn discard_recovery(format: &Format, root: &Path) -> Result<()> {
    remove_file(&recovery_path(format, root))?;
    // Only an empty folder goes: `remove_dir` refuses any other.
    let _ = std::fs::remove_dir(root.join(RECOVERY_DIR));
    Ok(())
}

// ---- Backups ------------------------------------------------------------------------------------

fn backup_file(format: &Format, name: &str) -> PathBuf {
    PathBuf::from(format!("{name}.{}", format.packed_extension))
}

/// The backups of the project named `name` in `dir`, oldest first: the files
/// `<name> <YYYY-MM-DD HH.MM.SS> UTC.<packed extension>`, with a number after the time for a
/// second backup in the same second. A file with another name is not one, whatever it starts
/// with.
pub fn backups(format: &Format, dir: &Path, name: &str) -> Vec<PathBuf> {
    files::backups_of(dir, &backup_file(format, name))
}

/// Write `packed`, a packed copy of the project named `name`, into `dir` as a dated backup,
/// then remove the oldest backups of the same project beyond `keep`
/// (1 to [`files::MAX_BACKUPS`]). An existing file is never replaced. Gives the new backup's
/// path.
pub fn backup_packed(format: &Format, packed: &[u8], dir: &Path, name: &str, keep: usize, at: SystemTime) -> Result<PathBuf> {
    files::backup_bytes(packed, &backup_file(format, name), dir, keep, at)
}

/// Back the project folder `root` up as it is on disk: [`pack_folder`], then [`backup_packed`].
pub fn backup(format: &Format, root: &Path, dir: &Path, name: &str, keep: usize, at: SystemTime) -> Result<PathBuf> {
    backup_packed(format, &pack_folder(format, root)?, dir, name, keep, at)
}

#[cfg(test)]
mod tests;
