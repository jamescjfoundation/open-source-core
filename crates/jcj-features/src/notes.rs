// SPDX-License-Identifier: MIT OR Apache-2.0
//! Notes attached to text.
//!
//! A note's text, author, title, type, color and dates live in the document under [`NOTES`]
//! (`core.notes`, saved and undone with it). Its anchor is a run mark on the characters it is
//! attached to ([`NOTE`], `core.note`: the note's id), so it moves with them through typing,
//! deleting, splitting and joining, and the document's text itself is never changed. Typing at
//! the edge of a noted range does not extend it (the editing commands this crate puts in place of
//! the engine's see to that). A note whose text was all deleted is still listed, as detached,
//! until it is deleted.
//!
//! Note types are the document's own list ([`NOTE_TYPES`], `core.note_types`); a document
//! without one has [`DEFAULT_NOTE_TYPES`].
//!
//! Ported from the first foundation app's notes (its decision 0023), with the marks moved into
//! extension data.

use std::collections::BTreeMap;

use jcj_doc::para::OBJ;
use jcj_doc::walk::body_paragraphs;
use jcj_doc::{Document, ExtKey, Path, Pos, StoryRef};
use jcj_engine::{CmdError, CmdResult, CommandSpec, Selection, Session, cmd::sel_result, params as p};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::data::{clean_line, next_id, parse_color, pos_at, store};
use crate::marks::{mark_range, unmark_range};

/// Run mark: the id of the note the characters are attached to.
pub const NOTE: ExtKey<u32> = ExtKey::new("core.note");
/// Document data: the notes, by id.
pub const NOTES: ExtKey<BTreeMap<String, Value>> = ExtKey::new("core.notes");
/// Document data: the note types.
pub const NOTE_TYPES: ExtKey<Vec<String>> = ExtKey::new("core.note_types");
/// Most notes a document keeps, and the longest note.
pub const MAX_NOTES: usize = 10_000;
pub const MAX_NOTE_CHARS: usize = 10_000;
/// Characters of anchored text a listing quotes.
pub const MAX_QUOTE_CHARS: usize = 120;
/// Most note types, and the longest.
pub const MAX_TYPES: usize = 100;
pub const MAX_TYPE_CHARS: usize = 40;
/// The note types a document starts with.
pub const DEFAULT_NOTE_TYPES: [&str; 5] = ["General", "Idea", "Question", "Research", "To Do"];

/// One note.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Note {
    pub text: String,
    pub author: String,
    pub title: String,
    /// One of the document's note types.
    pub kind: String,
    /// `#RRGGBB`, or empty.
    pub color: String,
    /// Unix seconds, given by the caller; 0 when unknown.
    pub created: u64,
    pub modified: u64,
}

fn clean(text: &str) -> String {
    text.replace("\r\n", "\n").chars().filter(|c| *c == '\n' || !c.is_control()).take(MAX_NOTE_CHARS).collect::<String>().trim().to_string()
}

impl Note {
    /// The note brought within limits.
    fn bounded(mut self) -> Self {
        self.text = clean(&self.text);
        self.author = clean_line(&self.author, 100);
        self.title = clean_line(&self.title, 200);
        self.kind = clean_line(&self.kind, MAX_TYPE_CHARS);
        if parse_color(&self.color).is_none() {
            self.color.clear();
        }
        self
    }

    /// Title, type, color and time from a command's parameters.
    fn apply_fields(&mut self, v: &Value) {
        if let Some(t) = p::str(v, "title") {
            self.title = clean_line(t, 200);
        }
        if let Some(k) = p::str(v, "type") {
            self.kind = clean_line(k, MAX_TYPE_CHARS);
        }
        if let Some(c) = p::str(v, "color") {
            self.color = if parse_color(c).is_some() { c.trim().to_uppercase() } else { String::new() };
        }
        if let Some(now) = p::u64(v, "now") {
            if self.created == 0 {
                self.created = now;
            }
            self.modified = now;
        }
    }
}

/// The document's note types.
pub fn note_types(doc: &Document) -> Vec<String> {
    match NOTE_TYPES.get_or_none(doc) {
        Some(list) => list.iter().map(|t| clean_line(t, MAX_TYPE_CHARS)).filter(|t| !t.is_empty()).take(MAX_TYPES).collect(),
        None => DEFAULT_NOTE_TYPES.iter().map(|t| (*t).to_string()).collect(),
    }
}

/// The notes of a document, by id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Notes {
    pub notes: BTreeMap<u32, Note>,
}

impl Notes {
    /// The notes `doc` holds; bad entries are skipped and texts cut to their limits.
    pub fn of_doc(doc: &Document) -> Self {
        let notes = NOTES
            .get_or_none(doc)
            .unwrap_or_default()
            .into_iter()
            .take(MAX_NOTES)
            .filter_map(|(k, v)| Some((k.parse::<u32>().ok()?, serde_json::from_value::<Note>(v).ok()?.bounded())))
            .collect();
        Notes { notes }
    }

    pub fn store(&self, doc: &mut Document) -> Result<(), CmdError> {
        let map: BTreeMap<String, Value> = self.notes.iter().map(|(k, v)| (k.to_string(), serde_json::to_value(v).unwrap_or_default())).collect();
        store(doc, NOTES, &map)
    }

    /// The id a new note takes.
    pub fn next_id(&self) -> u32 {
        next_id(self.notes.keys().copied())
    }

    /// Add `note` with a new id (and keep it within the limits).
    pub fn add(&mut self, note: Note) -> Result<u32, CmdError> {
        if self.notes.len() >= MAX_NOTES {
            return Err(CmdError::Failed(format!("a document keeps at most {MAX_NOTES} notes")));
        }
        let id = self.next_id();
        self.notes.insert(id, note.bounded());
        Ok(id)
    }
}

/// Where a note is anchored: its first and last body positions and the text it covers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Anchor {
    pub start: Pos,
    pub end: Pos,
    pub quote: String,
}

/// Every note's anchor in the body (tables included).
pub fn anchors(doc: &Document) -> BTreeMap<u32, Anchor> {
    let mut out: BTreeMap<u32, Anchor> = BTreeMap::new();
    for (path, p) in body_paragraphs(doc) {
        for (range, props) in p.run_ranges() {
            let Some(id) = NOTE.get_or_none(props) else { continue };
            let piece = p.text.get(range.clone()).unwrap_or("");
            let a =
                out.entry(id).or_insert_with(|| Anchor { start: pos_at(&path, range.start), end: pos_at(&path, range.end), quote: String::new() });
            // A note running into the next paragraph quotes it after a space.
            let next_paragraph = a.end.path != path;
            a.end = pos_at(&path, range.end);
            if a.quote.chars().count() < MAX_QUOTE_CHARS {
                if next_paragraph && !a.quote.is_empty() {
                    a.quote.push(' ');
                }
                a.quote.extend(piece.chars().filter(|c| *c != OBJ).take(MAX_QUOTE_CHARS));
            }
        }
    }
    out
}

/// The note at a position: on the character after it, else on the one before.
pub fn note_at(doc: &Document, pos: &Pos) -> Option<u32> {
    let p = doc.para_at(pos)?;
    let here = (pos.off < p.len()).then(|| NOTE.get_or_none(p.props_of_char(pos.off))).flatten();
    here.or_else(|| (pos.off > 0).then(|| NOTE.get_or_none(p.props_at(pos.off))).flatten())
}

/// `note.add {text, title?, type?, color?, now?}`: attach a note to the selection (or, with a
/// caret, to its paragraph).
fn add(s: &mut Session, v: &Value) -> CmdResult {
    let text = clean(p::req_str(v, "text")?);
    let mut notes = Notes::of_doc(&s.doc);
    let (mut a, mut b) = s.sel.ordered();
    if a == b {
        let len = s.doc.para_at(&a).map_or(0, |p| p.len());
        if len == 0 {
            return Err(CmdError::Failed("there is no text here to attach a note to".into()));
        }
        a.off = 0;
        b.off = len;
    }
    if a.story != StoryRef::Body {
        return Err(CmdError::Failed("notes attach to the document's main text".into()));
    }
    let mut note = Note { text, author: s.author.clone(), ..Note::default() };
    note.apply_fields(v);
    if note.kind.is_empty() {
        note.kind = note_types(&s.doc).into_iter().next().unwrap_or_default();
    }
    let id = notes.add(note)?;
    mark_range(&mut s.doc, &a, &b, NOTE, &id)?;
    notes.store(&mut s.doc)?;
    Ok(json!({"id": id}))
}

fn note_param(s: &Session, v: &Value) -> Result<(u32, Notes), CmdError> {
    let id = p::u64(v, "id").and_then(|i| u32::try_from(i).ok()).ok_or_else(|| CmdError::Params("`id` must be a note id".into()))?;
    let notes = Notes::of_doc(&s.doc);
    if !notes.notes.contains_key(&id) {
        return Err(CmdError::Params(format!("there is no note {id}")));
    }
    Ok((id, notes))
}

/// `note.edit {id, text?, title?, type?, color?, now?}`: change a note.
fn edit(s: &mut Session, v: &Value) -> CmdResult {
    let (id, mut notes) = note_param(s, v)?;
    if let Some(n) = notes.notes.get_mut(&id) {
        if let Some(t) = p::str(v, "text") {
            n.text = clean(t);
        }
        n.apply_fields(v);
    }
    notes.store(&mut s.doc)?;
    Ok(json!({"id": id}))
}

/// `note.delete {id}`: delete a note and its anchor.
fn delete(s: &mut Session, v: &Value) -> CmdResult {
    let (id, mut notes) = note_param(s, v)?;
    if let Some(a) = anchors(&s.doc).get(&id) {
        unmark_range(&mut s.doc, &a.start, &a.end, NOTE, &|n| *n == id)?;
    }
    notes.notes.remove(&id);
    notes.store(&mut s.doc)?;
    sel_result(s)
}

fn path_json(p: &Path) -> Value {
    json!(p.0)
}

/// The notes as a listing, in document order (detached notes last): id, text, author, title,
/// type, color, dates, the text they are attached to, where, and whether they still are.
pub fn list_json(doc: &Document) -> Value {
    let at = anchors(doc);
    let mut rows: Vec<(Option<Pos>, Value)> = Notes::of_doc(doc)
        .notes
        .into_iter()
        .map(|(id, n)| {
            let a = at.get(&id);
            (
                a.map(|a| a.start.clone()),
                json!({
                    "id": id,
                    "text": n.text,
                    "author": n.author,
                    "title": n.title,
                    "type": n.kind,
                    "color": n.color,
                    "created": n.created,
                    "modified": n.modified,
                    "quote": a.map(|a| a.quote.clone()).unwrap_or_default(),
                    "path": a.map(|a| path_json(&a.start.path)),
                    "offset": a.map(|a| a.start.off),
                    "attached": a.is_some(),
                }),
            )
        })
        .collect();
    rows.sort_by(|x, y| match (&x.0, &y.0) {
        (Some(a), Some(b)) => a.cmp(b),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    json!(rows.into_iter().map(|(_, v)| v).collect::<Vec<_>>())
}

/// `note.at`: the note at the caret, if any.
fn at(s: &mut Session, _: &Value) -> CmdResult {
    let Some(id) = note_at(&s.doc, &s.sel.focus) else { return Ok(Value::Null) };
    let n = Notes::of_doc(&s.doc).notes.get(&id).cloned().unwrap_or_default();
    Ok(json!({"id": id, "text": n.text, "author": n.author, "title": n.title, "type": n.kind, "color": n.color}))
}

/// `note.goto {id}`: select the text a note is attached to.
fn goto(s: &mut Session, v: &Value) -> CmdResult {
    let (id, _) = note_param(s, v)?;
    let a = anchors(&s.doc).get(&id).cloned().ok_or_else(|| CmdError::Failed(format!("note {id} is no longer attached to any text")))?;
    s.sel = Selection { anchor: a.start, focus: a.end };
    sel_result(s)
}

/// The next (or previous) note from the caret, wrapping round.
fn step(s: &mut Session, forward: bool) -> CmdResult {
    let here = s.sel.focus.clone();
    let mut all: Vec<(u32, Pos)> = anchors(&s.doc).into_iter().map(|(id, a)| (id, a.start)).collect();
    all.sort_by(|x, y| x.1.cmp(&y.1));
    let pick = if forward {
        all.iter().find(|(_, at)| *at > here).or_else(|| all.first())
    } else {
        all.iter().rev().find(|(_, at)| *at < here).or_else(|| all.last())
    };
    let Some((id, _)) = pick.cloned() else { return Err(CmdError::Failed("the document has no notes".into())) };
    goto(s, &json!({"id": id}))
}

/// `note.setTypes {types}`.
fn set_types(s: &mut Session, v: &Value) -> CmdResult {
    let list = v.get("types").and_then(Value::as_array).ok_or_else(|| CmdError::Params("`types` is a list".into()))?;
    let types: Vec<String> =
        list.iter().filter_map(Value::as_str).map(|t| clean_line(t, MAX_TYPE_CHARS)).filter(|t| !t.is_empty()).take(MAX_TYPES).collect();
    store(&mut s.doc, NOTE_TYPES, &types)?;
    Ok(json!(types))
}

fn has_notes(s: &Session) -> Option<&'static str> {
    if anchors(&s.doc).is_empty() { Some("the document has no notes") } else { None }
}

/// The note commands.
pub fn commands() -> Vec<CommandSpec> {
    const LOC: &str = "Review › Notes";
    vec![
        CommandSpec::new("note.add", "Add Note", LOC, add)
            .params(r##"{"text": string, "title"?, "type"?, "color"? "#RRGGBB", "now"? unix seconds}"##),
        CommandSpec::new("note.edit", "Edit Note", LOC, edit).params(r##"{"id": number, "text"?, "title"?, "type"?, "color"?, "now"?}"##),
        CommandSpec::new("note.next", "Next Note", LOC, |s, _| step(s, true)).key("Mod+Alt+Down").pure().when(has_notes),
        CommandSpec::new("note.previous", "Previous Note", LOC, |s, _| step(s, false)).key("Mod+Alt+Up").pure().when(has_notes),
        CommandSpec::new("note.types", "Note Types", LOC, |s, _| Ok(json!(note_types(&s.doc)))).pure(),
        CommandSpec::new("note.setTypes", "Set Note Types", LOC, set_types).params(r#"{"types": [string]}"#),
        CommandSpec::new("note.delete", "Delete Note", LOC, delete).params(r#"{"id": number}"#),
        CommandSpec::new("note.list", "List Notes", LOC, |s, _| Ok(list_json(&s.doc))).pure(),
        CommandSpec::new("note.at", "Note at Caret", LOC, at).pure(),
        CommandSpec::new("note.goto", "Go to Note", LOC, goto).params(r#"{"id": number}"#).pure(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{lines, select, session_with};

    fn session() -> Session {
        session_with(&["A heading", "Rain hits the window.", "A third line.", "Hello there."])
    }

    #[test]
    fn notes_attach_list_edit_and_delete() {
        let mut s = session();
        let before = lines(&s);
        select(&mut s, 1, 0, 4);
        let id = s.run("note.add", &json!({"text": "Make it snow?"})).unwrap()["id"].as_u64().unwrap();
        assert_eq!(lines(&s), before, "the text is not changed");
        let list = s.run("note.list", &json!({})).unwrap();
        assert_eq!(list[0]["quote"], "Rain");
        assert_eq!(list[0]["path"], json!([1]));
        assert_eq!(list[0]["attached"], true);
        s.sel = Selection::caret(pos_at(&Path::top(1), 2));
        assert_eq!(s.run("note.at", &json!({})).unwrap()["text"], "Make it snow?");
        s.run("note.edit", &json!({"id": id, "text": "Snow."})).unwrap();
        // A caret note covers its paragraph.
        s.sel = Selection::caret(pos_at(&Path::top(3), 0));
        s.run("note.add", &json!({"text": "Too polite."})).unwrap();
        let list = s.run("note.list", &json!({})).unwrap();
        assert_eq!((list[0]["text"].as_str(), list[1]["quote"].as_str()), (Some("Snow."), Some("Hello there.")));
        assert!(!NOTE.is_set(&s.doc.body[3].as_para().unwrap().mark), "the paragraph mark is not noted");
        s.run("note.delete", &json!({"id": id})).unwrap();
        assert_eq!(s.run("note.list", &json!({})).unwrap().as_array().map(Vec::len), Some(1));
        assert!(s.doc.body[1].as_para().unwrap().run_ranges().all(|(_, c)| !NOTE.is_set(c)), "its anchor is gone");
        assert!(s.run("note.edit", &json!({"id": 99, "text": "x"})).is_err());
        s.run("edit.undo", &json!({})).unwrap();
        assert_eq!(Notes::of_doc(&s.doc).notes.len(), 2, "undo brings the note back");
    }

    #[test]
    fn anchors_move_with_their_text() {
        let mut s = session();
        select(&mut s, 1, 5, 9);
        let id = s.run("note.add", &json!({"text": "hits?"})).unwrap()["id"].as_u64().unwrap() as u32;
        // Typing before the noted word moves the anchor; typing right after it does not extend it.
        s.sel = Selection::caret(pos_at(&Path::top(1), 0));
        s.run("text.insert", &json!({"text": "Cold "})).unwrap();
        assert_eq!(anchors(&s.doc)[&id].quote, "hits");
        s.sel = Selection::caret(pos_at(&Path::top(1), 14));
        s.run("text.insert", &json!({"text": "!"})).unwrap();
        assert_eq!(anchors(&s.doc)[&id].quote, "hits", "typing at the end of a note does not extend it");
        // Typing inside it does.
        s.sel = Selection::caret(pos_at(&Path::top(1), 12));
        s.run("text.insert", &json!({"text": "-"})).unwrap();
        assert_eq!(anchors(&s.doc)[&id].quote, "hi-ts");
        // A split keeps it on both halves; deleting all its text detaches it.
        s.sel = Selection::caret(pos_at(&Path::top(1), 12));
        s.run("text.newParagraph", &json!({})).unwrap();
        assert_eq!(anchors(&s.doc)[&id].quote, "hi -ts");
        s.run("edit.undo", &json!({})).unwrap();
        select(&mut s, 1, 10, 15);
        s.run("text.backspace", &json!({})).unwrap();
        let list = s.run("note.list", &json!({})).unwrap();
        assert_eq!(list[0]["attached"], false);
        assert!(s.run("note.goto", &json!({"id": id})).is_err());
    }

    #[test]
    fn note_fields_types_and_stepping() {
        let mut s = session_with(&["One.", "Two.", "Three."]);
        s.sel = Selection::caret(pos_at(&Path::top(1), 0));
        let a = s.run("note.add", &json!({"text": "Why?", "title": "Question", "type": "Idea", "color": "#ff0000", "now": 86_400})).unwrap()["id"]
            .clone();
        s.sel = Selection::caret(pos_at(&Path::top(2), 0));
        s.run("note.add", &json!({"text": "How?"})).unwrap();
        s.run("note.edit", &json!({"id": a, "now": 172_800})).unwrap();
        let list = s.run("note.list", &json!({})).unwrap();
        assert_eq!(
            (list[0]["title"].as_str(), list[0]["type"].as_str(), list[0]["color"].as_str()),
            (Some("Question"), Some("Idea"), Some("#FF0000"))
        );
        assert_eq!((list[0]["created"].as_u64(), list[0]["modified"].as_u64()), (Some(86_400), Some(172_800)));
        assert_eq!(list[1]["type"], "General", "the first type by default");
        s.sel = Selection::caret(pos_at(&Path::top(0), 0));
        s.run("note.next", &json!({})).unwrap();
        assert_eq!(s.sel.focus.path, Path::top(1));
        s.run("note.next", &json!({})).unwrap();
        assert_eq!(s.sel.focus.path, Path::top(2));
        s.run("note.next", &json!({})).unwrap();
        assert_eq!(s.sel.focus.path, Path::top(1), "wraps");
        s.run("note.setTypes", &json!({"types": ["Fix", " ", "Idea"]})).unwrap();
        assert_eq!(s.run("note.types", &json!({})).unwrap(), json!(["Fix", "Idea"]));
    }

    #[test]
    fn hostile_note_data_reads_safely() {
        let mut doc = Document::new();
        doc.ext.set("core.notes", json!({"x": {"text": "bad id"}, "3": {"text": 5}, "4": {"text": "ok\u{0}"}})).unwrap();
        let n = Notes::of_doc(&doc);
        assert_eq!(n.notes.len(), 1);
        assert_eq!(n.notes[&4].text, "ok");
        assert_eq!(n.next_id(), 5);
        doc.ext.set("core.notes", json!("nonsense")).unwrap();
        assert!(Notes::of_doc(&doc).notes.is_empty());
    }
}
