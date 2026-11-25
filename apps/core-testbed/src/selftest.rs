// SPDX-License-Identifier: MIT OR Apache-2.0
//! The self-test: one pass through every `jcj-` crate, the way an app uses them, without a window.
//!
//! `core-testbed selftest` runs it and prints one line per step; the crate's tests run it too, so
//! a pin move that breaks any crate's use from an app fails here, before any app sees it.

use std::time::{Duration, SystemTime};

use jcj_doc::{CharProps, Document, ExtKey, Paragraph, para_block};
use jcj_interchange::Format;
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

/// Run every step; stop at the first failure.
pub fn run() -> Result<Vec<Step>, String> {
    let mut steps = Vec::new();
    let mut step = |name: &'static str, detail: String| steps.push(Step { name, detail });

    // jcj-doc: the model and extension data.
    let doc = sample_document()?;
    let paras = jcj_doc::walk::body_paragraphs(&doc);
    check(paras.len() == 7, "the sample has seven paragraphs")?;
    check(STATUS.get_or_none(paras[0].1).as_deref() == Some("draft"), "paragraph extension data reads back")?;
    step("doc", format!("{} paragraphs, extension data on document, paragraph and run", paras.len()));

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

    // jcj-agents: an MCP server over a session.
    let info = jcj_agents::AgentInfo::new("core-testbed", "Core testbed", env!("CARGO_PKG_VERSION"), "The core's test app.");
    let mut server = jcj_agents::AgentServer::headless(self::session(doc), jcj_agents::AgentConfig::new(info));
    let init = server.handle(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}})).ok_or("initialize answers")?;
    check(init.pointer("/result/serverInfo/name").and_then(|v| v.as_str()) == Some("core-testbed"), "the server names the app")?;
    let tools = server.handle(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})).ok_or("tools/list answers")?;
    let count = tools.pointer("/result/tools").and_then(|t| t.as_array()).map_or(0, Vec::len);
    check(count > 0, "the server lists tools")?;
    step("agents", format!("{count} tools"));

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
                "sections",
                "layout",
                "output",
                "interchange",
                "proof",
                "file access",
                "project",
                "board",
                "entries",
                "theme",
                "agents"
            ]
        );
    }
}
