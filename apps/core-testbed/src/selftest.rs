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
    let mut session = jcj_engine::session(doc.clone());
    let before = jcj_doc::plain_text(paras[1].1);
    session.run("text.insert", &json!({"text": "Inserted. "})).map_err(|e| format!("text.insert: {e:?}"))?;
    let changed = session.doc.body.len();
    session.run("edit.undo", &json!({})).map_err(|e| format!("edit.undo: {e:?}"))?;
    check(session.doc.same_content(&doc), "undo restores the document")?;
    session.run("edit.redo", &json!({})).map_err(|e| format!("edit.redo: {e:?}"))?;
    step("engine", format!("insert, undo, redo ({changed} blocks; first body paragraph starts {:?})", before.get(..12).unwrap_or("")));

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
    step("output", format!("PDF {} bytes; page image {} x {}", pdf.len(), image.width, image.height));

    // jcj-interchange: every format out, and back in where it can be read.
    let mut formats = Vec::new();
    for format in Format::ALL {
        let bytes = jcj_interchange::export(format, &doc).map_err(|e| format!("export {format:?}: {e}"))?;
        if format.can_import() {
            let back = jcj_interchange::import(format, &bytes).map_err(|e| format!("import {format:?}: {e}"))?;
            let text: String = jcj_doc::walk::body_paragraphs(&back).iter().map(|(_, p)| jcj_doc::plain_text(p)).collect();
            check(text.contains("Second Part"), &format!("{format:?} keeps the text"))?;
        }
        formats.push(format.extension());
    }
    step("interchange", format!("exported {}", formats.join(", ")));

    // jcj-proof: spelling and hyphenation.
    let issues = jcj_proof::check("This sentense has a mistake.", Some("en-US"), &jcj_proof::SpellingOptions::default(), false);
    check(issues.iter().any(|i| i.range == (5..13)), "the misspelled word is found")?;
    step("proof", format!("{} issue(s); hyphenation: {}", issues.len(), jcj_proof::hyphenate("documentation", Some("en-US"))));

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

    // jcj-theme: the neutral tokens make a theme.
    let tokens = jcj_theme::Tokens::default();
    let json = tokens.to_json();
    check(jcj_theme::Tokens::from_json(&json).map_err(|e| e.to_string())? == tokens, "tokens round trip as JSON")?;
    step("theme", format!("neutral tokens, {} bytes of JSON", json.len()));

    // jcj-agents: an MCP server over a session.
    let info = jcj_agents::AgentInfo::new("core-testbed", "Core testbed", env!("CARGO_PKG_VERSION"), "The core's test app.");
    let mut server = jcj_agents::AgentServer::headless(jcj_engine::session(doc), jcj_agents::AgentConfig::new(info));
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
        assert_eq!(names, ["doc", "engine", "layout", "output", "interchange", "proof", "project", "board", "theme", "agents"]);
    }
}
