// SPDX-License-Identifier: MIT OR Apache-2.0
//! The self-test: one pass through every `jcj-` crate, the way an app uses them, without a window.
//!
//! `core-testbed selftest` runs it and prints one line per step; the crate's tests run it too, so
//! a pin move that breaks any crate's use from an app fails here, before any app sees it.

use std::time::{Duration, SystemTime};

use jcj_doc::{CharProps, Document, ExtKey, Paragraph, para_block};
use jcj_interchange::Format;
use jcj_shell::egui::{Key, Modifiers};
use serde_json::json;

/// A value the test app keeps on paragraphs, as an app would (its own namespace).
pub const STATUS: ExtKey<String> = ExtKey::new("testbed.status");
/// A value the test app keeps on runs.
pub const MARK: ExtKey<u32> = ExtKey::new("testbed.mark");
/// A value the test app keeps on the document.
pub const VERSION: ExtKey<u32> = ExtKey::new("testbed.version");

/// The test app's identity in project files.
pub const APP: jcj_project::App = jcj_project::App::new("org.jcj.core-testbed", 1);

/// The test app's command registry: the engine's commands, the core's general editing commands
/// and the document features, as an app assembles it.
pub fn registry() -> std::sync::Arc<jcj_engine::Registry> {
    std::sync::Arc::new(jcj_engine::registry_with(jcj_engine::commands::merged(jcj_features::commands())))
}

/// A session on `doc` with [`registry`].
pub fn session(doc: Document) -> jcj_engine::Session {
    jcj_engine::session_with(doc, registry())
}

/// One step's outcome.
#[derive(Debug, Clone)]
pub struct Step {
    pub name: &'static str,
    pub detail: String,
}

/// A sample document written for the test app: headings, plain and bold text, and extension data
/// on the document, a paragraph and a run.
pub fn sample_document() -> Result<Document, String> {
    let mut doc = Document::new();
    let mut paras = Vec::new();
    let mut title = Paragraph::with_text("A Sample Document", CharProps::default()).styled("Heading1");
    STATUS.set(&mut title, &"draft".to_string()).map_err(|e| e.to_string())?;
    paras.push(title);
    let body = "This paragraph was written for the core's test app. It is long enough to wrap across \
                several lines of a page, so layout, rendering and export all have something to do.";
    for i in 0..3 {
        paras.push(Paragraph::with_text(&format!("{body} ({})", i + 1), CharProps::default()));
    }
    let bold = CharProps { bold: Some(true), ..Default::default() };
    let mut marked = Paragraph::with_text("Bold text with a mark on it.", bold);
    if let Some(run) = marked.runs.first_mut() {
        MARK.set(run, &7).map_err(|e| e.to_string())?;
    }
    paras.push(marked);
    paras.push(Paragraph::with_text("A Second Part", CharProps::default()).styled("Heading1"));
    paras.push(Paragraph::with_text("Text of the second part.", CharProps::default()));
    doc.body = paras.into_iter().map(para_block).collect();
    VERSION.set(&mut doc, &1).map_err(|e| e.to_string())?;
    Ok(doc)
}

fn check(ok: bool, what: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(what.to_owned()) }
}

/// How the testbed would lay a project out as a folder: a manifest and an outline written
/// last, chapters as text, pictures carried along, a copy kept of what a save would write over.
pub const FOLDER: jcj_project::folder::Format = jcj_project::folder::Format {
    last: &["testbed.toml", "outline.txt"],
    belongs: folder_belongs,
    stored: &[".png"],
    conflicts: Some("conflicts"),
    ..jcj_project::folder::Format::new("testbed.toml", 2, "application/x-jcj-core-testbed-packed", "jcjtestz")
};

fn folder_belongs(path: &str) -> bool {
    path == "outline.txt" || ["chapters/", "pictures/", "conflicts/"].iter().any(|dir| path.starts_with(dir))
}

/// The testbed's reader of a folder project, written once over any source: the folder on disk,
/// a packed file, a recovery copy. It gives the chapters' text in the outline's order, and
/// tells the source what each file it parsed is written back as.
fn read_chapters(source: &mut dyn jcj_project::folder::Source) -> jcj_project::Result<Vec<String>> {
    use jcj_project::folder::read_manifest;
    let (manifest, _schema) = read_manifest(&FOLDER, source)?;
    source.rendered("testbed.toml", manifest.as_bytes());
    let outline = source.read_text("outline.txt", 1 << 20)?.unwrap_or_default();
    source.rendered("outline.txt", outline.as_bytes());
    let mut chapters = Vec::new();
    for name in outline.lines().filter_map(|line| line.trim().strip_prefix("- ")) {
        let path = format!("chapters/{name}.md");
        let text = source.read_text(&path, 1 << 20)?.ok_or_else(|| jcj_project::Error::Damaged(format!("{path} is missing")))?;
        // The testbed writes a chapter with one line break at its end, however it was typed.
        source.rendered(&path, format!("{}\n", text.trim_end()).as_bytes());
        chapters.push(text.trim_end().to_owned());
    }
    Ok(chapters)
}

fn chapter_files(chapters: &[(&str, &str)]) -> jcj_project::folder::Files {
    let mut files = jcj_project::folder::Files::new();
    files.insert("testbed.toml".into(), b"schema = 2\ntitle = \"Folder\"\n".to_vec());
    files.insert("outline.txt".into(), chapters.iter().map(|(name, _)| format!("- {name}\n")).collect::<String>().into_bytes());
    for (name, text) in chapters {
        files.insert(format!("chapters/{name}.md"), format!("{text}\n").into_bytes());
    }
    files
}

/// The folder layer, used the way an app uses it: create, read through a source, save what
/// changed, notice a change made elsewhere, pack and unpack, recover, back up.
fn folder_project() -> Result<String, String> {
    use jcj_project::folder::{self, Disk, Folder, Memory, Sink, Source, Stamp};
    let err = |e: jcj_project::Error| e.to_string();
    let n = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("core-testbed-folder-{}-{n}", std::process::id()));
    let root = dir.join("Folder");

    // A new project, opened again and read.
    let first = chapter_files(&[("one", "The first chapter."), ("two", "The second chapter.")]);
    check(first.keys().all(|path| FOLDER.allows(path) && folder::plain_path(path, FOLDER.limits.name)), "the testbed's files are its own")?;
    check(folder::any_path("anything") && !FOLDER.allows("readme.txt") && folder::toml_nesting("a = [[1]]") == 2, "names and nesting are checked")?;
    Folder::create(&root, &FOLDER, &first).map_err(err)?;
    std::fs::write(root.join("chapters/two.md"), "The second chapter.\n\n\n").map_err(|e| e.to_string())?;
    std::fs::create_dir_all(root.join("pictures"))
        .and_then(|()| std::fs::write(root.join("pictures/map.png"), b"a picture"))
        .map_err(|e| e.to_string())?;
    let mut project = Folder::open(&root, &FOLDER).map_err(err)?;
    check(project.schema() == 2 && folder::schema_of("schema = 2").ok() == Some(2), "the manifest says its schema")?;
    check(project.root() == root && project.format().manifest == "testbed.toml", "the folder knows where and what it is")?;
    let chapters = read_chapters(&mut project).map_err(err)?;
    check(chapters == ["The first chapter.", "The second chapter."], "the chapters read in the outline's order")?;
    check(project.list("chapters").map_err(err)? == ["one.md", "two.md"], "a folder of the project lists its files")?;
    check(project.read_tree("pictures", 1 << 20).map_err(err)?.len() == 1, "a folder is read whole")?;
    check(
        project.stamp("chapters/two.md") == Some(Stamp::of(b"The second chapter.\n\n\n", b"The second chapter.\n")),
        "a file is known as read and as written",
    )?;

    // Saving writes what changed, the manifest's companions last, and leaves hand-typed text.
    let mut second = chapter_files(&[("one", "The first chapter, revised."), ("two", "The second chapter."), ("three", "A third.")]);
    second.insert("pictures/map.png".into(), b"a picture".to_vec());
    let saved = project.save_safely(&second).map_err(err)?;
    check(saved.written == ["chapters/one.md", "chapters/three.md", "outline.txt"], "only what changed is written, the outline last")?;
    check(std::fs::read(root.join("chapters/two.md")).ok().as_deref() == Some(b"The second chapter.\n\n\n"), "text typed by hand is not rewritten")?;
    check(folder::recovered(&FOLDER, &root).is_none() && project.known().count() == 6, "a good save leaves no recovery copy")?;

    // A change made elsewhere is found, kept when a save writes over it, and taken in.
    std::fs::write(root.join("chapters/one.md"), "Changed on another device.\n").map_err(|e| e.to_string())?;
    let outside = project.outside_changes(&|path| path.starts_with("chapters/")).map_err(err)?;
    check(outside.len() == 1 && outside[0].path == "chapters/one.md" && outside[0].now.is_some(), "the change made elsewhere is found")?;
    second.insert("chapters/one.md".into(), b"The first chapter, revised again.\n".to_vec());
    let mut disk = Disk::new(&root);
    let kept = project.save_to(&second, &mut disk).map_err(err)?;
    check(kept.conflicts.len() == 1 && kept.conflicts[0].starts_with("conflicts/"), "a copy of their change is kept")?;
    check(
        FOLDER.conflict_name("chapters/one.md", SystemTime::UNIX_EPOCH).is_some_and(|name| name.ends_with("chapters-one.md")),
        "a conflict copy is named for its file",
    )?;
    disk.write("chapters/four.md", b"Added elsewhere.\n").map_err(err)?;
    let added = project.outside_changes(&|path| path.starts_with("chapters/")).map_err(err)?;
    check(added.len() == 1 && added[0].path == "chapters/four.md", "a file added elsewhere is found")?;
    project.accept("chapters/four.md", Some(Stamp::of(b"Added elsewhere.\n", b"Added elsewhere.\n")));
    second.insert("chapters/four.md".into(), b"Added elsewhere.\n".to_vec());
    check(project.save(&second).map_err(err)? == folder::Saved::default(), "a change taken in is not written again")?;

    // One file: the same bytes each time, read back within limits, unpacked into a new folder.
    let packed = folder::pack_folder(&FOLDER, &root).map_err(err)?;
    let on_disk = folder::folder_files(&FOLDER, &root).map_err(err)?;
    let again = folder::pack(&FOLDER, &on_disk).map_err(err)?;
    check(again == packed && folder::hash(&again) == folder::hash(&packed), "packing gives the same bytes")?;
    let unpacked = folder::unpack(&FOLDER, &packed).map_err(err)?;
    check(unpacked == on_disk, "a packed project unpacks to its files")?;
    check(read_chapters(&mut Memory(&unpacked)).map_err(err)?.len() == 3, "the same reader reads a packed project")?;
    let (copy, _) = folder::unpack_into(&FOLDER, &packed, &dir.join("Copy"), |files| read_chapters(&mut Memory(files)).map(|_| ())).map_err(err)?;
    check(copy.root().join("pictures/map.png").is_file(), "a packed project unpacks into a new folder")?;
    let tiny = jcj_project::folder::Format { limits: folder::Limits { entry: 4, ..folder::Limits::DEFAULT }, ..FOLDER };
    check(matches!(folder::unpack(&tiny, &packed), Err(jcj_project::Error::TooLarge(_))), "a packed project is read within limits")?;

    // Recovery: unsaved work kept inside the folder, found again, then discarded.
    let unsaved = chapter_files(&[("one", "Typed and not saved.")]);
    folder::write_recovery(&FOLDER, &root, &unsaved).map_err(err)?;
    check(folder::recovery_path(&FOLDER, &root).is_file(), "the recovery copy is inside the folder")?;
    let found = folder::recovered(&FOLDER, &root).ok_or("the unsaved work is found")?;
    check(read_chapters(&mut Memory(&found)).map_err(err)? == ["Typed and not saved."], "the unsaved work reads")?;
    folder::discard_recovery(&FOLDER, &root).map_err(err)?;

    // Backups: dated packed copies, the oldest removed.
    let backups = dir.join("Backups");
    let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000);
    folder::backup(&FOLDER, &root, &backups, "Folder", 2, at).map_err(err)?;
    folder::backup_packed(&FOLDER, &packed, &backups, "Folder", 2, at).map_err(err)?;
    let newest =
        jcj_project::files::backup_bytes(&packed, std::path::Path::new("Folder.jcjtestz"), &backups, 2, at + Duration::from_secs(1)).map_err(err)?;
    let listed = folder::backups(&FOLDER, &backups, "Folder");
    check(listed.len() == 2 && listed.last() == Some(&newest), "two backups are kept, oldest first")?;

    let _ = std::fs::remove_dir_all(&dir);
    Ok(format!("{} files, {} bytes packed, {} conflict copy, {} backups kept", on_disk.len(), packed.len(), kept.conflicts.len(), listed.len()))
}

/// Run every step; stop at the first failure.
pub fn run() -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    let mut step = |name: &'static str, detail: String| steps.push(Step { name, detail });

    // jcj-doc: the model and extension data.
    let doc = sample_document()?;
    let paras = jcj_doc::walk::body_paragraphs(&doc);
    check(paras.len() == 7, "the sample has seven paragraphs")?;
    check(STATUS.get_or_none(paras[0].1).as_deref() == Some("draft"), "paragraph extension data reads back")?;
    // The word list and the word-level comparison from the model crate alone, as an app's model
    // crate (no engine, no layout) uses them on its own text type.
    let lines: Vec<String> = paras.iter().map(|(_, p)| jcj_doc::plain_text(p)).collect();
    let listed: usize = lines.iter().map(|l| jcj_doc::words(l).len()).sum();
    check(listed == jcj_doc::verify::document_words(&doc).len(), "the word list is the document's words")?;
    let mut reworded = lines.clone();
    if let Some(last) = reworded.last_mut() {
        *last = last.replacen("second", "last", 1);
    }
    let reworded = jcj_doc::wordiff::paragraphs(&lines, &reworded);
    check(reworded.added == 1 && reworded.removed == 1 && !reworded.is_same(), "a changed word shows in two lists of paragraphs")?;
    step("doc", format!("{} paragraphs, {listed} words listed, extension data on document, paragraph and run", paras.len()));

    // jcj-engine: a session, a command, undo and redo.
    let mut session = self::session(doc.clone());
    let before = jcj_doc::plain_text(paras[1].1);
    session.run("text.insert", &json!({"text": "Inserted. "})).map_err(|e| format!("text.insert: {e:?}"))?;
    let changed = session.doc.body.len();
    session.run("edit.undo", &json!({})).map_err(|e| format!("edit.undo: {e:?}"))?;
    check(session.doc.same_content(&doc), "undo restores the document")?;
    session.run("edit.redo", &json!({})).map_err(|e| format!("edit.redo: {e:?}"))?;
    step("engine", format!("insert, undo, redo ({changed} blocks; first body paragraph starts {:?})", before.get(..12).unwrap_or("")));

    // jcj-engine's general commands and jcj-features: find, a bookmark, a note, undo.
    let found = session.run("find.all", &json!({"query": "Second"})).map_err(|e| format!("find.all: {e:?}"))?;
    let hits = found.get("matches").and_then(|m| m.as_array()).map_or(0, Vec::len);
    check(hits == 2, "find.all finds both matches")?;
    session.run("bookmark.add", &json!({"name": "Start"})).map_err(|e| format!("bookmark.add: {e:?}"))?;
    let marks = session.run("bookmark.list", &json!({})).map_err(|e| format!("bookmark.list: {e:?}"))?;
    check(marks.to_string().contains("Start"), "the bookmark is listed")?;
    session.run("note.add", &json!({"text": "A note on the caret's paragraph."})).map_err(|e| format!("note.add: {e:?}"))?;
    session.run("edit.undo", &json!({})).map_err(|e| format!("edit.undo: {e:?}"))?;
    session.run("edit.undo", &json!({})).map_err(|e| format!("edit.undo: {e:?}"))?;
    let marks = session.run("bookmark.list", &json!({})).map_err(|e| format!("bookmark.list: {e:?}"))?;
    check(!marks.to_string().contains("Start"), "undo removes the bookmark")?;
    step("features", format!("find.all: {hits} matches; bookmark and note added and undone"));

    // jcj-shell's keys: a shifted caret key extends the selection, by the key map's own rule,
    // with the keys the registry's commands declare.
    let mut keyed = self::session(doc.clone());
    let infos: Vec<jcj_shell::CommandInfo> = keyed.registry.all().iter().map(|c| jcj_shell::CommandInfo::new(c.id, c.label, c.shortcut)).collect();
    let keys = jcj_shell::keys::KeyMap::from_commands(&infos);
    let extend = keys.command_for("Shift+Right").ok_or("Shift+Right runs a command")?;
    check(keys.shifted("Shift+Right").as_ref() == Some(&extend) && extend.id.starts_with(jcj_shell::keys::CARET_COMMANDS), "the rule gives it")?;
    keyed.run("caret.docStart", &json!({})).map_err(|e| format!("caret.docStart: {e:?}"))?;
    keyed.run(&extend.id, &extend.params).map_err(|e| format!("{}: {e:?}", extend.id))?;
    check(keyed.sel.anchor != keyed.sel.focus, "a shifted caret key extends the selection")?;
    check(keys.command_for("Shift+Backspace").map(|c| c.id).as_deref() == Some(jcj_shell::keys::BACKSPACE), "Shift+Backspace deletes back")?;
    // macOS's own text keys reach the engine's caret commands, as this system's engine declares
    // them: Option+Arrow by word, Command+Left the line's start, Command+Up the document's.
    let opt = Modifiers { alt: true, ..Modifiers::NONE };
    let command = Modifiers { command: true, mac_cmd: true, ..Modifiers::NONE };
    let mac_press = |key: Key, m: Modifiers| jcj_shell::keys::shortcut(key, m, true).and_then(|s| keys.command_for(&s)).map(|c| c.id);
    check(mac_press(Key::ArrowLeft, opt).as_deref() == Some("caret.wordLeft"), "Option+Left moves by word on macOS")?;
    check(
        mac_press(Key::ArrowRight, Modifiers { shift: true, ..opt }).as_deref() == Some("caret.wordRight"),
        "Option+Shift+Right selects by word on macOS",
    )?;
    check(mac_press(Key::ArrowLeft, command).as_deref() == Some("caret.home"), "Command+Left goes to the line's start on macOS")?;
    check(mac_press(Key::ArrowUp, command).as_deref() == Some("caret.docStart"), "Command+Up goes to the document's start on macOS")?;
    step("keys", format!("Shift+Right runs {} extending the selection; Shift+Backspace deletes back; macOS's text keys reach the caret", extend.id));

    // jcj-doc's section settings: a second section starting on an odd page, numbered in roman.
    let mut sectioned = doc.clone();
    sectioned.last_section = jcj_doc::SectionProps {
        start: jcj_doc::section::SectionStart::OddPage,
        page_num_format: jcj_doc::section::NumFormat::LowerRoman,
        ..jcj_doc::SectionProps::default()
    };
    let mut first = jcj_doc::Paragraph::with_text("Before the section.", CharProps::default());
    first.section = Some(Box::new(jcj_doc::SectionProps::default()));
    sectioned.body.insert(0, para_block(first));
    let sectioned_pages = jcj_layout::page_count(&jcj_layout::print_layout(&sectioned, &mut jcj_layout::LayoutCache::new()));
    check(sectioned_pages == 3, "a section starting on an odd page leaves page 2 blank")?;
    let note = sectioned.add_part(jcj_doc::PartKind::Footnote, vec![para_block(Paragraph::with_text("A footnote.", CharProps::default()))]);
    let reference = jcj_doc::para::InlineObject::NoteRef { kind: jcj_doc::para::NoteKind::Footnote, id: note, custom: String::new() };
    if let Some(jcj_doc::Block::Para(p)) = sectioned.body.first_mut().map(std::sync::Arc::make_mut) {
        let at = p.text.len();
        p.insert_object(at, reference, &CharProps::default()).map_err(|e| format!("note reference: {e:?}"))?;
    }
    check(jcj_layout::page_count(&jcj_layout::print_layout(&sectioned, &mut jcj_layout::LayoutCache::new())) == 3, "a footnote keeps its page")?;
    step("sections", format!("{sectioned_pages} pages: a section on an odd page, numbered in roman, with a footnote"));
    // The EPUB writer keeps the footnote, as an endnote linked both ways.
    let epub = jcj_interchange::export(Format::Epub, &sectioned).map_err(|e| format!("export EPUB: {e}"))?;
    let back = jcj_interchange::import(Format::Epub, &epub).map_err(|e| format!("import EPUB: {e}"))?;
    let text: String = jcj_doc::walk::body_paragraphs(&back).iter().map(|(_, p)| jcj_doc::plain_text(p)).collect();
    check(text.contains("A footnote."), "the EPUB keeps the footnote")?;
    // The same document with the notes the way a caller chooses: numbered continuously, a
    // heading in the document's language, a link back that adds no word. The writer says which words
    // it added, so a check of the file leaves out exactly those.
    let chosen = jcj_interchange::EpubOptions::default()
        .note_numbering(jcj_interchange::NoteNumbering::Continuous)
        .note_backlink(jcj_interchange::NoteBacklink::Symbol { label: "Volver al texto".into() })
        .note_heading("Notas");
    let noted = jcj_interchange::export_epub(&sectioned, &chosen).map_err(|e| format!("export EPUB: {e}"))?;
    check(noted.added_words == ["1", "Notas", "1"], "the EPUB writer says which words it added")?;
    let back = jcj_interchange::import(Format::Epub, &noted.bytes).map_err(|e| format!("import EPUB: {e}"))?;
    let letters = |words: &[String]| {
        let mut all: Vec<char> = words.iter().flat_map(|w| w.chars()).filter(|c| c.is_alphanumeric()).collect();
        all.sort_unstable();
        all
    };
    let mut source = jcj_doc::verify::document_words(&sectioned);
    source.extend(jcj_doc::words("A footnote."));
    source.extend(noted.added_words.iter().cloned());
    check(letters(&jcj_doc::verify::document_words(&back)) == letters(&source), "the EPUB holds the document's words and the added ones, no other")?;
    let plain = jcj_interchange::export_epub(&sectioned, &jcj_interchange::EpubOptions::default()).map_err(|e| format!("export EPUB: {e}"))?;
    check(plain.bytes == epub, "the EPUB writer's default choices are the plain export")?;

    // jcj-layout: pages.
    let mut cache = jcj_layout::LayoutCache::new();
    let layout = jcj_layout::print_layout(&doc, &mut cache);
    let pages = jcj_layout::page_count(&layout);
    check(pages >= 1, "the sample lays out onto at least one page")?;
    let boxes = jcj_layout::page_boxes(&layout);
    step("layout", format!("{pages} page(s), first page {:.0} x {:.0} pt", boxes[0].page.w, boxes[0].page.h));

    // jcj-output: PDF and a page image.
    let pdf = jcj_output::export_pdf(&doc, &layout, &jcj_output::PdfSettings::deterministic()).map_err(|e| format!("pdf: {e}"))?;
    check(pdf.starts_with(b"%PDF-"), "the PDF starts with its header")?;
    let image = jcj_output::render_page(&doc, &layout, 0, 0.5, &jcj_output::RenderSettings::default()).ok_or("page 1 did not render")?;
    check(image.width > 0 && image.height > 0, "the page image has a size")?;
    // Read the PDF back: its pages and their text, as an app's import or golden test does.
    let read = jcj_output::read_pages(&pdf, &jcj_output::ReadLimits::default()).map_err(|e| format!("pdf read: {e}"))?;
    check(read.len() == pages, "the PDF reads back with every page")?;
    let text: String =
        read.iter().flat_map(|p| &p.glyphs).filter(|g| g.mode == jcj_output::GlyphMode::Fill).filter_map(|g| g.text.as_deref()).collect();
    check(text.contains("A Sample Document") && text.contains("Second Part"), "the PDF's text reads back")?;
    let glyphs: usize = read.iter().map(|p| p.glyphs.len()).sum();
    // Its facts, as a preflight check reads them: every font embedded, page boxes.
    let facts = jcj_output::read_facts(&pdf, &jcj_output::ReadLimits::default()).map_err(|e| format!("pdf facts: {e}"))?;
    check(facts.pages.len() == pages, "the PDF's facts cover every page")?;
    check(!facts.fonts.is_empty() && facts.unembedded_fonts().is_empty(), "the PDF embeds every font")?;
    // Trim boxes for a printer, set on the exported file and read back.
    let boxes: Vec<jcj_output::Boxes> = facts.pages.iter().map(|p| jcj_output::Boxes { trim: p.media, bleed: None }).collect();
    let boxed = jcj_output::set_page_boxes(&pdf, &boxes, &jcj_output::ReadLimits::default()).map_err(|e| format!("pdf boxes: {e}"))?;
    let boxed_facts = jcj_output::read_facts(&boxed, &jcj_output::ReadLimits::default()).map_err(|e| format!("pdf facts: {e}"))?;
    check(boxed_facts.pages.iter().all(|p| p.trim == Some(p.media)), "the PDF's trim boxes read back")?;
    // PDF/A-2b and PDF/UA-1 declared on a tagged export; the text reads back the same.
    let tagged = jcj_output::export_pdf(&doc, &layout, &jcj_output::PdfSettings { tagged: true, ..jcj_output::PdfSettings::deterministic() })
        .map_err(|e| format!("pdf: {e}"))?;
    let conformance = jcj_output::Conformance { pdf_a: true, pdf_ua: true, author: "The testbed".into(), language: "en".into() };
    let declared = jcj_output::declare(&tagged, &conformance, &jcj_output::ReadLimits::default()).map_err(|e| format!("pdf standards: {e}"))?;
    let declared_text = String::from_utf8_lossy(&declared);
    check(declared_text.contains("<pdfaid:part>2</pdfaid:part>") && declared_text.contains("/OutputIntents"), "the PDF declares PDF/A-2b")?;
    check(declared_text.contains("<pdfuaid:part>1</pdfuaid:part>") && declared_text.contains("/DisplayDocTitle true"), "the PDF declares PDF/UA-1")?;
    let again = jcj_output::read_pages(&declared, &jcj_output::ReadLimits::default()).map_err(|e| format!("pdf read: {e}"))?;
    check(again.len() == pages, "the declared PDF reads back with every page")?;
    step(
        "output",
        format!(
            "PDF {} bytes, {glyphs} glyphs read back, {} fonts all embedded; page image {} x {}",
            pdf.len(),
            facts.fonts.len(),
            image.width,
            image.height
        ),
    );

    // jcj-interchange: every format out, and back in where it can be read.
    let mut formats = Vec::new();
    for format in Format::ALL {
        let bytes = jcj_interchange::export(format, &doc).map_err(|e| format!("export {format:?}: {e}"))?;
        // EPUB is not offered for import by default, but reads back when asked for by name.
        if format.can_import() || format == Format::Epub {
            let back = jcj_interchange::import(format, &bytes).map_err(|e| format!("import {format:?}: {e}"))?;
            let text: String = jcj_doc::walk::body_paragraphs(&back).iter().map(|(_, p)| jcj_doc::plain_text(p)).collect();
            check(text.contains("Second Part"), &format!("{format:?} keeps the text"))?;
            // Every word comes back, in order (jcj_features::verify).
            let words = jcj_features::verify::Check::of(&jcj_features::verify::document_words(&doc), &jcj_features::verify::document_words(&back));
            check(words.passed(), &format!("{format:?} keeps every word: {words}"))?;
        }
        formats.push(format.extension());
    }
    // RTF with each paragraph's indents written out, as an app that lays out its own paragraphs
    // exports it: an indented paragraph keeps its indent when read back.
    let mut indented = doc.clone();
    if let Some(jcj_doc::Block::Para(p)) = indented.body.get_mut(1).map(std::sync::Arc::make_mut) {
        p.props.indent_left = Some(144.0);
    }
    let rtf = jcj_interchange::export_rtf_direct(&indented).map_err(|e| format!("direct RTF: {e}"))?;
    let back = jcj_interchange::import(Format::Rtf, &rtf).map_err(|e| format!("direct RTF import: {e}"))?;
    let indent = jcj_doc::walk::body_paragraphs(&back).get(1).map(|(_, p)| back.styles.resolve_para(&p.props).indent_left);
    check(indent == Some(144.0), "direct RTF keeps a paragraph's indent")?;
    // Word by word: one changed word in the edited copy shows as one removed and one added.
    let mut edited = doc.clone();
    if let Some(jcj_doc::Block::Para(p)) = edited.body.get_mut(1).map(std::sync::Arc::make_mut) {
        *p = Paragraph::with_text(&jcj_doc::plain_text(p).replacen("written", "made", 1), CharProps::default());
    }
    let diff = jcj_features::wordiff::documents(&doc, &edited);
    check(diff.removed == 1 && diff.added == 1, "a changed word shows word by word")?;
    step("interchange", format!("exported {}; direct RTF keeps indents; every word verified; a change shown word by word", formats.join(", ")));

    // jcj-proof: spelling and hyphenation.
    let issues = jcj_proof::check("This sentense has a mistake.", Some("en-US"), &jcj_proof::SpellingOptions::default(), false);
    check(issues.iter().any(|i| i.range == (5..13)), "the misspelled word is found")?;
    // The dictionary generation tells a spelling cache when to check again.
    let generation = jcj_proof::dictionary_generation();
    jcj_proof::add_word("testbedword");
    check(jcj_proof::dictionary_generation() != generation, "adding a word moves the dictionary generation")?;
    jcj_proof::remove_word("testbedword");
    step("proof", format!("{} issue(s); hyphenation: {}", issues.len(), jcj_proof::hyphenate("documentation", Some("en-US"))));

    // jcj-engine's file access: which folders are in force (none here, then the temp folder).
    check(jcj_engine::file_access::current().is_none(), "nothing is confined by default")?;
    let roots = jcj_engine::file_access::AutomationRoots::new(Some(&std::env::temp_dir()), None)?;
    let confined = jcj_engine::file_access::confine(roots.as_ref(), || jcj_engine::file_access::current().is_some());
    check(confined, "confined code sees its folders")?;
    step("file access", "confinement reported".to_owned());

    // jcj-project: a container with two documents, an asset and a snapshot.
    let mut project = jcj_project::Project::with_document(&APP, "Main", doc.clone()).map_err(|e| e.to_string())?;
    let notes = project.add_document("Notes", Document::new()).map_err(|e| e.to_string())?;
    project.add_asset(b"asset bytes".to_vec(), "bin").map_err(|e| e.to_string())?;
    project.take_snapshot(notes, "Before", SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000)).map_err(|e| e.to_string())?;
    let bytes = project.to_bytes().map_err(|e| e.to_string())?;
    let back = jcj_project::Project::from_bytes(&bytes, &APP).map_err(|e| e.to_string())?;
    let main = back.find_document("Main").and_then(|id| back.document(id)).ok_or("the main document is back")?;
    check(VERSION.get_or_none(main) == Some(1), "document extension data survives the project file")?;
    check(back.to_bytes().map_err(|e| e.to_string())? == bytes, "saving again gives the same bytes")?;
    step("project", format!("{} bytes, {} documents, {} snapshot(s)", bytes.len(), back.document_count(), back.snapshots().len()));

    // jcj-project's folder layer: a project as a folder of plain files.
    step("folder", folder_project()?);

    // jcj-board: cards.
    let mut board = jcj_board::Board::default();
    let a = board.add_card("First", "A card.").map_err(|e| format!("{e:?}"))?;
    let b = board.add_card("Second", "Another card.").map_err(|e| format!("{e:?}"))?;
    check(board.reorder(1, Some(0)), "cards reorder")?;
    check(board.index_of(b) == Some(0) && board.index_of(a) == Some(1), "the order changed")?;
    step("board", "two cards, reordered".to_owned());

    // jcj-entries: entries found by name in the sample's text.
    let mut people: Vec<(u32, jcj_entries::Entry<u32, String>)> =
        vec![(1, jcj_entries::Entry::new("person", "Sample")), (2, jcj_entries::Entry::new("person", "Nobody"))];
    if let Some((_, e)) = people.get_mut(0) {
        e.aliases.push("Example".into());
    }
    let paragraphs: Vec<String> = paras.iter().map(|(_, p)| p.plain_text()).collect();
    let docs = [jcj_entries::Document { id: 100u32, links: &[], paragraphs: &paragraphs }];
    let refs = jcj_entries::backlinks(1, &people[0].1.names(), docs);
    check(refs.mentioned == vec![100], "the sample mentions the entry by name")?;
    check(jcj_entries::find_by_name(people.iter().map(|(i, e)| (i, e)), "example") == Some(1), "an alias finds its entry")?;
    step("entries", "an entry found by its name in the sample".to_owned());

    // jcj-theme: the neutral tokens make a theme.
    let tokens = jcj_theme::Tokens::default();
    let json = tokens.to_json();
    check(jcj_theme::Tokens::from_json(&json).map_err(|e| e.to_string())? == tokens, "tokens round trip as JSON")?;
    step("theme", format!("neutral tokens, {} bytes of JSON", json.len()));

    // jcj-agents: an MCP server over a session and the testbed's own commands, every tool of
    // the testbed's own named after it.
    let mut server = crate::agents::server(doc, None);
    let init = server.handle(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}})).ok_or("initialize answers")?;
    check(init.pointer("/result/serverInfo/name").and_then(|v| v.as_str()) == Some("core-testbed"), "the server names the app")?;
    let tools = server.handle(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})).ok_or("tools/list answers")?;
    let count = tools.pointer("/result/tools").and_then(|t| t.as_array()).map_or(0, Vec::len);
    check(count > 0, "the server lists tools")?;
    let own = server.tool_names();
    check(!own.is_empty() && own.iter().all(|n| n.starts_with("testbed_")), "the testbed's own tools carry its name")?;
    check(jcj_agents::app_tool_name(crate::agents::PREFIX, "board.list") == "testbed_board_list", "an app's tool is named after the app")?;
    step("agents", format!("{count} tools, {} of them the testbed's own", own.len()));

    Ok(steps)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_self_test_passes() {
        let steps = super::run().unwrap();
        let names: Vec<&str> = steps.iter().map(|s| s.name).collect();
        assert_eq!(
            names,
            [
                "doc",
                "engine",
                "features",
                "keys",
                "sections",
                "layout",
                "output",
                "interchange",
                "proof",
                "file access",
                "project",
                "folder",
                "board",
                "entries",
                "theme",
                "agents"
            ]
        );
    }
}
