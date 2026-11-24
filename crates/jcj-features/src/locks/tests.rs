// SPDX-License-Identifier: MIT OR Apache-2.0
use jcj_engine::Selection;
use jcj_layout::paginate::{NoContinuations, paginate};
use serde_json::json;

use super::*;
use crate::pages::default_pages;
use crate::tests::session_with;

/// The number of one-line paragraphs that fill a page exactly.
fn per_page() -> usize {
    let lines: Vec<String> = (0..200).map(|i| format!("Line {i}.")).collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let s = session_with(&refs);
    let pages = default_pages(&s.doc);
    page_start(&pages[1]).map(|(b, _)| b).unwrap()
}

/// A three-page document of numbered one-line paragraphs (the last page part full).
fn session() -> (Session, usize) {
    let n = per_page();
    let lines: Vec<String> = (0..2 * n + n / 2).map(|i| format!("Line {i}.")).collect();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    (session_with(&refs), n)
}

fn labels(s: &Session) -> Vec<String> {
    label_pages(&s.doc, &default_pages(&s.doc)).map(|l| l.all()).unwrap_or_default()
}

/// The text each page starts with; placeholders show as "(placeholder)".
fn firsts(s: &Session) -> Vec<String> {
    let pages = default_pages(&s.doc);
    let text: Vec<String> = pages
        .iter()
        .map(|pg| page_start(pg).map(|(b, o)| s.doc.body[b].as_para().map_or(String::new(), |p| p.text[o..].to_string())).unwrap_or_default())
        .collect();
    match label_pages(&s.doc, &pages) {
        None => text,
        Some(l) => l
            .sequence
            .iter()
            .map(|e| match e {
                LabeledPage::Page { index, .. } => text[*index].clone(),
                LabeledPage::Placeholder { .. } => "(placeholder)".into(),
            })
            .collect(),
    }
}

fn start_of_page(s: &Session, page: usize) -> usize {
    page_start(&default_pages(&s.doc)[page]).map(|(b, _)| b).unwrap()
}

#[test]
fn letters_follow_the_scheme() {
    assert_eq!(letters(1), "A");
    assert_eq!(letters(26), "Z");
    assert_eq!(letters(27), "ZA");
    assert_eq!(letters(0), "A");
    assert_eq!(combined(&["10", "11"]), "10-11");
    assert_eq!(combined(&["7B", "8", "9"]), "7B-9");
}

#[test]
fn locked_pages_keep_their_numbers_and_overflow_takes_letters() {
    let (mut s, n) = session();
    let before = firsts(&s);
    assert_eq!(before.len(), 3);
    assert!(label_pages(&s.doc, &default_pages(&s.doc)).is_none(), "not locked");
    s.run("lock.pages", &json!({})).unwrap();
    assert_eq!(labels(&s), ["1", "2", "3"]);
    // More lines on page 1: they flow onto 1A; pages 2 and 3 start where they did.
    s.sel = Selection::caret(body_pos(5, 7));
    s.run("text.insert", &json!({"text": "\rMore.".repeat(n / 2)})).unwrap();
    assert_eq!(labels(&s), ["1", "1A", "2", "3"]);
    let after = firsts(&s);
    assert_eq!((&after[2], &after[3]), (&before[1], &before[2]));
    // Deleting most of page 1 does not pull page 2's text back.
    s.sel = Selection { anchor: body_pos(1, 0), focus: body_pos(n, 0) };
    s.run("text.backspace", &json!({})).unwrap();
    assert_eq!(labels(&s), ["1", "2", "3"]);
    assert_eq!(firsts(&s)[1], before[1]);
}

#[test]
fn deleted_pages_leave_a_placeholder_and_typing_at_a_page_start_stays_on_it() {
    let (mut s, _) = session();
    s.run("lock.pages", &json!({})).unwrap();
    let (start2, start3) = (start_of_page(&s, 1), start_of_page(&s, 2));
    // Typing at the very start of page 2 stays on page 2.
    s.sel = Selection::caret(body_pos(start2, 0));
    s.run("text.insert", &json!({"text": "Now "})).unwrap();
    assert!(firsts(&s)[1].starts_with("Now Line"), "{:?}", firsts(&s));
    // All of page 2's text deleted: a placeholder keeps its number.
    s.sel = Selection { anchor: body_pos(start2, 0), focus: body_pos(start3, 0) };
    s.run("text.backspace", &json!({})).unwrap();
    assert_eq!(labels(&s), ["1", "2", "3"]);
    assert_eq!(firsts(&s)[1], "(placeholder)");
    let st = s.run("lock.status", &json!({})).unwrap();
    assert_eq!(st["placeholders"], json!([{"before": 1, "label": "2"}]));
    // Unlock all: numbered 1, 2 again, with no placeholder.
    s.run("lock.unlockAll", &json!({})).unwrap();
    assert!(label_pages(&s.doc, &default_pages(&s.doc)).is_none());
    assert_eq!(default_pages(&s.doc).len(), 2);
    assert!(lock_marks(&s.doc).is_empty());
}

#[test]
fn a_second_lock_level_and_unlocking_it() {
    let (mut s, n) = session();
    s.run("lock.pages", &json!({})).unwrap();
    s.sel = Selection::caret(body_pos(5, 7));
    s.run("text.insert", &json!({"text": "\rMore.".repeat(n / 2)})).unwrap();
    assert_eq!(labels(&s), ["1", "1A", "2", "3"]);
    // Lock again: 1A is locked at level 2; more text after it makes 1AA.
    s.run("lock.pages", &json!({"revisionMode": true})).unwrap();
    assert!(crate::revisions::Revisions::of_doc(&s.doc).mode, "locking can turn revision mode on");
    assert_eq!(Locks::of_doc(&s.doc).level(), 2);
    let start = start_of_page(&s, 1);
    s.sel = Selection::caret(body_pos(start + 1, 0));
    s.run("text.insert", &json!({"text": "Extra.\r".repeat(2 * n)})).unwrap();
    // Two pages' worth of paragraphs after 1A's first.
    assert_eq!(labels(&s)[..4], ["1", "1A", "1AA", "1AB"]);
    assert_eq!(labels(&s)[labels(&s).len() - 2..], ["2", "3"]);
    // Unlock one level: 1A is no longer locked, so the pages after 1 are 1A, 1B, 1C again.
    s.run("lock.unlockLevel", &json!({})).unwrap();
    assert_eq!(labels(&s)[..4], ["1", "1A", "1B", "1C"]);
    assert!(s.run("lock.unlockAll", &json!({})).is_ok());
    assert!(s.run("lock.unlockLevel", &json!({})).is_err(), "nothing locked");
}

#[test]
fn the_lock_pass_starts_pages_at_locks_in_the_output() {
    let (mut s, n) = session();
    s.run("lock.pages", &json!({})).unwrap();
    s.sel = Selection::caret(body_pos(5, 7));
    s.run("text.insert", &json!({"text": "\rMore.".repeat(n / 2)})).unwrap();
    let pag = paginate(&s.doc, &mut LockPass::new(NoContinuations));
    assert_eq!(pag.pages.len(), 4);
    assert!(pag.matches_plan);
    let plain = paginate(&s.doc, &mut NoContinuations);
    assert_eq!(plain.pages.len(), 3, "without the pass the text flows freely");
    let labels = label_pages(&s.doc, &pag.pages).map(|l| l.page_labels()).unwrap_or_default();
    let mut out = pag.doc.clone();
    crate::header_footer::apply(&mut out, Some(&labels), &crate::header_footer::NoFields);
    let header = |i: usize| -> String {
        let starts = crate::watermark::page_starts(&out);
        let sect = starts.get(i + 1).and_then(|n| out.body.get(n - 1)).and_then(|b| b.as_para()).and_then(|p| p.section.clone());
        let id = match sect {
            Some(sct) => sct.headers.default,
            None => out.last_section.headers.default,
        };
        id.and_then(|id| out.parts.get(&id)).and_then(|p| p.blocks.first()).and_then(|b| b.as_para()).map(|p| p.text.clone()).unwrap_or_default()
    };
    assert_eq!(header(0), "", "no number on page 1");
    assert_eq!((header(1).as_str(), header(2).as_str(), header(3).as_str()), ("1A.", "2.", "3."));
}

#[test]
fn page_tools_move_relabel_add_and_remove_locked_pages() {
    let (mut s, _) = session();
    s.run("lock.pages", &json!({})).unwrap();
    let start2 = firsts(&s)[1].clone();
    // On page 2: its start moves up a paragraph (page 1 is full: that line goes to page 2), then back.
    let b2 = start_of_page(&s, 1);
    s.sel = Selection::caret(body_pos(b2 + 1, 0));
    s.run("lock.adjustBreak", &json!({"by": -1})).unwrap();
    assert_eq!(firsts(&s)[1], s.doc.body[b2 - 1].as_para().map(|p| p.text.clone()).unwrap_or_default());
    s.run("lock.adjustBreak", &json!({"by": 1})).unwrap();
    assert_eq!(firsts(&s)[1], start2);
    // Moving by nothing, or by something that is not 1 or -1, is refused and changes nothing.
    for by in [json!(0), json!("down"), json!(0.5), json!(2)] {
        assert!(matches!(s.run("lock.adjustBreak", &json!({ "by": by })), Err(CmdError::Params(_))), "{by}");
    }
    assert_eq!(firsts(&s)[1], start2);
    assert_eq!(labels(&s), ["1", "2", "3"]);
    // Its label by hand.
    s.run("lock.setLabel", &json!({"label": "2X"})).unwrap();
    assert_eq!(labels(&s), ["1", "2X", "3"]);
    assert!(s.run("lock.setLabel", &json!({"label": "3"})).is_err(), "labels stay unique");
    // A new locked page in the middle of page 1.
    s.sel = Selection::caret(body_pos(10, 0));
    assert_eq!(s.run("lock.lockPage", &json!({})).unwrap()["label"], "1A");
    assert_eq!(labels(&s)[..2], ["1", "1A"]);
    // And unlocked again: its text flows back.
    s.run("lock.unlockPage", &json!({})).unwrap();
    assert_eq!(labels(&s), ["1", "2X", "3"]);
    s.sel = Selection::caret(body_pos(0, 0));
    assert!(s.run("lock.unlockPage", &json!({})).is_err(), "the first locked page stays locked");
}

#[test]
fn hostile_lock_data_reads_safely() {
    let mut doc = Document::new();
    doc.ext.set("core.page_locks", json!({"pages": [{"id": 1, "label": "1<b>", "level": 1}]})).unwrap();
    assert_eq!(Locks::of_doc(&doc).pages[0].label, "1b");
    doc.ext.set("core.page_locks", json!({"pages": "all"})).unwrap();
    assert!(!Locks::of_doc(&doc).is_locked());
}

#[test]
fn deleting_a_locked_pages_first_words_keeps_the_page_start() {
    let (mut s, _) = session();
    s.run("lock.pages", &json!({})).unwrap();
    let start2 = start_of_page(&s, 1);
    let before = firsts(&s);
    // "Line " goes: the lock moves onto what is left of the paragraph.
    s.sel = Selection { anchor: body_pos(start2, 0), focus: body_pos(start2, 5) };
    s.run("text.delete", &json!({})).unwrap();
    assert_eq!(labels(&s), ["1", "2", "3"]);
    assert_eq!(firsts(&s)[1], before[1]["Line ".len()..]);
    assert_eq!(lock_marks(&s.doc).values().filter(|(b, o)| *b == start2 && *o == 0).count(), 1);
    // Typing in the middle of a locked page does not spread the mark.
    s.sel = Selection::caret(body_pos(start2, 2));
    s.run("text.insert", &json!({"text": "zz"})).unwrap();
    let p = s.doc.body[start2].as_para().unwrap();
    assert!(!PAGE_LOCK.is_set(p.props_of_char(2)));
}
