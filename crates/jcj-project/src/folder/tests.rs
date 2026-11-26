// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::{Duration, UNIX_EPOCH};

use super::*;

/// A layout with an index beside its manifest, text the app parses, media it only carries, and
/// conflict copies: the manifest and the index are written last.
const DRAFT: Format = Format {
    last: &["project.toml", "index.toml"],
    belongs: draft_belongs,
    conflicts: Some("conflicts"),
    ..Format::new("project.toml", 3, "application/x-example-draft-packed", "draftz")
};

fn draft_belongs(path: &str) -> bool {
    path == "index.toml" || ["text/", "notes/", "media/", "conflicts/"].iter().any(|dir| path.starts_with(dir))
}

/// A layout the app holds whole in memory: designs and pictures, pictures stored as they are,
/// no hidden files.
const ALBUM: Format =
    Format { belongs: album_belongs, stored: &[".png", ".jpg"], ..Format::new("album.toml", 1, "application/x-example-album-packed", "albumz") };

fn album_belongs(path: &str) -> bool {
    !path.split('/').any(|part| part.starts_with('.')) && !path.chars().any(char::is_control)
}

fn temp_dir(name: &str) -> PathBuf {
    let n = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("jcj-project-folder-{name}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn files(list: &[(&str, &str)]) -> Files {
    list.iter().map(|(path, text)| ((*path).to_owned(), text.as_bytes().to_vec())).collect()
}

fn draft() -> Files {
    files(&[
        ("project.toml", "schema = 3\ntitle = \"A Draft\"\n"),
        ("index.toml", "schema = 3\nitems = [\"one\", \"two\"]\n"),
        ("text/one.md", "The first chapter.\n"),
        ("text/two.md", "The second chapter.\n"),
        ("notes/people.md", "Who is who.\n"),
    ])
}

fn on_disk(root: &Path, path: &str) -> Option<String> {
    std::fs::read_to_string(root.join(path)).ok()
}

fn put(root: &Path, path: &str, text: &str) {
    let full = root.join(path);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(full, text).unwrap();
}

// ---- Names ----------------------------------------------------------------------------------

#[test]
fn a_plain_path_stays_inside_the_folder_on_every_system() {
    for good in ["a.txt", "text/one.md", "a/b/c/d.bin", "with space/é.md", ".hidden/x"] {
        assert!(plain_path(good, 512), "{good}");
    }
    for bad in ["", "/abs", "a//b", "a/", "../up", "a/../b", "./a", "a\\b", "C:/x", "nul\0", "a/."] {
        assert!(!plain_path(bad, 512), "{bad:?}");
    }
    assert!(plain_path("abcd", 4) && !plain_path("abcde", 4));
}

#[test]
fn a_format_allows_its_own_files_and_never_the_recovery_folder() {
    for good in ["project.toml", "index.toml", "text/one.md", "media/cover/front.png", "conflicts/2026 text-one.md"] {
        assert!(DRAFT.allows(good), "{good}");
    }
    for bad in ["readme.txt", "other/one.md", "text/../project.toml", ".recovery/unsaved.draftz", "mimetype", "text\\one.md"] {
        assert!(!DRAFT.allows(bad), "{bad}");
    }
    // A format where every plain path belongs still keeps the two names the layer owns.
    let open = Format::new("m.toml", 1, "application/x-example", "mz");
    assert!(open.allows("anything/at/all.bin") && open.allows("m.toml"));
    assert!(!open.allows(".recovery/unsaved.mz") && !open.allows("mimetype"));
    assert!(any_path("x"));
    assert!(ALBUM.allows("assets/cover.png") && !ALBUM.allows("assets/.DS_Store") && !ALBUM.allows("assets/a\u{7}.png"));
}

// ---- The manifest ---------------------------------------------------------------------------

#[test]
fn the_manifest_says_the_schema() {
    assert_eq!(schema_of("schema = 3\ntitle = \"x\"\n").unwrap(), 3);
    assert_eq!(schema_of("# A comment first.\nschema = 0\n").unwrap(), 0);
    assert!(matches!(schema_of("title = \"x\"\n"), Err(Error::NotAProject)));
    assert!(matches!(schema_of("schema = \"3\"\n"), Err(Error::NotAProject)));
    assert!(matches!(schema_of("schema = -1\n"), Err(Error::Damaged(_))));
    assert!(matches!(schema_of("schema = 99999999999\n"), Err(Error::Damaged(_))));
    assert!(matches!(schema_of("schema = [\n"), Err(Error::Damaged(_))));
}

#[test]
fn a_manifest_that_nests_too_deeply_is_refused_before_it_is_parsed() {
    assert_eq!(toml_nesting("a = [[1, 2], {b = [3]}]"), 3);
    assert_eq!(toml_nesting("a = \"[[[[\" # [[[[\nb = '{{{'\n[table]\n"), 1);
    assert_eq!(toml_nesting("a = \"an \\\" escaped [ quote\"\n"), 0);
    let deep = format!("schema = 1\na = {}{}\n", "[".repeat(MAX_MANIFEST_NESTING + 1), "]".repeat(MAX_MANIFEST_NESTING + 1));
    assert!(matches!(schema_of(&deep), Err(Error::Damaged(why)) if why.contains("nests deeper")));
}

// ---- Opening and reading --------------------------------------------------------------------

#[test]
fn a_project_is_created_opened_and_read() {
    let root = temp_dir("open").join("A Draft");
    let created = Folder::create(&root, &DRAFT, &draft()).unwrap();
    assert_eq!((created.root(), created.schema(), created.format().manifest), (root.as_path(), 3, "project.toml"));
    assert_eq!(on_disk(&root, "text/one.md").as_deref(), Some("The first chapter.\n"));

    let mut folder = Folder::open(&root, &DRAFT).unwrap();
    assert_eq!(folder.schema(), 3);
    assert_eq!(folder.known().collect::<Vec<_>>(), ["project.toml"], "only the manifest is read to open");
    assert_eq!(folder.read_text("text/one.md", 1 << 10).unwrap().as_deref(), Some("The first chapter.\n"));
    assert_eq!(folder.read("text/missing.md", 1 << 10).unwrap(), None);
    assert_eq!(folder.list("text").unwrap(), ["one.md", "two.md"]);
    assert_eq!(folder.list("").unwrap(), ["index.toml", "project.toml"]);
    assert!(folder.list("nowhere").unwrap().is_empty());
    assert_eq!(folder.stamp("text/one.md"), Some(Stamp::of(b"The first chapter.\n", b"The first chapter.\n")));
    assert_eq!(folder.stamp("text/two.md"), None);

    // Limits and names hold for every read.
    assert!(matches!(folder.read("text/one.md", 4), Err(Error::TooLarge(why)) if why.contains("text/one.md")));
    assert!(matches!(folder.read("readme.txt", 1 << 10), Err(Error::Invalid(_))));
    assert!(matches!(folder.read("../outside", 1 << 10), Err(Error::Invalid(_))));
    assert!(matches!(folder.list("../outside"), Err(Error::Invalid(_))));
    put(&root, "text/bad.md", "");
    std::fs::write(root.join("text/bad.md"), [0xFF, 0xFE]).unwrap();
    assert!(matches!(folder.read_text("text/bad.md", 1 << 10), Err(Error::Damaged(why)) if why.contains("UTF-8")));

    // A second project is not made over the first.
    assert!(matches!(Folder::create(&root, &DRAFT, &draft()), Err(Error::Invalid(why)) if why.contains("already exists")));
}

#[test]
fn what_is_not_a_project_or_is_too_new_is_refused() {
    let dir = temp_dir("refuse");
    assert!(matches!(Folder::open(&dir.join("missing"), &DRAFT), Err(Error::NotAProject)));
    put(&dir, "empty/readme.txt", "no manifest here");
    assert!(matches!(Folder::open(&dir.join("empty"), &DRAFT), Err(Error::NotAProject)));
    put(&dir, "newer/project.toml", "schema = 4\n");
    assert!(matches!(Folder::open(&dir.join("newer"), &DRAFT), Err(Error::NewerApp { found: 4, supported: 3 })));
    put(&dir, "older/project.toml", "schema = 1\n");
    assert_eq!(Folder::open(&dir.join("older"), &DRAFT).unwrap().schema(), 1, "an older project is the app's to migrate");
    put(&dir, "damaged/project.toml", "schema = = 3\n");
    assert!(matches!(Folder::open(&dir.join("damaged"), &DRAFT), Err(Error::Damaged(_))));
    let small = Format { limits: Limits { manifest: 8, ..Limits::DEFAULT }, ..DRAFT };
    put(&dir, "large/project.toml", "schema = 3\ntitle = \"long\"\n");
    assert!(matches!(Folder::open(&dir.join("large"), &small), Err(Error::TooLarge(_))));
    // A project needs its manifest to be made or saved.
    let mut no_manifest = draft();
    no_manifest.remove("project.toml");
    assert!(matches!(Folder::create(&dir.join("new"), &DRAFT, &no_manifest), Err(Error::Invalid(_))));
    assert!(!dir.join("new").join("index.toml").exists());
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_is_never_followed() {
    let dir = temp_dir("links");
    let root = dir.join("project");
    Folder::create(&root, &DRAFT, &draft()).unwrap();
    put(&dir, "outside.md", "Not the project's.\n");
    std::os::unix::fs::symlink(dir.join("outside.md"), root.join("text/link.md")).unwrap();
    std::os::unix::fs::symlink(&dir, root.join("media")).unwrap();
    let mut folder = Folder::open(&root, &DRAFT).unwrap();
    assert_eq!(folder.read("text/link.md", 1 << 10).unwrap(), None);
    assert_eq!(folder.list("text").unwrap(), ["one.md", "two.md"]);
    let packed = folder_files(&DRAFT, &root).unwrap();
    assert_eq!(packed, draft(), "neither the linked file nor the linked folder is packed");
}

#[test]
fn memory_is_a_source_like_a_folder() {
    let all = draft();
    let mut memory = Memory(&all);
    assert_eq!(memory.read_text("text/one.md", 1 << 10).unwrap().as_deref(), Some("The first chapter.\n"));
    assert_eq!(memory.read("text/none.md", 1 << 10).unwrap(), None);
    assert!(matches!(memory.read("text/one.md", 4), Err(Error::TooLarge(_))));
    assert_eq!(memory.list("text").unwrap(), ["one.md", "two.md"]);
    assert_eq!(memory.list("").unwrap(), ["index.toml", "project.toml"]);
    assert!(memory.list("media").unwrap().is_empty());
    memory.rendered("text/one.md", b"ignored");
    assert_eq!(read_manifest(&DRAFT, &mut memory).unwrap(), ("schema = 3\ntitle = \"A Draft\"\n".to_owned(), 3));
    assert!(matches!(read_manifest(&ALBUM, &mut memory), Err(Error::NotAProject)));
}

// ---- Saving ---------------------------------------------------------------------------------

#[test]
fn a_save_writes_only_what_changed_with_the_manifest_last() {
    let root = temp_dir("save").join("p");
    let mut now = draft();
    let mut folder = Folder::create(&root, &DRAFT, &now).unwrap();
    assert_eq!(folder.save(&now).unwrap(), Saved::default(), "nothing changed, nothing written");

    now.insert("text/three.md".into(), b"A third.\n".to_vec());
    now.insert("index.toml".into(), b"schema = 3\nitems = [\"one\", \"two\", \"three\"]\n".to_vec());
    now.insert("project.toml".into(), b"schema = 3\ntitle = \"Renamed\"\n".to_vec());
    now.insert("notes/people.md".into(), b"Who is who, revised.\n".to_vec());
    let saved = folder.save(&now).unwrap();
    assert_eq!(saved.written, ["notes/people.md", "text/three.md", "project.toml", "index.toml"]);
    assert!(saved.removed.is_empty() && saved.conflicts.is_empty());

    // A file the app no longer renders is removed; the manifest and its companions never are.
    now.remove("text/two.md");
    let saved = folder.save(&now).unwrap();
    assert_eq!((saved.written.len(), saved.removed), (0, vec!["text/two.md".to_owned()]));
    assert!(!root.join("text/two.md").exists());

    // A path that is not the project's is refused, and nothing is written for it.
    now.insert("readme.txt".into(), b"x".to_vec());
    assert!(matches!(folder.save(&now), Err(Error::Invalid(why)) if why.contains("readme.txt")));
    assert!(!root.join("readme.txt").exists());
}

#[test]
fn the_format_s_default_is_the_manifest_alone_last() {
    let root = temp_dir("album").join("a");
    let album = files(&[("album.toml", "schema = 1\n"), ("designs/plain.toml", "name = \"Plain\"\n"), ("zebra/z.txt", "z")]);
    let mut memory = Vec::new();
    struct Record<'a>(&'a mut Vec<String>);
    impl Sink for Record<'_> {
        fn write(&mut self, path: &str, _bytes: &[u8]) -> Result<()> {
            self.0.push(path.to_owned());
            Ok(())
        }
        fn remove(&mut self, _path: &str) -> Result<()> {
            Ok(())
        }
    }
    std::fs::create_dir_all(&root).unwrap();
    put(&root, "album.toml", "schema = 1\nold = true\n");
    let mut folder = Folder::open(&root, &ALBUM).unwrap();
    folder.save_to(&album, &mut Record(&mut memory)).unwrap();
    assert_eq!(memory, ["designs/plain.toml", "zebra/z.txt", "album.toml"]);
}

#[test]
fn a_file_the_app_only_read_or_never_saw_is_left_alone() {
    let root = temp_dir("left").join("p");
    Folder::create(&root, &DRAFT, &draft()).unwrap();
    put(&root, "media/cover.png", "a picture");
    put(&root, "text/orphan.md", "A file the index does not name.\n");
    put(&root, "readme.txt", "The user's own file.\n");

    let mut folder = Folder::open(&root, &DRAFT).unwrap();
    // The app reads its text, saying what each renders to, and looks at a picture.
    let mut model = Files::new();
    for path in ["project.toml", "index.toml", "text/one.md", "text/two.md", "notes/people.md"] {
        let bytes = folder.read(path, 1 << 20).unwrap().unwrap();
        folder.rendered(path, &bytes);
        model.insert(path.to_owned(), bytes);
    }
    assert_eq!(folder.read("media/cover.png", 1 << 20).unwrap().as_deref(), Some(b"a picture".as_slice()));

    model.remove("text/two.md");
    let saved = folder.save(&model).unwrap();
    assert_eq!(saved.removed, ["text/two.md"], "only a file the app held is removed");
    for kept in ["media/cover.png", "text/orphan.md", "readme.txt"] {
        assert!(root.join(kept).exists(), "{kept}");
    }
}

#[test]
fn a_folder_read_whole_is_held_by_the_app() {
    let root = temp_dir("tree").join("a");
    let album = files(&[("album.toml", "schema = 1\n"), ("designs/plain.toml", "a"), ("designs/more/fine.toml", "b"), ("assets/cover.png", "c")]);
    Folder::create(&root, &ALBUM, &album).unwrap();
    put(&root, "designs/.hidden", "not the project's");

    let mut folder = Folder::open(&root, &ALBUM).unwrap();
    let designs = folder.read_tree("designs", 1 << 20).unwrap();
    assert_eq!(designs.keys().collect::<Vec<_>>(), ["designs/more/fine.toml", "designs/plain.toml"]);
    assert!(matches!(folder.read_tree("assets", 0), Err(Error::TooLarge(_))));
    assert!(folder.read_tree("missing", 1 << 20).unwrap().is_empty());

    // The app drops a design: the save removes its file, and leaves the assets it never read.
    let mut now = files(&[("album.toml", "schema = 1\n")]);
    now.insert("designs/plain.toml".into(), b"a".to_vec());
    assert_eq!(folder.save(&now).unwrap().removed, ["designs/more/fine.toml"]);
    assert!(root.join("assets/cover.png").exists() && root.join("designs/.hidden").exists());
}

#[test]
fn text_written_by_hand_is_not_rewritten_until_the_model_changes() {
    let root = temp_dir("hand").join("p");
    Folder::create(&root, &DRAFT, &draft()).unwrap();
    // Written by hand: the app would write it with one space, not two.
    put(&root, "text/one.md", "The  first chapter.\n");

    let mut folder = Folder::open(&root, &DRAFT).unwrap();
    let mut model = Files::new();
    for path in ["project.toml", "index.toml", "text/one.md", "text/two.md", "notes/people.md"] {
        let bytes = folder.read(path, 1 << 20).unwrap().unwrap();
        // What the app's writer makes of what it parsed.
        let rendered = String::from_utf8(bytes).unwrap().replace("  ", " ").into_bytes();
        folder.rendered(path, &rendered);
        model.insert(path.to_owned(), rendered);
    }
    assert_eq!(folder.stamp("text/one.md"), Some(Stamp::of(b"The  first chapter.\n", b"The first chapter.\n")));
    assert_eq!(folder.save(&model).unwrap(), Saved::default());
    assert_eq!(on_disk(&root, "text/one.md").as_deref(), Some("The  first chapter.\n"), "byte for byte as it was");

    model.insert("text/one.md".into(), b"The first chapter, revised.\n".to_vec());
    let saved = folder.save(&model).unwrap();
    assert_eq!((saved.written, saved.conflicts.len()), (vec!["text/one.md".to_owned()], 0));
    assert_eq!(on_disk(&root, "text/one.md").as_deref(), Some("The first chapter, revised.\n"));
}

/// A sink that writes to disk until it has written `left` files, then fails.
struct Failing {
    disk: Disk,
    left: usize,
}

impl Sink for Failing {
    fn write(&mut self, path: &str, bytes: &[u8]) -> Result<()> {
        if self.left == 0 {
            return Err(Error::Invalid(format!("{path}: the disk is full")));
        }
        self.left -= 1;
        self.disk.write(path, bytes)
    }

    fn remove(&mut self, path: &str) -> Result<()> {
        self.disk.remove(path)
    }
}

#[test]
fn an_interrupted_save_leaves_the_old_manifest_and_the_next_save_finishes() {
    let root = temp_dir("interrupted").join("p");
    let mut folder = Folder::create(&root, &DRAFT, &draft()).unwrap();
    let mut now = draft();
    now.insert("text/one.md".into(), b"One, revised.\n".to_vec());
    now.insert("text/three.md".into(), b"Three.\n".to_vec());
    now.insert("index.toml".into(), b"schema = 3\nitems = [\"one\", \"two\", \"three\"]\n".to_vec());

    let mut failing = Failing { disk: Disk::new(&root), left: 2 };
    assert!(folder.save_to(&now, &mut failing).is_err());
    // The two text files are whole and new; the index still names what it named.
    assert_eq!(on_disk(&root, "text/one.md").as_deref(), Some("One, revised.\n"));
    assert_eq!(on_disk(&root, "text/three.md").as_deref(), Some("Three.\n"));
    assert_eq!(on_disk(&root, "index.toml").as_deref(), Some("schema = 3\nitems = [\"one\", \"two\"]\n"));

    // The next save writes only what the first did not reach.
    assert_eq!(folder.save(&now).unwrap().written, ["index.toml"]);
    let mut reopened = Folder::open(&root, &DRAFT).unwrap();
    assert_eq!(reopened.read_text("index.toml", 1 << 10).unwrap().as_deref(), Some("schema = 3\nitems = [\"one\", \"two\", \"three\"]\n"));
}

// ---- Changes made elsewhere -----------------------------------------------------------------

fn text_only(path: &str) -> bool {
    path.ends_with(".md") || path.ends_with(".toml")
}

#[test]
fn changes_made_elsewhere_are_found_and_taken_in() {
    let root = temp_dir("outside").join("p");
    let mut folder = Folder::create(&root, &DRAFT, &draft()).unwrap();
    assert!(folder.outside_changes(&text_only).unwrap().is_empty());

    put(&root, "text/one.md", "Changed on another device.\n");
    std::fs::remove_file(root.join("text/two.md")).unwrap();
    put(&root, "text/new.md", "Added elsewhere.\n");
    put(&root, "media/cover.png", "not watched");
    put(&root, "readme.txt", "not the project's");
    put(&root, ".recovery/unsaved.draftz", "not a project file");

    let changes = folder.outside_changes(&text_only).unwrap();
    assert_eq!(
        changes,
        [
            OutsideChange { path: "text/new.md".into(), now: Some(b"Added elsewhere.\n".to_vec()) },
            OutsideChange { path: "text/one.md".into(), now: Some(b"Changed on another device.\n".to_vec()) },
            OutsideChange { path: "text/two.md".into(), now: None },
        ]
    );
    assert!(folder.outside_changes(&|path| path == "index.toml").unwrap().is_empty());

    // The app takes each change in and says so; the folder then has nothing more to report.
    for change in changes {
        folder.accept(&change.path, change.now.as_deref().map(|bytes| Stamp::of(bytes, bytes)));
    }
    assert!(folder.outside_changes(&text_only).unwrap().is_empty());
    assert_eq!(folder.stamp("text/two.md"), None);
}

#[test]
fn a_save_keeps_a_copy_of_what_it_would_write_over() {
    let root = temp_dir("conflict").join("p");
    let mut folder = Folder::create(&root, &DRAFT, &draft()).unwrap();
    put(&root, "text/one.md", "Theirs, written elsewhere.\n");
    put(&root, "text/two.md", "Theirs too, and ours is going away.\n");

    let mut now = draft();
    now.insert("text/one.md".into(), b"Ours.\n".to_vec());
    now.remove("text/two.md");
    let saved = folder.save(&now).unwrap();
    assert_eq!(saved.written, ["text/one.md"]);
    assert_eq!(saved.removed, ["text/two.md"]);
    assert_eq!(saved.conflicts.len(), 2);
    assert!(saved.conflicts[0].starts_with("conflicts/") && saved.conflicts[0].ends_with(" UTC text-one.md"), "{}", saved.conflicts[0]);
    assert_eq!(on_disk(&root, &saved.conflicts[0]).as_deref(), Some("Theirs, written elsewhere.\n"));
    assert_eq!(on_disk(&root, &saved.conflicts[1]).as_deref(), Some("Theirs too, and ours is going away.\n"));
    assert_eq!(on_disk(&root, "text/one.md").as_deref(), Some("Ours.\n"));

    // A change that is what the save writes anyway is no conflict.
    put(&root, "text/one.md", "Both wrote this.\n");
    now.insert("text/one.md".into(), b"Both wrote this.\n".to_vec());
    assert!(folder.save(&now).unwrap().conflicts.is_empty());

    // The copies are the project's files: packed with it, and never removed by a save.
    assert!(folder_files(&DRAFT, &root).unwrap().keys().any(|k| k.starts_with("conflicts/")));
    assert_eq!(folder.save(&now).unwrap(), Saved::default());
    assert_eq!(std::fs::read_dir(root.join("conflicts")).unwrap().count(), 2);
}

#[test]
fn a_conflict_copy_is_named_with_the_format_s_time() {
    fn day(_at: SystemTime) -> String {
        "20261119T140322Z".to_owned()
    }
    let own = Format { conflict_stamp: day, ..DRAFT };
    assert_eq!(own.conflict_name("text/a/b.md", UNIX_EPOCH).as_deref(), Some("conflicts/20261119T140322Z text-a-b.md"));
    assert_eq!(DRAFT.conflict_name("text/one.md", UNIX_EPOCH).as_deref(), Some("conflicts/1970-01-01 00.00.00 UTC text-one.md"));
    assert_eq!(ALBUM.conflict_name("designs/plain.toml", UNIX_EPOCH), None);
}

#[test]
fn a_format_without_conflict_copies_writes_over() {
    let root = temp_dir("no-conflict").join("a");
    let mut album = files(&[("album.toml", "schema = 1\n"), ("designs/plain.toml", "ours\n")]);
    let mut folder = Folder::create(&root, &ALBUM, &album).unwrap();
    put(&root, "designs/plain.toml", "theirs\n");
    album.insert("designs/plain.toml".into(), b"ours, revised\n".to_vec());
    assert_eq!(folder.save(&album).unwrap(), Saved { written: vec!["designs/plain.toml".into()], ..Saved::default() });
    assert!(!root.join("conflicts").exists());
}

// ---- One file -------------------------------------------------------------------------------

#[test]
fn packing_gives_the_same_bytes_for_the_same_files() {
    let album = files(&[("album.toml", "schema = 1\n"), ("designs/plain.toml", "name = \"Plain\"\n"), ("assets/cover.png", "pretend picture bytes")]);
    let packed = pack(&ALBUM, &album).unwrap();
    assert_eq!(pack(&ALBUM, &album).unwrap(), packed);
    assert_eq!(unpack(&ALBUM, &packed).unwrap(), album);

    // The shape other tools can rely on: `mimetype` first and stored, then the files in name
    // order with a fixed time; a file that is compressed already is stored as it is.
    let mut zip = ZipArchive::new(Cursor::new(packed.as_slice())).unwrap();
    let names: Vec<String> = (0..zip.len()).map(|i| zip.by_index(i).unwrap().name().to_owned()).collect();
    assert_eq!(names, ["mimetype", "album.toml", "assets/cover.png", "designs/plain.toml"]);
    for (name, method) in
        [("mimetype", CompressionMethod::Stored), ("assets/cover.png", CompressionMethod::Stored), ("album.toml", CompressionMethod::Deflated)]
    {
        let entry = zip.by_name(name).unwrap();
        assert_eq!(entry.compression(), method, "{name}");
        assert_eq!(entry.last_modified(), Some(DateTime::default()), "{name}");
        assert_eq!(entry.unix_mode().map(|m| m & 0o777), Some(0o644), "{name}");
    }
    assert!(packed.starts_with(b"PK"));
    assert!(packed.windows(ALBUM.mimetype.len()).any(|w| w == ALBUM.mimetype.as_bytes()), "the mimetype is readable in the file");

    assert!(matches!(pack(&ALBUM, &files(&[("album.toml", "schema = 1\n"), ("assets/.hidden", "x")])), Err(Error::Invalid(_))));
}

#[test]
fn a_folder_is_packed_without_what_is_not_the_project_s() {
    let root = temp_dir("pack-folder").join("p");
    Folder::create(&root, &DRAFT, &draft()).unwrap();
    put(&root, "media/cover.png", "a picture");
    put(&root, "readme.txt", "the user's");
    put(&root, ".recovery/unsaved.draftz", "unsaved work");
    put(&root, ".git/config", "version control");
    let mut expected = draft();
    expected.insert("media/cover.png".into(), b"a picture".to_vec());
    assert_eq!(folder_files(&DRAFT, &root).unwrap(), expected);
    assert_eq!(pack_folder(&DRAFT, &root).unwrap(), pack(&DRAFT, &expected).unwrap());

    let tight = Format { limits: Limits { unpacked: 16, ..Limits::DEFAULT }, ..DRAFT };
    assert!(matches!(folder_files(&tight, &root), Err(Error::TooLarge(_))));
    let few = Format { limits: Limits { entries: 2, ..Limits::DEFAULT }, ..DRAFT };
    assert!(matches!(folder_files(&few, &root), Err(Error::TooLarge(_))));
}

/// A zip with these entries, as given (no checks): for files this layer would never write.
fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        zip.start_file(*name, SimpleFileOptions::default().compression_method(CompressionMethod::Stored)).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

#[test]
fn a_packed_project_is_never_trusted() {
    let mime = DRAFT.mimetype.as_bytes();
    let manifest: &[u8] = b"schema = 3\n";
    assert!(matches!(unpack(&DRAFT, b"not a zip at all"), Err(Error::NotAProject)));
    assert!(matches!(unpack(&DRAFT, &zip_of(&[])), Err(Error::NotAProject)));
    assert!(matches!(unpack(&DRAFT, &zip_of(&[("project.toml", manifest)])), Err(Error::NotAProject)), "no mimetype first");
    assert!(matches!(unpack(&DRAFT, &zip_of(&[("mimetype", b"application/zip"), ("project.toml", manifest)])), Err(Error::NotAProject)));
    assert!(matches!(unpack(&ALBUM, &pack(&DRAFT, &draft()).unwrap()), Err(Error::NotAProject)), "another app's packed project");

    for outside in ["../escape.md", "/abs.md", "text\\one.md", "readme.txt", ".recovery/unsaved.draftz"] {
        let zip = zip_of(&[("mimetype", mime), ("project.toml", manifest), (outside, b"x")]);
        assert!(matches!(unpack(&DRAFT, &zip), Err(Error::Damaged(_))), "{outside}");
    }
    // A folder entry is passed over; the files are read.
    let ok = zip_of(&[("mimetype", mime), ("project.toml", manifest), ("text/one.md", b"One.\n")]);
    assert_eq!(unpack(&DRAFT, &ok).unwrap().len(), 2);

    let with = |limits: Limits| Format { limits, ..DRAFT };
    assert!(matches!(unpack(&with(Limits { packed: 16, ..Limits::DEFAULT }), &ok), Err(Error::TooLarge(_))));
    assert!(matches!(unpack(&with(Limits { entries: 1, ..Limits::DEFAULT }), &ok), Err(Error::TooLarge(_))));
    assert!(matches!(unpack(&with(Limits { entry: 4, ..Limits::DEFAULT }), &ok), Err(Error::TooLarge(why)) if why.contains("project.toml")));
    assert!(matches!(unpack(&with(Limits { unpacked: 12, ..Limits::DEFAULT }), &ok), Err(Error::TooLarge(_))));
    assert!(unpack(&with(Limits { entries: 2, entry: 11, unpacked: 16, ..Limits::DEFAULT }), &ok).is_ok(), "exactly at the limits");

    // Cut short or with a byte changed, it is an error, never a panic.
    let whole = pack(&DRAFT, &draft()).unwrap();
    for cut in [1, whole.len() / 2, whole.len() - 1] {
        assert!(unpack(&DRAFT, &whole[..cut]).is_err());
    }
    for at in (0..whole.len()).step_by(7) {
        let mut damaged = whole.clone();
        damaged[at] ^= 0x55;
        let _ = unpack(&DRAFT, &damaged);
    }
}

#[test]
fn a_packed_project_is_unpacked_into_a_new_folder_only_when_it_reads() {
    let dir = temp_dir("unpack");
    let packed = pack(&DRAFT, &draft()).unwrap();
    let (mut folder, written) = unpack_into(&DRAFT, &packed, &dir.join("fresh"), |_| Ok(())).unwrap();
    assert_eq!(written, draft());
    assert_eq!(folder.known().collect::<Vec<_>>(), ["project.toml"], "opened like any folder: the app reads it next");
    for path in draft().keys() {
        let bytes = folder.read(path, 1 << 10).unwrap().unwrap();
        folder.rendered(path, &bytes);
    }
    assert_eq!(folder.save(&draft()).unwrap(), Saved::default(), "what was unpacked is what the app renders");

    // An existing empty folder will do; one with anything in it will not.
    std::fs::create_dir_all(dir.join("empty")).unwrap();
    assert!(unpack_into(&DRAFT, &packed, &dir.join("empty"), |_| Ok(())).is_ok());
    assert!(matches!(unpack_into(&DRAFT, &packed, &dir.join("fresh"), |_| Ok(())), Err(Error::Invalid(why)) if why.contains("not empty")));

    // The app reads the project first; when it does not read, nothing is written.
    let refused = unpack_into(&DRAFT, &packed, &dir.join("refused"), |files| {
        assert!(files.contains_key("index.toml"));
        Err(Error::Damaged("index.toml names a file that is missing".into()))
    });
    assert!(matches!(refused, Err(Error::Damaged(_))));
    assert!(!dir.join("refused").exists());

    let mut newer = draft();
    newer.insert("project.toml".into(), b"schema = 9\n".to_vec());
    let newer = pack(&DRAFT, &newer).unwrap();
    assert!(matches!(unpack_into(&DRAFT, &newer, &dir.join("newer"), |_| Ok(())), Err(Error::NewerApp { found: 9, supported: 3 })));
    assert!(!dir.join("newer").exists());
}

// ---- Recovery -------------------------------------------------------------------------------

#[test]
fn unsaved_work_is_kept_inside_the_folder_and_found_after_a_crash() {
    let root = temp_dir("recovery").join("p");
    let mut folder = Folder::create(&root, &DRAFT, &draft()).unwrap();
    assert_eq!(recovery_path(&DRAFT, &root), root.join(".recovery").join("unsaved.draftz"));
    assert_eq!(recovered(&DRAFT, &root), None);

    let mut unsaved = draft();
    unsaved.insert("text/one.md".into(), b"Typed and not saved yet.\n".to_vec());
    write_recovery(&DRAFT, &root, &unsaved).unwrap();
    assert_eq!(on_disk(&root, "text/one.md").as_deref(), Some("The first chapter.\n"), "the project's own files are not touched");
    assert_eq!(recovered(&DRAFT, &root), Some(unsaved.clone()));
    assert_eq!(folder_files(&DRAFT, &root).unwrap(), draft(), "the recovery copy is never packed");

    // The app stops part way through a save: whole files, and the work still recoverable.
    write_recovery(&DRAFT, &root, &unsaved).unwrap();
    let mut failing = Failing { disk: Disk::new(&root), left: 0 };
    assert!(folder.save_to(&unsaved, &mut failing).is_err());
    assert_eq!(recovered(&DRAFT, &root), Some(unsaved.clone()));
    let found = recovered(&DRAFT, &root).unwrap();
    assert_eq!(Memory(&found).read_text("text/one.md", 1 << 10).unwrap().as_deref(), Some("Typed and not saved yet.\n"));

    // A save with recovery around it leaves none behind.
    assert_eq!(folder.save_safely(&unsaved).unwrap().written, ["text/one.md"]);
    assert_eq!(recovered(&DRAFT, &root), None);
    assert!(!root.join(".recovery").exists(), "the empty recovery folder goes too");
    discard_recovery(&DRAFT, &root).unwrap();

    // A recovery copy that does not read is no recovery, and something else in the folder stays.
    put(&root, ".recovery/unsaved.draftz", "not a packed project");
    put(&root, ".recovery/keep.txt", "something else");
    assert_eq!(recovered(&DRAFT, &root), None);
    discard_recovery(&DRAFT, &root).unwrap();
    assert!(!recovery_path(&DRAFT, &root).exists() && root.join(".recovery/keep.txt").exists());
}

// ---- Backups --------------------------------------------------------------------------------

#[test]
fn backups_are_dated_packed_copies_and_the_oldest_go() {
    let dir = temp_dir("backups");
    let (root, backups_dir) = (dir.join("A Draft"), dir.join("Backups"));
    Folder::create(&root, &DRAFT, &draft()).unwrap();
    let t0 = UNIX_EPOCH + Duration::from_secs(1_795_000_000);

    let first = backup(&DRAFT, &root, &backups_dir, "A Draft", 3, t0).unwrap();
    let name = first.file_name().unwrap().to_str().unwrap().to_owned();
    assert_eq!(name, format!("A Draft {}.draftz", files::stamp(t0)));
    assert_eq!(unpack(&DRAFT, &std::fs::read(&first).unwrap()).unwrap(), draft(), "a backup opens like any packed project");

    // A second backup in the same second gets a number; nothing is replaced.
    let second = backup(&DRAFT, &root, &backups_dir, "A Draft", 3, t0).unwrap();
    assert_eq!(second.file_name().unwrap().to_str().unwrap(), format!("A Draft {} 2.draftz", files::stamp(t0)));

    // Files that only look alike are not this project's backups, and are never removed.
    put(&backups_dir, &format!("A Draft 2 {}.draftz", files::stamp(t0)), "another project's backup");
    put(&backups_dir, "A Draft final.draftz", "named by hand");
    for i in 1..=3 {
        backup_packed(&DRAFT, b"packed", &backups_dir, "A Draft", 3, t0 + Duration::from_secs(i)).unwrap();
    }
    let kept = backups(&DRAFT, &backups_dir, "A Draft");
    assert_eq!(kept.len(), 3);
    assert!(!first.exists() && !second.exists(), "the oldest are removed");
    assert!(kept.windows(2).all(|pair| pair[0] < pair[1]), "oldest first");
    assert_eq!(backups(&DRAFT, &backups_dir, "A Draft 2").len(), 1);
    assert!(backups_dir.join("A Draft final.draftz").exists());
    assert!(backups(&DRAFT, &dir.join("nowhere"), "A Draft").is_empty());
}
