// SPDX-License-Identifier: MIT OR Apache-2.0
use std::sync::Arc;

use jcj_doc::props::{Align, CharProps};
use jcj_doc::{Document, ExtKey, Path, Pos, StoryRef};
use serde_json::{Value, json};

use super::find::{Query, find_all, find_in, replace_all_with};
use super::spelling::{DOCUMENT_WORDS, DocumentWords, misspellings, misspellings_with};
use super::{merged, specs};
use crate::{CmdError, CommandSpec, Selection, Session, duplicate_ids, engine_specs, native, registry_with, session_with};

const LABEL: ExtKey<String> = ExtKey::new("test.label");
const MARK: ExtKey<u32> = ExtKey::new("test.mark");

fn session() -> Session {
    session_with(Document::new(), Arc::new(registry_with(merged(Vec::new()))))
}

/// A session whose body holds `lines`, one paragraph each, with the caret at the start.
fn doc(lines: &str) -> Session {
    let mut s = session();
    s.run("text.insert", &json!({"text": lines, "paragraphs": true})).unwrap();
    s.sel = Selection::caret(Pos::body(0, 0));
    s
}

fn body(s: &Session) -> Vec<String> {
    s.doc.para_paths(StoryRef::Body).iter().filter_map(|p| s.doc.para(StoryRef::Body, p)).map(|p| p.text.clone()).collect()
}

fn select(s: &mut Session, block: usize, from: usize, to: usize) {
    s.sel = Selection { anchor: Pos::body(block, from), focus: Pos::body(block, to) };
}

fn char_at(s: &Session, block: usize, off: usize) -> CharProps {
    s.doc.para(StoryRef::Body, &Path::top(block)).map(|p| p.props_of_char(off).clone()).unwrap_or_default()
}

fn q(text: &str, match_case: bool, whole_word: bool) -> Query {
    Query { text: text.into(), match_case, whole_word, styles: Vec::new() }
}

// The registry.

#[test]
fn the_core_commands_are_new_ids_registered_once() {
    let ours = specs();
    assert!(duplicate_ids(&ours).is_empty());
    let engine: Vec<&str> = engine_specs().iter().map(|s| s.id).collect();
    // An upstream update that adds one of these ids would hide its command: rename ours then.
    for s in &ours {
        assert!(!engine.contains(&s.id), "{} is an engine id", s.id);
        assert!(["find.", "format.", "edit.", "select.", "spelling."].iter().any(|p| s.id.starts_with(p)), "{}", s.id);
    }
    let r = registry_with(merged(Vec::new()));
    assert!(duplicate_ids(r.all()).is_empty());
    assert!(r.get("find.replaceAll").is_some() && r.get("edit.undo").is_some());
}

#[test]
fn an_app_command_replaces_a_core_command() {
    let mine = CommandSpec::new("spelling.check", "My Check", "App", |_, _| Ok(json!("mine"))).pure();
    let all = merged(vec![mine]);
    assert!(duplicate_ids(&all).is_empty());
    let mut s = session_with(Document::new(), Arc::new(registry_with(all)));
    assert_eq!(s.run("spelling.check", &json!({})).unwrap(), json!("mine"));
    assert!(s.run("spelling.suggest", &json!({"word": "hous"})).is_ok());
}

// Find and replace.

#[test]
fn matching_rules() {
    let ranges = |t: &str, query: &Query| find_in(t, query).into_iter().map(|r| (r.start, r.end)).collect::<Vec<_>>();
    assert_eq!(ranges("Anna and anna and ANNAbel", &q("anna", false, false)), [(0, 4), (9, 13), (18, 22)]);
    assert_eq!(ranges("Anna and anna and ANNAbel", &q("anna", true, false)), [(9, 13)]);
    assert_eq!(ranges("Anna and anna and ANNAbel", &q("anna", false, true)), [(0, 4), (9, 13)]);
    assert_eq!(ranges("aaaa", &q("aa", false, false)), [(0, 2), (2, 4)], "no overlaps");
    assert_eq!(ranges("Café CAFÉ", &q("café", false, false)), [(0, 5), (6, 11)], "case beyond ASCII");
    assert_eq!(ranges("it's its", &q("it", false, true)), Vec::<(usize, usize)>::new(), "an apostrophe is part of a word");
    assert!(find_in("short", &q("longer text", false, false)).is_empty());
    assert!(find_in("anything", &q("", false, false)).is_empty());
}

#[test]
fn find_all_lists_matches_without_moving_the_selection() {
    let mut s = doc("one dog\ntwo dogs\nno cat");
    let r = s.run("find.all", &json!({"query": "dog"})).unwrap();
    assert_eq!(r["count"], 2);
    assert_eq!(r["matches"][1]["start"], crate::cmd::pos_json(&Pos::body(1, 4)));
    assert_eq!(s.sel, Selection::caret(Pos::body(0, 0)));
    assert_eq!(s.run("find.all", &json!({"query": "dog", "wholeWord": true})).unwrap()["count"], 1);
    for bad in [json!({}), json!({"query": ""}), json!({"query": "two\nlines"})] {
        assert!(matches!(s.run("find.all", &bad), Err(CmdError::Params(_))), "{bad}");
    }
}

#[test]
fn find_next_and_previous_wrap_round_the_body() {
    let mut s = doc("one dog\ntwo dog\nthree dog");
    let depth = s.undo_depth();
    let block = |s: &Session| s.sel.focus.path.last();
    let r = s.run("find.next", &json!({"query": "dog"})).unwrap();
    assert_eq!((block(&s), s.selected_text(), r["count"].clone()), (0, "dog".to_string(), json!(3)));
    s.run("find.next", &json!({"query": "dog"})).unwrap();
    assert_eq!(block(&s), 1);
    s.run("find.next", &json!({"query": "dog"})).unwrap();
    s.run("find.next", &json!({"query": "dog"})).unwrap();
    assert_eq!(block(&s), 0, "wraps to the start");
    s.run("find.previous", &json!({"query": "dog"})).unwrap();
    assert_eq!(block(&s), 2, "and back to the end");
    assert!(matches!(s.run("find.next", &json!({"query": "cat"})), Err(CmdError::Failed(_))));
    assert_eq!(s.undo_depth(), depth, "finding changes nothing");
}

#[test]
fn a_search_can_be_limited_to_paragraph_styles() {
    let mut s = doc("Dog heading\ndog text\nDog text");
    s.sel = Selection::caret(Pos::body(0, 0));
    s.run("para.style", &json!({"style": "Heading1"})).unwrap();
    let ids = |r: Value| r["matches"].as_array().unwrap().iter().map(|m| m["start"]["path"][0].as_u64().unwrap()).collect::<Vec<_>>();
    assert_eq!(ids(s.run("find.all", &json!({"query": "dog", "styles": ["Heading1"]})).unwrap()), [0]);
    // By name too, and the default style.
    assert_eq!(ids(s.run("find.all", &json!({"query": "dog", "styles": ["heading 1"]})).unwrap()), [0]);
    assert_eq!(ids(s.run("find.all", &json!({"query": "dog", "styles": ["Normal"]})).unwrap()), [1, 2]);
    s.run("find.next", &json!({"query": "dog", "styles": ["Normal"]})).unwrap();
    assert_eq!(s.sel.focus.path.last(), 1);
}

#[test]
fn replace_one_then_all_as_one_undo_step() {
    let mut s = doc("The dog barks.\nA dog and a dog. Dogma.");
    s.run("find.next", &json!({"query": "dog", "wholeWord": true})).unwrap();
    let r = s.run("find.replace", &json!({"query": "dog", "with": "cat", "wholeWord": true})).unwrap();
    assert_eq!(r["replaced"], true);
    assert_eq!(body(&s)[0], "The cat barks.");
    assert_eq!((s.sel.focus.path.last(), s.selected_text()), (1, "dog".to_string()), "the next match is selected");
    let r = s.run("find.replaceAll", &json!({"query": "dog", "with": "cat", "wholeWord": true})).unwrap();
    assert_eq!(r["replaced"], 2);
    assert_eq!(body(&s)[1], "A cat and a cat. Dogma.");
    assert_eq!(s.undo_label(), Some("Replace All"));
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(body(&s)[1], "A dog and a dog. Dogma.", "one undo step");
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(body(&s)[0], "The dog barks.");
    // An empty replacement deletes; a replacement on two lines is refused.
    assert_eq!(s.run("find.replaceAll", &json!({"query": "Dog", "with": "", "matchCase": true})).unwrap()["replaced"], 1);
    assert_eq!(body(&s)[1], "A dog and a dog. ma.");
    assert!(s.run("find.replaceAll", &json!({"query": "dog", "with": "a\nb"})).is_err());
    // Replace with nothing selected only moves to a match.
    s.sel = Selection::caret(Pos::body(0, 0));
    assert_eq!(s.run("find.replace", &json!({"query": "dog", "with": "cat"})).unwrap()["replaced"], false);
    assert_eq!(s.selected_text(), "dog");
}

#[test]
fn replacements_keep_formatting_and_are_tracked_while_changes_are_tracked() {
    let mut s = doc("A bold dog.");
    select(&mut s, 0, 7, 10);
    s.run("format.bold", &json!({"value": true})).unwrap();
    s.run("find.replaceAll", &json!({"query": "dog", "with": "wolf"})).unwrap();
    assert_eq!(body(&s)[0], "A bold wolf.");
    assert_eq!(char_at(&s, 0, 8).bold, Some(true), "the replaced text's formatting");
    s.doc.settings.track_changes = true;
    s.run("find.replaceAll", &json!({"query": "wolf", "with": "fox"})).unwrap();
    assert!(find_all(&s.doc, &q("wolf", false, false)).is_empty(), "deleted text is not found again");
    assert_eq!(find_all(&s.doc, &q("fox", false, false)).len(), 1);
    assert!(body(&s)[0].contains("wolf"), "the deletion is kept as a tracked change: {:?}", body(&s));
}

#[test]
fn an_app_chooses_each_replacement() {
    let mut s = doc("first dog\nsecond dog");
    let n = replace_all_with(&mut s, &q("dog", false, false), &|_, at| if at.path.last() == 0 { "CAT".into() } else { "cat".into() }).unwrap();
    assert_eq!(n, 2);
    assert_eq!(body(&s), ["first CAT", "second cat"]);
}

// Formatting.

#[test]
fn plain_text_removes_text_styles_but_keeps_font_size_and_marks() {
    let mut s = doc("Some marked text");
    select(&mut s, 0, 5, 11);
    s.run("format.font", &json!({"name": "Courier New"})).unwrap();
    s.run("format.size", &json!({"size": 14})).unwrap();
    for id in ["format.bold", "format.italic", "format.underline", "format.strikethrough", "format.superscript", "format.smallCaps"] {
        s.run(id, &json!({"value": true})).unwrap();
    }
    s.run("format.highlight", &json!({"color": "yellow"})).unwrap();
    s.run("format.color", &json!({"color": "FF0000"})).unwrap();
    let (a, b) = s.sel.ordered();
    s.doc.format_range(&a, &b, &|c| MARK.set(c, &7).unwrap()).unwrap();
    s.run("format.plain", &json!({})).unwrap();
    let c = char_at(&s, 0, 6);
    assert_eq!(
        (c.bold, c.italic, c.underline, c.strike, c.vert_align, c.small_caps, c.highlight, c.color),
        (None, None, None, None, None, None, None, None)
    );
    assert_eq!((c.font.as_deref(), c.size), (Some("Courier New"), Some(14.0)));
    assert_eq!(MARK.get_or_none(&c), Some(7));
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(char_at(&s, 0, 6).bold, Some(true));
    // At a caret between words it applies to what is typed next.
    s.sel = Selection::caret(Pos::body(0, 16));
    s.run("format.bold", &json!({"value": true})).unwrap();
    s.run("format.plain", &json!({})).unwrap();
    s.run("text.insert", &json!({"text": " more"})).unwrap();
    assert_eq!(char_at(&s, 0, 18).bold, None);
}

#[test]
fn reset_to_style_keeps_the_style_and_extension_data() {
    let mut s = doc("A heading\nNext");
    s.run("para.style", &json!({"style": "Heading1"})).unwrap();
    s.run("para.alignCenter", &json!({})).unwrap();
    select(&mut s, 0, 0, 1);
    s.run("format.bold", &json!({"value": true})).unwrap();
    let a = Pos::body(0, 0);
    let b = Pos::body(0, 1);
    s.doc.format_range(&a, &b, &|c| MARK.set(c, &3).unwrap()).unwrap();
    LABEL.set(s.doc.para_mut(StoryRef::Body, &Path::top(0)).unwrap(), &"kept".to_string()).unwrap();
    s.sel = Selection::caret(Pos::body(0, 4));
    s.run("format.resetToStyle", &json!({})).unwrap();
    let p = s.doc.para(StoryRef::Body, &Path::top(0)).unwrap();
    assert_eq!(p.props.style.as_deref(), Some("Heading1"));
    assert_eq!(p.props.align, None);
    assert_eq!(LABEL.get_or_none(p).as_deref(), Some("kept"));
    assert_eq!(p.props_of_char(0).bold, None);
    assert_eq!(MARK.get_or_none(p.props_of_char(0)), Some(3));
    s.run("edit.undo", &json!({})).unwrap();
    let p = s.doc.para(StoryRef::Body, &Path::top(0)).unwrap();
    assert_eq!((p.props.align, p.props_of_char(0).bold), (Some(Align::Center), Some(true)));
}

#[test]
fn the_engines_paragraph_reset_drops_extension_data() {
    // Why `format.resetToStyle` exists. If this starts failing, upstream keeps extension data
    // now: say so in the module docs.
    let mut s = doc("Text");
    LABEL.set(s.doc.para_mut(StoryRef::Body, &Path::top(0)).unwrap(), &"lost".to_string()).unwrap();
    s.run("para.reset", &json!({})).unwrap();
    assert!(!LABEL.is_set(s.doc.para(StoryRef::Body, &Path::top(0)).unwrap()));
}

#[test]
fn toggle_capitals_flips_the_selection_and_keeps_runs() {
    let mut s = doc("Rain falls straße.");
    assert!(matches!(s.run("format.toggleCapitals", &json!({})), Err(CmdError::Disabled(_))));
    select(&mut s, 0, 0, 2);
    s.run("format.bold", &json!({"value": true})).unwrap();
    select(&mut s, 0, 0, 4);
    s.run("format.toggleCapitals", &json!({})).unwrap();
    assert_eq!(body(&s)[0], "RAIN falls straße.");
    assert_eq!((char_at(&s, 0, 1).bold, char_at(&s, 0, 2).bold), (Some(true), None), "runs untouched");
    s.run("format.toggleCapitals", &json!({})).unwrap();
    assert_eq!(body(&s)[0], "rain falls straße.");
    select(&mut s, 0, 11, 18);
    s.run("format.toggleCapitals", &json!({})).unwrap();
    assert_eq!(body(&s)[0], "rain falls STRASSE.", "a letter whose capital is longer");
    s.run("edit.undo", &json!({})).unwrap();
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(body(&s)[0], "RAIN falls straße.");
}

// Editing.

#[test]
fn new_page_splits_the_paragraph_and_toggles_at_its_start() {
    let mut s = doc("Rain falls.");
    s.sel = Selection::caret(Pos::body(0, 5));
    s.run("edit.newPage", &json!({})).unwrap();
    assert_eq!(body(&s), ["Rain ", "falls."]);
    let breaks = |s: &Session| {
        s.doc.para_paths(StoryRef::Body).iter().map(|p| s.doc.para(StoryRef::Body, p).and_then(|x| x.props.page_break_before)).collect::<Vec<_>>()
    };
    assert_eq!(breaks(&s), [None, Some(true)]);
    assert_eq!(s.sel, Selection::caret(Pos::body(1, 0)));
    s.run("edit.newPage", &json!({})).unwrap();
    assert_eq!(breaks(&s), [None, None], "at the start of a page it is removed");
    s.run("edit.newPage", &json!({})).unwrap();
    assert_eq!((body(&s).len(), breaks(&s)), (2, vec![None, Some(true)]), "and set again without a split");
    for _ in 0..3 {
        s.run("edit.undo", &json!({})).unwrap();
    }
    assert_eq!(body(&s), ["Rain falls."]);
    // Over a selection, the selection goes first.
    select(&mut s, 0, 4, 5);
    s.run("edit.newPage", &json!({})).unwrap();
    assert_eq!(body(&s), ["Rain", "falls."]);
}

#[test]
fn delete_paragraph_removes_the_caret_paragraph() {
    let mut s = doc("One\nTwo\nThree");
    s.sel = Selection::caret(Pos::body(1, 2));
    s.run("edit.deleteParagraph", &json!({})).unwrap();
    assert_eq!(body(&s), ["One", "Three"]);
    assert_eq!(s.sel, Selection::caret(Pos::body(1, 0)));
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(body(&s), ["One", "Two", "Three"]);
    // The last paragraph, then the only one: a story keeps one empty paragraph.
    let mut one = doc("Only");
    one.run("edit.deleteParagraph", &json!({})).unwrap();
    assert_eq!(body(&one), [""]);
    // Tracked: the paragraph stays as a deletion to accept or reject.
    s.doc.settings.track_changes = true;
    s.sel = Selection::caret(Pos::body(2, 0));
    s.run("edit.deleteParagraph", &json!({})).unwrap();
    assert!(body(&s).concat().contains("Three"), "{:?}", body(&s));
    s.run("review.acceptAll", &json!({})).unwrap();
    assert_eq!(body(&s), ["One", "Two"]);
}

#[test]
fn select_next_and_previous_paragraph() {
    let mut s = doc("One\nTwo\nThree");
    let depth = s.undo_depth();
    s.run("select.nextParagraph", &json!({})).unwrap();
    assert_eq!(s.selected_text(), "Two");
    s.run("select.nextParagraph", &json!({})).unwrap();
    s.run("select.nextParagraph", &json!({})).unwrap();
    assert_eq!(s.selected_text(), "Three", "stays on the last");
    s.run("select.previousParagraph", &json!({})).unwrap();
    assert_eq!(s.selected_text(), "Two");
    assert_eq!(s.undo_depth(), depth);
}

// Spelling.

#[test]
fn check_ignore_and_replace() {
    let mut s = doc("A glorp and a dragn.\nAll fine here.");
    let words = |s: &mut Session| -> Vec<String> {
        let r = s.run("spelling.check", &json!({})).unwrap();
        r["misspellings"].as_array().unwrap().iter().filter_map(|m| m["word"].as_str().map(str::to_string)).collect()
    };
    assert_eq!(words(&mut s), ["glorp", "dragn"]);
    assert_eq!(s.run("spelling.ignore", &json!({"word": "Glorp"})).unwrap()["added"], true);
    assert_eq!(s.run("spelling.ignore", &json!({"word": "glorp"})).unwrap()["added"], false, "once, whatever the case");
    assert_eq!(words(&mut s), ["dragn"]);
    assert_eq!(s.run("spelling.words", &json!({})).unwrap()["words"], json!(["Glorp"]));
    // Kept with the document, and undoable.
    let back = native::open(&native::save(&s.doc).unwrap()).unwrap();
    assert_eq!(DocumentWords::of(&back).words, ["Glorp"]);
    s.run("edit.undo", &json!({})).unwrap();
    assert!(!DOCUMENT_WORDS.is_set(&s.doc));
    s.run("spelling.ignore", &json!({"word": "glorp"})).unwrap();
    assert_eq!(s.run("spelling.unignore", &json!({"word": "GLORP"})).unwrap()["removed"], true);
    assert_eq!(words(&mut s), ["glorp", "dragn"]);
    assert!(matches!(s.run("spelling.ignore", &json!({"word": " "})), Err(CmdError::Params(_))));

    let sug = s.run("spelling.suggest", &json!({"word": "hosue"})).unwrap();
    assert!(sug["suggestions"].as_array().unwrap().iter().any(|x| x == "house"), "{sug}");
    let sug = s.run("spelling.suggest", &json!({"word": "dragn", "max": 2})).unwrap();
    assert_eq!(sug["suggestions"].as_array().map(Vec::len), Some(2));

    let found = s.run("spelling.check", &json!({})).unwrap();
    let mut params = found["misspellings"][1].clone();
    params["with"] = json!("dragon");
    s.run("spelling.replace", &params).unwrap();
    assert_eq!(body(&s)[0], "A glorp and a dragon.");
    assert!(matches!(s.run("spelling.replace", &params), Err(CmdError::Failed(_))), "the word is no longer there");
    s.run("edit.undo", &json!({})).unwrap();
    assert_eq!(body(&s)[0], "A glorp and a dragn.");
}

#[test]
fn an_app_adds_its_own_spelling_rules() {
    let mut s = doc("Zorvex speaks.\nA plimbo\nA trunzel here.");
    // Text marked "do not check" is skipped.
    select(&mut s, 2, 2, 9);
    let (a, b) = s.sel.ordered();
    s.doc.format_range(&a, &b, &|c| c.no_proof = Some(true)).unwrap();
    let found: Vec<String> = misspellings(&s.doc).into_iter().map(|m| m.word).collect();
    assert_eq!(found, ["Zorvex", "plimbo"]);
    let opts = jcj_proof::SpellingOptions::default();
    let skip_short = |p: &jcj_doc::Paragraph| p.text.len() < 10;
    let known = |w: &str| w == "Zorvex";
    assert!(misspellings_with(&s.doc, &opts, &skip_short, &known).is_empty());
    assert_eq!(misspellings_with(&s.doc, &opts, &|_| false, &known).len(), 1);
}
