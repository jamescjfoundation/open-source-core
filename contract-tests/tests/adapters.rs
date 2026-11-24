// SPDX-License-Identifier: MIT OR Apache-2.0
//! Each adapter function with its documented inputs and outputs. A failure here means an
//! upstream change altered what a `jcj-` function promises.

use std::sync::Arc;

use contract_tests::{document, lines, lines_per_page, long_para, para};
use jcj_doc::{Document, ExtKey, Paragraph, Path, Pos, StoryRef};
use jcj_layout::paginate::{Continuation, NoContinuations, PageBreak, PaginationPass, Piece, paginate};
use jcj_layout::{LayoutCache, Rect, ViewMode, hit, page_boxes, page_count, print_layout};
use serde_json::{Value, json};

// ---- jcj-layout ----------------------------------------------------------------------------

#[test]
fn layout_letter_pages_with_inch_margins() {
    let d = document(lines(100));
    let l = print_layout(&d, &mut LayoutCache::new());
    assert_eq!(page_count(&l), 100usize.div_ceil(lines_per_page()));
    let b = page_boxes(&l);
    assert_eq!(b[0].page, Rect::new(0.0, 0.0, 612.0, 792.0));
    assert_eq!(b[0].body, Rect::new(72.0, 72.0, 468.0, 648.0));
    assert_eq!((b[0].number, b[1].number), (1, 2));
    assert_eq!(b[0].markup, 0.0);
    // A web layout is one page whatever the length.
    let opts = jcj_layout::LayoutOptions { view: ViewMode::Web, web_width: 500.0, ..Default::default() };
    assert_eq!(page_count(&jcj_layout::layout(&d, &mut LayoutCache::new(), &opts)), 1);
}

#[test]
fn hit_testing_maps_points_and_positions_both_ways() {
    let d = document(vec![para("Alpha beta gamma"), para("Delta")]);
    let l = print_layout(&d, &mut LayoutCache::new());
    // The caret at the start of the body is at the top left of the body area.
    let c = hit::caret_rect(&l, &Pos::body(0, 0)).unwrap();
    assert_eq!(c.page, 0);
    assert!((c.rect.x - 72.0).abs() < 0.5 && c.rect.y >= 72.0 && c.rect.y < 90.0, "{c:?}");
    // A point left of the text goes to the line's start, right of it to its end.
    let y = c.rect.y + c.rect.h / 2.0;
    assert_eq!(hit::hit(&l, 0, 10.0, y), Some(Pos::body(0, 0)));
    assert_eq!(hit::hit(&l, 0, 600.0, y), Some(Pos::body(0, "Alpha beta gamma".len())));
    // Below all the text: the last line.
    assert_eq!(hit::hit(&l, 0, 100.0, 700.0).map(|p| p.path), Some(Path::top(1)));
    assert_eq!(hit::page_of(&l, &Pos::body(1, 0)), Some(0));
    assert!(hit::caret_rect(&l, &Pos::body(5, 0)).is_none(), "no such paragraph");
}

#[test]
fn the_bundled_fonts_are_there_without_the_system() {
    for f in jcj_layout::fonts::BUNDLED_FAMILIES {
        assert!(jcj_layout::fonts::is_loaded(f), "{f}");
    }
}

#[test]
fn measurement_reports_lines_heights_and_keep_settings() {
    let mut p = long_para(3);
    p.props.keep_next = Some(true);
    let d = document(vec![p, para("Next")]);
    let l = print_layout(&d, &mut LayoutCache::new());
    let m = jcj_layout::measure::measure(&d, &l);
    assert_eq!(m.len(), 2);
    assert!(m[0].keep_next && !m[1].keep_next);
    assert!(m[0].widow_control, "the fixture's default");
    assert_eq!(m[0].lines.len(), m[0].heights.len());
    assert_eq!(m[0].lines.last().unwrap().1, m[0].text.len());
    let line = m[1].height();
    assert!(line > 12.0 && line < 20.0, "{line}");
    let alone = jcj_layout::measure::paragraph_height(&d, d.body[1].as_para().unwrap());
    assert!((alone - line).abs() < 0.01);
}

/// Repeats a paragraph's first three letters, with "(cont.)", at the head of the next page.
struct Repeat;

impl PaginationPass for Repeat {
    fn continuation(&self, doc: &Document, at: &PageBreak<'_>) -> Continuation {
        let Some(after) = at.after.filter(|_| at.splits_paragraph()) else { return Continuation::default() };
        let text = doc.body.get(after.block).and_then(|b| b.as_para()).map(|p| p.text.chars().take(3).collect::<String>()).unwrap_or_default();
        Continuation { foot: vec![para("(more)")], head: vec![para(&format!("{text} (cont.)"))] }
    }
}

#[test]
fn a_pagination_pass_adds_continuation_lines_and_keeps_the_map() {
    let n = lines_per_page();
    let mut paras = lines(n - 4);
    paras.push(long_para(8));
    paras.push(para("End."));
    let d = document(paras);
    // Without a pass: the source's own pages.
    let plain = paginate(&d, &mut NoContinuations);
    assert!(plain.matches_plan);
    assert_eq!(plain.pages.len(), print_layout(&d, &mut LayoutCache::new()).pages.len());
    // With it: the foot and head lines around the split.
    let p = paginate(&d, &mut Repeat);
    assert!(p.matches_plan);
    let added: Vec<(&str, bool)> = p
        .pages
        .iter()
        .flatten()
        .filter_map(|piece| match piece {
            Piece::Added { paragraph, foot } => Some((paragraph.text.as_str(), *foot)),
            Piece::Source { .. } => None,
        })
        .collect();
    assert_eq!(added, [("(more)", true), ("The (cont.)", false)]);
    // The laid-out output puts the head line at the top of page 2.
    let head = p.page_starts()[1];
    assert_eq!(p.doc.body[head].as_para().unwrap().text, "The (cont.)");
    assert_eq!(hit::page_of(&p.layout, &Pos::body(head, 0)), Some(1));
    // Positions map both ways across the split.
    let src = Pos::body(n - 4, d.body[n - 4].as_para().unwrap().text.len() - 2);
    let out = p.to_output(&src).unwrap();
    assert!(out.path.last() > head);
    assert_eq!(p.to_source(&out), Some(src));
}

// ---- jcj-engine ----------------------------------------------------------------------------

const NOTE: ExtKey<String> = ExtKey::new("contract.note");

fn note_specs() -> Vec<jcj_engine::CommandSpec> {
    vec![
        jcj_engine::CommandSpec::new("contract.note", "Note", "Contract", |s, v| {
            let t = jcj_engine::params::str(v, "text").unwrap_or_default().to_string();
            NOTE.set(&mut s.doc, &t).map_err(|e| jcj_engine::CmdError::Failed(e.to_string()))?;
            Ok(json!(t))
        })
        .params(r#"{"text": string}"#),
        jcj_engine::CommandSpec::new("contract.readNote", "Read Note", "Contract", |s, _| Ok(json!(NOTE.get_or_none(&s.doc)))).pure(),
    ]
}

#[test]
fn an_app_registry_runs_its_commands_and_the_engines() {
    let reg = Arc::new(jcj_engine::registry_with(note_specs()));
    assert!(jcj_engine::duplicate_ids(reg.all()).is_empty());
    let mut s = jcj_engine::session_with(Document::new(), reg);
    assert_eq!(s.run("contract.note", &json!({"text": "hi"})).unwrap(), json!("hi"));
    assert_eq!(NOTE.get_or_none(&s.doc).as_deref(), Some("hi"));
    s.run("edit.undo", &json!({})).unwrap();
    assert!(!NOTE.is_set(&s.doc));
    assert!(matches!(s.run("contract.missing", &json!({})), Err(jcj_engine::CmdError::Unknown(_))));
    let mut twice = note_specs();
    twice.extend(note_specs());
    assert_eq!(jcj_engine::try_registry_with(twice).err(), Some(vec!["contract.note", "contract.readNote"]));
}

// ---- jcj-output ----------------------------------------------------------------------------

#[test]
fn output_writes_pages_and_images_of_the_layout() {
    let d = document(lines(60));
    let l = print_layout(&d, &mut LayoutCache::new());
    let all = jcj_output::export_pdf(&d, &l, &jcj_output::PdfSettings::deterministic()).unwrap();
    let first =
        jcj_output::export_pdf(&d, &l, &jcj_output::PdfSettings { pages: Some(vec![0]), ..jcj_output::PdfSettings::deterministic() }).unwrap();
    assert!(first.len() < all.len());
    let img = jcj_output::render_page(&d, &l, 1, 1.0, &jcj_output::RenderSettings::default()).unwrap();
    assert_eq!((img.width, img.height), (612, 792));
    assert_eq!(img.pixel(1, 1), Some([255, 255, 255, 255]));
}

// ---- jcj-proof -----------------------------------------------------------------------------

#[test]
fn proofing_marks_misspellings_and_hyphenates() {
    assert!(jcj_proof::is_correct("paragraph", None));
    assert!(!jcj_proof::is_correct("paragrahp", Some("en-US")));
    assert!(jcj_proof::suggestions("paragrahp", 3, None).iter().any(|s| s == "paragraph"));
    let issues = jcj_proof::check("One mispeled word.", None, &jcj_proof::SpellingOptions::default(), false);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].range, 4..12);
    let lim = jcj_proof::HyphenLimits::default();
    assert!(jcj_proof::hyphen_points("cat", None, &lim).is_empty());
    assert_eq!(jcj_proof::hyphenate("international", None).replace('-', ""), "international");
}

// ---- jcj-agents ----------------------------------------------------------------------------

#[test]
fn agents_see_the_apps_commands_as_tools() {
    let session = jcj_engine::session_with(Document::new(), Arc::new(jcj_engine::registry_with(note_specs())));
    let info = jcj_agents::AgentInfo::new("contract", "Contract", "0.0.0", "Tests.");
    let mut server = jcj_agents::AgentServer::headless(session, jcj_agents::AgentConfig::new(info).export("contract."));
    let list = server.handle(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})).unwrap();
    let names: Vec<&str> = list["result"]["tools"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"contract_note") && names.contains(&"execute"));
    let r = server
        .handle(
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "contract_note", "arguments": {"text": "from an agent"}}}),
        )
        .unwrap();
    assert_eq!(r["result"]["isError"], Value::Bool(false));
    let read =
        server.handle(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "contract_readNote", "arguments": {}}})).unwrap();
    assert_eq!(read["result"]["content"][0]["text"], "\"from an agent\"");
    assert_eq!(
        list["result"]["tools"].as_array().unwrap().iter().find(|t| t["name"] == "contract_readNote").unwrap()["annotations"]["readOnlyHint"],
        true
    );
}

#[test]
fn the_documents_story_api_reaches_the_body() {
    // The walk the adapters build on: body paragraphs in order with their paths.
    let d = document(vec![para("a"), Paragraph::new(), para("c")]);
    let ps = jcj_doc::walk::body_paragraphs(&d);
    assert_eq!(ps.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>(), vec![Path::top(0), Path::top(1), Path::top(2)]);
    assert!(d.story(StoryRef::Body).is_some());
}
