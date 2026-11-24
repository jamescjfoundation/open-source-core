// SPDX-License-Identifier: MIT OR Apache-2.0
use std::sync::Arc;

use jcj_doc::props::{CharProps, LineSpacing};
use jcj_doc::{Document, Paragraph, Path, Pos, StoryRef, para_block};

use crate::measure::{MeasureCache, line_ranges, measure, measure_cached, measure_text};
use crate::paginate::{Continuation, NoContinuations, PageBreak, PaginationPass, Piece, paginate};
use crate::{LayoutCache, hit, page_boxes, page_count, pages, print_layout};

/// Text written for these tests.
const WORDS: &str = "The quick brown fox jumps over the lazy dog while the patient owl counts the stars above the quiet valley. ";

/// A document on a bundled font with single spacing and no space between paragraphs, so line
/// heights are the same everywhere.
fn doc_of(paras: Vec<Paragraph>) -> Document {
    let mut d = Document::new();
    let st = Arc::make_mut(&mut d.styles);
    st.default_chr.font = Some("Source Serif 4".into());
    st.default_chr.size = Some(12.0);
    st.default_para.space_after = Some(0.0);
    st.default_para.line_spacing = Some(LineSpacing::Multiple(1.0));
    d.body = paras.into_iter().map(para_block).collect();
    d
}

fn para(text: &str) -> Paragraph {
    Paragraph::with_text(text, CharProps::default())
}

fn lines_doc(n: usize) -> Document {
    doc_of((0..n).map(|i| para(&format!("Line {i}"))).collect())
}

#[test]
fn a_long_document_has_several_letter_pages() {
    let d = lines_doc(120);
    let l = print_layout(&d, &mut LayoutCache::new());
    assert!(page_count(&l) >= 2, "{} pages", page_count(&l));
    let boxes = page_boxes(&l);
    assert_eq!(boxes.len(), page_count(&l));
    let b = boxes[0];
    assert_eq!((b.page.w, b.page.h), (d.last_section.page_w, d.last_section.page_h));
    assert!((b.body.x - d.last_section.margin_left).abs() < 0.01);
    assert_eq!(boxes[1].number, 2);
}

#[test]
fn a_caret_and_a_click_agree() {
    let d = doc_of(vec![para("Hello there"), para("Second paragraph")]);
    let l = print_layout(&d, &mut LayoutCache::new());
    let pos = Pos::body(1, 3);
    let c = hit::caret_rect(&l, &pos).unwrap();
    assert_eq!(c.page, 0);
    assert!(c.rect.h > 4.0 && c.rect.w == 0.0);
    // A click in the middle of the caret finds the same place.
    let back = hit::hit(&l, 0, c.rect.x, c.rect.y + c.rect.h / 2.0).unwrap();
    assert_eq!(back, pos);
    assert_eq!(hit::page_of(&l, &pos), Some(0));
    assert_eq!(hit::story_at(&l, 0, c.rect.x, c.rect.y + 1.0), Some(StoryRef::Body));
    let rects = hit::selection_rects(&l, &d, &Pos::body(0, 0), &Pos::body(1, 4));
    assert!(rects.len() >= 2);
    assert!(hit::hit(&l, 99, 0.0, 0.0).is_none());
}

#[test]
fn measuring_from_a_layout_and_on_its_own_agree() {
    let d = doc_of(vec![para(&WORDS.repeat(4)), para("Short")]);
    let l = print_layout(&d, &mut LayoutCache::new());
    let full = measure(&d, &l);
    let mut cache = MeasureCache::new();
    let alone = measure_cached(&d, &mut cache);
    assert_eq!(full.len(), 2);
    assert_eq!(full[0].lines, alone[0].lines);
    assert!(full[0].lines.len() > 2);
    assert_eq!(cache.len(), 2);
    // Every byte of the paragraph is on exactly one line, in order.
    let lr = &full[0].lines;
    assert_eq!(lr.first().unwrap().0, 0);
    assert!(lr.windows(2).all(|w| w[0].1 <= w[1].0));
    let pl = crate::measure::body_paragraph_layout(&l, 0).unwrap();
    assert_eq!(line_ranges(&pl), *lr);
    // The rest of a paragraph measured as its own text.
    let (lines, heights) = measure_text(&d, d.body[0].as_para().unwrap(), "A few words");
    assert_eq!(lines.len(), 1);
    assert_eq!(heights.len(), 1);
}

#[test]
fn page_pieces_mark_a_paragraph_split_across_pages() {
    let mut paras: Vec<Paragraph> = (0..25).map(|i| para(&format!("Line {i}"))).collect();
    paras.push(para(&WORDS.repeat(20)));
    let d = doc_of(paras);
    let l = print_layout(&d, &mut LayoutCache::new());
    let ps = pages::body_pieces(&l);
    assert!(ps.len() >= 2);
    let end = ps[0].last().unwrap();
    let start = ps[1].first().unwrap();
    assert_eq!(end.block, 25);
    assert!(end.continues && start.continued);
    assert_eq!(end.bytes.as_ref().unwrap().end, start.bytes.as_ref().unwrap().start);
    assert_eq!(pages::page_ends(&l).len(), ps.len());
}

#[test]
fn without_a_pass_pagination_matches_the_plan() {
    let d = lines_doc(130);
    let p = paginate(&d, &mut NoContinuations);
    assert!(p.matches_plan, "{} planned, {} laid out", p.pages.len(), p.layout.pages.len());
    assert!(p.pages.len() >= 2);
    // Every source paragraph is shown exactly once, in order.
    let blocks: Vec<usize> = p.map.iter().flatten().map(|s| s.block).collect();
    assert_eq!(blocks, (0..130).collect::<Vec<_>>());
    let starts = p.page_starts();
    assert_eq!(starts[0], 0);
    assert_eq!(p.to_output(&Pos::body(57, 2)).and_then(|o| p.to_source(&o)), Some(Pos::body(57, 2)));
}

/// Adds "(continued)" at the foot of a page whose last paragraph goes on, and the paragraph's
/// first word at the head of the next page.
struct Marks;

impl PaginationPass for Marks {
    fn continuation(&self, doc: &Document, at: &PageBreak<'_>) -> Continuation {
        if !at.splits_paragraph() {
            return Continuation::default();
        }
        let first =
            at.before.and_then(|b| doc.body.get(b.block)).and_then(|b| b.as_para()).map(|p| p.text.split(' ').next().unwrap_or("").to_string());
        Continuation { foot: vec![para("(continued)")], head: vec![para(&format!("{} (more)", first.unwrap_or_default()))] }
    }
}

#[test]
fn a_pass_adds_lines_where_a_paragraph_breaks() {
    let mut paras: Vec<Paragraph> = (0..25).map(|i| para(&format!("Line {i}"))).collect();
    paras.push(para(&WORDS.repeat(20)));
    paras.push(para("After"));
    let d = doc_of(paras);
    let p = paginate(&d, &mut Marks);
    assert!(p.matches_plan);
    let page0 = &p.pages[0];
    assert!(matches!(page0.last(), Some(Piece::Added { foot: true, paragraph }) if paragraph.text == "(continued)"));
    assert!(matches!(p.pages[1].first(), Some(Piece::Added { foot: false, paragraph }) if paragraph.text == "The (more)"));
    // The added lines have no source; a click on one goes to the next real text.
    let foot_out = p.page_starts()[1] - 1;
    assert_eq!(p.map[foot_out], None);
    let src = p.to_source(&Pos { story: StoryRef::Body, path: Path::top(foot_out), off: 0 }).unwrap();
    assert_eq!(src.path, Path::top(25));
    assert!(src.off > 0);
    // The source document is untouched.
    assert_eq!(d.body.len(), 27);
}

#[test]
fn keep_with_next_moves_a_paragraph_to_its_partner() {
    let mut paras: Vec<Paragraph> = (0..200).map(|i| para(&format!("Line {i}"))).collect();
    let l = print_layout(&doc_of(paras.clone()), &mut LayoutCache::new());
    // The last line of page 1, kept with the next: it goes to page 2.
    let last = pages::body_pieces(&l)[0].last().unwrap().block;
    paras[last].props.keep_next = Some(true);
    let d = doc_of(paras);
    let p = paginate(&d, &mut NoContinuations);
    assert!(p.matches_plan);
    let Piece::Source { part, .. } = p.pages[1].first().unwrap() else { panic!("page 2 starts with an added line") };
    assert_eq!(part.block, last);
}

#[test]
fn page_break_before_starts_a_page() {
    let mut paras: Vec<Paragraph> = (0..5).map(|i| para(&format!("Line {i}"))).collect();
    paras[3].props.page_break_before = Some(true);
    let p = paginate(&doc_of(paras), &mut NoContinuations);
    assert_eq!(p.pages.len(), 2);
    assert!(matches!(p.pages[1].first(), Some(Piece::Source { part, .. }) if part.block == 3));
    assert!(p.matches_plan);
}
