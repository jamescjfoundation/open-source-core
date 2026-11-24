// SPDX-License-Identifier: MIT OR Apache-2.0
//! Model round trips: open, edit, undo, save as native JSON, reopen. Extension data on the
//! document, a paragraph and a run survives each step.

use contract_tests::{document, para};
use jcj_doc::{ExtKey, HasExt, Pos, StoryRef};
use jcj_engine::{Selection, native};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Label {
    name: String,
    level: u8,
}

const DOC_KEY: ExtKey<Label> = ExtKey::new("contract.document");
const PARA_KEY: ExtKey<Label> = ExtKey::new("contract.paragraph");
const RUN_KEY: ExtKey<String> = ExtKey::new("contract.run");

fn labelled() -> jcj_doc::Document {
    let mut first = para("First paragraph text.");
    PARA_KEY.set(&mut first, &Label { name: "opening".into(), level: 1 }).unwrap();
    RUN_KEY.set(&mut first.runs[0], &"run value".to_string()).unwrap();
    let mut d = document(vec![first, para("Second paragraph.")]);
    DOC_KEY.set(&mut d, &Label { name: "whole".into(), level: 0 }).unwrap();
    d
}

fn assert_labels(d: &jcj_doc::Document) {
    assert_eq!(DOC_KEY.get(d).unwrap(), Some(Label { name: "whole".into(), level: 0 }));
    let p = d.body[0].as_para().unwrap();
    assert_eq!(PARA_KEY.get(p).unwrap(), Some(Label { name: "opening".into(), level: 1 }));
    assert_eq!(RUN_KEY.get_or_none(&p.runs[0]).as_deref(), Some("run value"));
    assert!(!PARA_KEY.is_set(d.body[1].as_para().unwrap()));
}

#[test]
fn native_json_round_trips_the_model_and_its_extension_data() {
    let d = labelled();
    let bytes = native::save(&d).unwrap();
    let back = native::open(&bytes).unwrap();
    assert!(back.same_content(&d), "the reopened document has the same content");
    assert_labels(&back);
    // Saving the reopened document gives the same bytes.
    assert_eq!(native::save(&back).unwrap(), bytes);
}

#[test]
fn open_edit_undo_save_reopen() {
    let bytes = native::save(&labelled()).unwrap();
    let mut s = jcj_engine::session(native::open(&bytes).unwrap());
    let original = s.doc.body[0].as_para().unwrap().text.clone();

    // Edit: type into the labelled paragraph and its labelled run.
    s.sel = Selection::caret(Pos::body(0, 5));
    s.run("text.insert", &json!({"text": " inserted"})).unwrap();
    assert_eq!(s.doc.body[0].as_para().unwrap().text, "First inserted paragraph text.");
    assert!(s.dirty);
    // Typing keeps the paragraph's data, and the run's on the text it had.
    let p = s.doc.body[0].as_para().unwrap();
    assert!(PARA_KEY.is_set(p));
    assert!(p.run_ranges().any(|(r, c)| r.start == 0 && c.ext.contains("contract.run")));

    // An extension edit through the model, recorded as its own undo step.
    s.checkpoint("Relabel");
    DOC_KEY.set(&mut s.doc, &Label { name: "changed".into(), level: 9 }).unwrap();

    // Undo both, in order.
    assert!(s.undo());
    assert_eq!(DOC_KEY.get_or_none(&s.doc).map(|l| l.name).as_deref(), Some("whole"));
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc.body[0].as_para().unwrap().text, original);
    assert_labels(&s.doc);

    // Redo the typing, save, reopen.
    s.run("edit.redo", &json!({})).unwrap();
    let saved = native::save(&s.doc).unwrap();
    let back = native::open(&saved).unwrap();
    assert_eq!(back.body[0].as_para().unwrap().text, "First inserted paragraph text.");
    assert_labels(&back);
    assert_eq!(back.para(StoryRef::Body, &jcj_doc::Path::top(1)).unwrap().text, "Second paragraph.");
}

#[test]
fn extension_data_is_compared_and_copied_with_the_model() {
    let a = labelled();
    let mut b = a.clone();
    assert!(a.same_content(&b));
    DOC_KEY.set(&mut b, &Label { name: "other".into(), level: 2 }).unwrap();
    assert!(!a.same_content(&b), "a change to extension data is a change to the document");
    let mut c = a.clone();
    DOC_KEY.remove(&mut c);
    assert!(!c.ext().contains("contract.document"));
    assert!(a.ext().contains("contract.document"));
}
