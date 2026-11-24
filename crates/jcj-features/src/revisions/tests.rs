// SPDX-License-Identifier: MIT OR Apache-2.0
use serde_json::json;

use super::*;
use crate::data::body_pos;
use crate::tests::session_with;

fn session() -> Session {
    session_with(&["A heading", "Rain hits the window.", "A third line.", "Hello there."])
}

/// Each run of a paragraph: (text, revision set).
fn runs(s: &Session, block: usize) -> Vec<(String, Option<u32>)> {
    let p = s.doc.body[block].as_para().unwrap();
    p.run_ranges().map(|(r, c)| (p.text[r].to_string(), REVISED.get_or_none(c))).collect()
}

#[test]
fn the_cycle_names_and_colors() {
    let c = default_cycle();
    assert_eq!(cycle_set(&c, 1).name, "Blue");
    assert_eq!(cycle_set(&c, 9).name, "Tan");
    assert_eq!(cycle_set(&c, 10).name, "Second White");
    assert_eq!(cycle_set(&c, 11).name, "Second Blue");
    assert_eq!(cycle_set(&c, 25).name, "Third Goldenrod");
    assert!(DEFAULT_CYCLE.iter().all(|(_, c)| parse_color(c).is_some()));
    let mut r = Revisions::default();
    assert_eq!((r.start_next(), r.current.as_str()), (Some(0), "Blue"));
    assert_eq!((r.start_next(), r.current.as_str()), (Some(1), "Pink"));
    assert_eq!(cycle_set(&[], 3).name, "Fourth White", "an empty cycle still names sets");
}

#[test]
fn an_app_cycle_replaces_the_default() {
    let mut s = session();
    s.run("revision.configure", &json!({"cycle": [{"name": "Draft", "color": "#ffffff"}, {"name": "Amber", "color": "#ffbf00"}]})).unwrap();
    s.run("revision.next", &json!({})).unwrap();
    assert_eq!(Revisions::of_doc(&s.doc).current, "Amber");
    s.run("revision.next", &json!({})).unwrap();
    assert_eq!(Revisions::of_doc(&s.doc).current, "Second Draft");
    assert!(s.run("revision.configure", &json!({"cycle": [{"name": "X", "color": "nope"}]})).is_err());
    s.run("revision.configure", &json!({"cycle": null})).unwrap();
    assert!(Revisions::of_doc(&s.doc).cycle.is_empty(), "null goes back to the default");
}

#[test]
fn typing_and_deleting_in_revision_mode() {
    let mut s = session();
    s.sel = Selection::caret(body_pos(1, 4));
    s.run("text.insert", &json!({"text": "!"})).unwrap();
    assert!(runs(&s, 1).iter().all(|(_, r)| r.is_none()), "out of revision mode nothing is marked");
    s.run("revision.mode", &json!({"on": true})).unwrap();
    assert_eq!(Revisions::of_doc(&s.doc).current, "Blue", "turning it on starts Blue");
    s.sel = Selection::caret(body_pos(1, 0));
    s.run("text.insert", &json!({"text": "Cold "})).unwrap();
    assert_eq!(runs(&s, 1)[0], ("Cold ".to_string(), Some(0)));
    // A deletion marks its paragraph.
    s.sel = Selection::caret(body_pos(3, 5));
    s.run("text.backspace", &json!({})).unwrap();
    assert_eq!(REVISED.get_or_none(&s.doc.body[3].as_para().unwrap().mark), Some(0));
    // The next revision is Pink, and typing takes it.
    s.run("revision.next", &json!({})).unwrap();
    s.sel = Selection::caret(body_pos(3, 0));
    s.run("text.insert", &json!({"text": "Oh. "})).unwrap();
    assert_eq!(runs(&s, 3)[0], ("Oh. ".to_string(), Some(1)));
    // Out of revision mode again, typing next to revised text is not revised.
    s.run("revision.mode", &json!({})).unwrap();
    s.sel = Selection::caret(body_pos(3, 4));
    s.run("text.insert", &json!({"text": "x"})).unwrap();
    assert_eq!(runs(&s, 3)[1], ("xHell there.".to_string(), None));
    assert_eq!(collated(&s.doc), "Blue, Pink");
}

#[test]
fn mark_clear_step_and_roll_back() {
    let mut s = session();
    s.sel = Selection { anchor: body_pos(1, 0), focus: body_pos(1, 4) };
    s.run("revision.markRevised", &json!({})).unwrap();
    assert_eq!(runs(&s, 1)[0], ("Rain".to_string(), Some(0)));
    s.run("revision.next", &json!({})).unwrap();
    s.sel = Selection::caret(body_pos(3, 0));
    s.run("revision.markRevised", &json!({})).unwrap();
    assert_eq!(runs(&s, 3), [("Hello there.".to_string(), Some(1))]);
    // Stepping goes round the revised stretches.
    s.sel = Selection::caret(body_pos(0, 0));
    s.run("revision.gotoNext", &json!({})).unwrap();
    assert_eq!(s.selected_text(), "Rain");
    s.run("revision.gotoNext", &json!({})).unwrap();
    assert_eq!(s.selected_text(), "Hello there.");
    s.run("revision.gotoNext", &json!({})).unwrap();
    assert_eq!(s.selected_text(), "Rain", "wraps");
    // Roll back to Blue: Pink's marks go, Blue's stay.
    s.run("revision.rollback", &json!({"to": "Blue"})).unwrap();
    assert_eq!(runs(&s, 3)[0].1, None);
    assert_eq!(REVISED.get_or_none(&s.doc.body[3].as_para().unwrap().mark), None);
    assert_eq!(runs(&s, 1)[0].1, Some(0));
    assert_eq!(Revisions::of_doc(&s.doc).sets.len(), 1);
    // Clear the paragraph's marks, then all.
    s.sel = Selection::caret(body_pos(1, 2));
    s.run("revision.clearRevised", &json!({})).unwrap();
    assert!(runs(&s, 1).iter().all(|(_, r)| r.is_none()));
    s.run("revision.rollback", &json!({"to": ""})).unwrap();
    assert!(Revisions::of_doc(&s.doc).sets.is_empty());
    assert!(s.run("revision.rollback", &json!({"to": "Mauve"})).is_err());
    assert!(s.run("revision.gotoNext", &json!({})).is_err(), "nothing is revised");
}

#[test]
fn settings_are_bounded_and_configurable() {
    let mut s = session();
    s.run("revision.next", &json!({})).unwrap();
    s.run(
        "revision.configure",
        &json!({"mark": "#", "markAt": 7.5, "set": {"index": 0, "name": "Blue Draft", "color": "#3366ff", "underline": true}}),
    )
    .unwrap();
    let r = Revisions::of_doc(&s.doc);
    assert_eq!((r.mark.as_str(), r.mark_at, r.current.as_str()), ("#", 7.5, "Blue Draft"));
    assert!(r.sets[0].underline);
    assert!(s.run("revision.configure", &json!({"markAt": 40})).is_err());
    assert!(s.run("revision.configure", &json!({"set": {"index": 0, "color": "blue"}})).is_err());
    let mut doc = Document::new();
    doc.ext.set("core.revisions", json!({"sets": [{"name": "X", "color": "nope"}], "current": "Y", "mark": "  ", "markAt": 99})).unwrap();
    let r = Revisions::of_doc(&doc);
    assert_eq!((r.sets[0].color.as_str(), r.current.as_str(), r.mark.as_str(), r.mark_at), ("#FFFFFF", "", "*", 8.0));
}

#[test]
fn output_takes_set_styles_and_page_colors() {
    let mut s = session();
    s.run("revision.next", &json!({})).unwrap();
    s.run("revision.configure", &json!({"colorPages": true, "set": {"index": 0, "bold": true}})).unwrap();
    s.sel = Selection { anchor: body_pos(1, 0), focus: body_pos(1, 4) };
    s.run("revision.markRevised", &json!({})).unwrap();
    let pages = crate::pages::default_pages(&s.doc);
    assert_eq!(page_sets(&s.doc, &pages), vec![Some(0)]);
    let mut out = s.doc.clone();
    decorate_output(&mut out);
    let p = out.body[1].as_para().unwrap();
    assert_eq!(p.run_ranges().next().map(|(_, c)| c.bold), Some(Some(true)));
    let fills = out.body[0].as_para().unwrap().objects.iter().filter(|o| matches!(o, InlineObject::Shape { fill: Some(_), .. })).count();
    assert_eq!(fills, 1, "the page is colored Blue");
}
