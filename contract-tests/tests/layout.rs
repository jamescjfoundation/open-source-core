// SPDX-License-Identifier: MIT OR Apache-2.0
//! Layout facts the apps rely on, each on a document built in code: keep with next, widow and
//! orphan control, page break before, footnotes, drop caps. These test the layout engine itself
//! (through `jcj-layout`), not the core's pagination pass.

use contract_tests::{document, lines, lines_per_page, long_para, para};
use jcj_doc::{Paragraph, PartKind, Pos, StoryRef};
use jcj_engine::Selection;
use jcj_layout::pages::{PagePiece, body_pieces};
use jcj_layout::{DocLayout, LayoutCache, Placed, hit, print_layout};
use serde_json::json;

fn lay(paras: Vec<Paragraph>) -> DocLayout {
    print_layout(&document(paras), &mut LayoutCache::new())
}

fn page_of_block(l: &DocLayout, block: usize) -> usize {
    hit::page_of(l, &Pos::body(block, 0)).unwrap()
}

/// The lines of body block `block` on each page, in page order.
fn lines_by_page(l: &DocLayout, block: usize) -> Vec<usize> {
    body_pieces(l).iter().flatten().filter(|p: &&PagePiece| p.block == block).map(|p| p.lines.as_ref().map_or(0, |r| r.len())).collect()
}

#[test]
fn the_fixture_page_holds_a_known_number_of_lines() {
    // 12 pt Source Serif 4, single spaced, in 9 inches of body height. It changes only when line
    // height or page size does: a drift that moves every page break in every app.
    assert_eq!(lines_per_page(), 39);
}

#[test]
fn keep_with_next_takes_a_paragraph_to_the_next_page() {
    let n = lines_per_page();
    let mut paras = lines(n + 5);
    // The last line of page 1 keeps with the first of page 2.
    paras[n - 1].props.keep_next = Some(true);
    let l = lay(paras.clone());
    assert_eq!(page_of_block(&l, n - 1), 1, "kept with its next paragraph");
    assert_eq!(page_of_block(&l, n - 2), 0);
    // Without it the page is full.
    paras[n - 1].props.keep_next = Some(false);
    assert_eq!(page_of_block(&lay(paras), n - 1), 0);
}

#[test]
fn keep_with_next_chains_move_together() {
    let n = lines_per_page();
    let mut paras = lines(n + 5);
    for p in paras.iter_mut().take(n).skip(n - 3) {
        p.props.keep_next = Some(true);
    }
    let l = lay(paras);
    for b in n - 3..n {
        assert_eq!(page_of_block(&l, b), 1, "block {b}");
    }
}

#[test]
fn orphan_control_moves_a_lone_first_line() {
    let n = lines_per_page();
    // One line of room is left on page 1 for a paragraph of several lines.
    let mut paras = lines(n - 1);
    let mut long = long_para(6);
    long.props.widow_control = Some(true);
    paras.push(long.clone());
    let l = lay(paras.clone());
    let split = lines_by_page(&l, n - 1);
    assert!(split[0] == 0 || split[0] >= 2, "with widow control no single first line is left: {split:?}");
    long.props.widow_control = Some(false);
    *paras.last_mut().unwrap() = long;
    assert_eq!(lines_by_page(&lay(paras), n - 1)[0], 1, "without it the first line stays");
}

#[test]
fn widow_control_carries_two_lines() {
    let n = lines_per_page();
    let mut long = long_para(6);
    long.props.widow_control = Some(false);
    let total = lines_by_page(&lay(vec![long.clone()]), 0)[0];
    assert!(total >= 4, "{total} lines");
    // Room for all but one line of the paragraph on page 1.
    let mut paras = lines(n - (total - 1));
    paras.push(long.clone());
    let at = paras.len() - 1;
    assert_eq!(lines_by_page(&lay(paras.clone()), at), vec![total - 1, 1], "without widow control one line is carried");
    long.props.widow_control = Some(true);
    paras[at] = long;
    assert_eq!(lines_by_page(&lay(paras), at), vec![total - 2, 2], "with it two are");
}

#[test]
fn page_break_before_starts_a_new_page() {
    let mut paras = lines(6);
    paras[3].props.page_break_before = Some(true);
    let l = lay(paras);
    assert_eq!(l.pages.len(), 2);
    assert_eq!(page_of_block(&l, 2), 0);
    assert_eq!(page_of_block(&l, 3), 1);
    assert_eq!(body_pieces(&l)[1][0].block, 3);
}

#[test]
fn a_footnote_is_laid_out_at_the_foot_of_its_page() {
    let mut s = jcj_engine::session(document(vec![para("Text with a note."), para("More text.")]));
    s.sel = Selection::caret(Pos::body(0, "Text with a note".len()));
    s.run("references.footnote", &json!({"text": "The note itself."})).unwrap();
    let doc = s.doc.clone();
    let (&id, _) = doc.parts.iter().find(|(_, p)| p.kind == PartKind::Footnote).expect("a footnote part");
    let l = print_layout(&doc, &mut LayoutCache::new());
    let page = &l.pages[0];
    let note_y = page.items.iter().find_map(|it| match it {
        Placed::Lines { story: StoryRef::Part(p), y, .. } if *p == id => Some(*y),
        _ => None,
    });
    let note_y = note_y.expect("the note is on the page of its reference");
    let body_y = page
        .items
        .iter()
        .filter_map(|it| match it {
            Placed::Lines { story: StoryRef::Body, y, .. } => Some(*y),
            _ => None,
        })
        .fold(0.0f32, f32::max);
    assert!(note_y > body_y, "the note ({note_y}) is below the body text ({body_y})");
    assert!(note_y > page.body.y + page.body.h / 2.0, "in the lower half of the page");
    // The note's text can be hit and has a caret.
    let pos = Pos { story: StoryRef::Part(id), path: jcj_doc::Path::top(0), off: 0 };
    assert_eq!(hit::caret_rect(&l, &pos).map(|c| c.page), Some(0));
}

#[test]
fn a_drop_cap_drops_into_the_lines_below() {
    let mut p = long_para(3);
    p.props.drop_cap = Some(3);
    let l = lay(vec![p]);
    let pl = jcj_layout::measure::body_paragraph_layout(&l, 0).unwrap();
    let (clusters, dropped, indent) = pl.drop_cap.expect("a drop cap");
    assert_eq!(dropped, 3);
    assert!(clusters >= 1);
    assert!(indent > 0.0);
    // The lines beside the letter are indented; the line after them is not.
    let left = |i: usize| pl.lines[i].left;
    assert!(left(1) > left(3) + 1.0, "line 2 at {} and line 4 at {}", left(1), left(3));
    // Without the drop cap no line is indented.
    let plain = lay(vec![long_para(3)]);
    let pl2 = jcj_layout::measure::body_paragraph_layout(&plain, 0).unwrap();
    assert!(pl2.drop_cap.is_none());
    assert!((pl2.lines[1].left - pl2.lines[3].left).abs() < 0.01);
}
