// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bookmarks and labels: named places in a document to go to. A label is a bookmark that is also
//! offered by name in a Go To list; both are kept the same way.
//!
//! A bookmark's name lives in the document under [`BOOKMARKS`] (`core.bookmarks`); its place is
//! the paragraph carrying its id ([`BOOKMARK`], `core.bookmark`, paragraph extension data), so it
//! moves with the paragraph through every edit. A paragraph holds at most one. Splitting a
//! paragraph leaves the bookmark on the first part.
//!
//! Ported from the first foundation app's bookmarks, with the paragraph field moved into
//! extension data.

use std::collections::BTreeMap;

use jcj_doc::walk::body_paragraphs;
use jcj_doc::{Document, ExtKey, Path, StoryRef};
use jcj_engine::{CmdError, CmdResult, CommandSpec, Selection, Session, cmd::sel_result, params as p};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::data::{clean_line, next_id, pos_at, store};
use crate::pages::{page_of, planned_pages};

/// Paragraph data: the id of the bookmark on this paragraph.
pub const BOOKMARK: ExtKey<u32> = ExtKey::new("core.bookmark");
/// Document data: the bookmarks' names, by id.
pub const BOOKMARKS: ExtKey<BTreeMap<String, Value>> = ExtKey::new("core.bookmarks");
/// Most bookmarks a document keeps, and the longest name.
pub const MAX_BOOKMARKS: usize = 10_000;
pub const MAX_NAME_CHARS: usize = 100;

/// One bookmark.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Bookmark {
    pub name: String,
    /// Listed as a label too.
    pub label: bool,
}

/// A document's bookmarks, by id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bookmarks {
    pub items: BTreeMap<u32, Bookmark>,
}

impl Bookmarks {
    /// The bookmarks `doc` holds; bad entries are skipped and names cut to their limit.
    pub fn of_doc(doc: &Document) -> Self {
        let items = BOOKMARKS
            .get_or_none(doc)
            .unwrap_or_default()
            .into_iter()
            .take(MAX_BOOKMARKS)
            .filter_map(|(k, v)| {
                let mut b: Bookmark = serde_json::from_value(v).ok()?;
                b.name = clean_line(&b.name, MAX_NAME_CHARS);
                Some((k.parse().ok()?, b))
            })
            .collect();
        Bookmarks { items }
    }

    pub fn store(&self, doc: &mut Document) -> Result<(), CmdError> {
        let map: BTreeMap<String, Value> = self.items.iter().map(|(k, v)| (k.to_string(), serde_json::to_value(v).unwrap_or_default())).collect();
        store(doc, BOOKMARKS, &map)
    }
}

/// Every bookmark that is in the document, as (paragraph, id), in document order.
pub fn placed(doc: &Document) -> Vec<(Path, u32)> {
    let known = Bookmarks::of_doc(doc);
    body_paragraphs(doc)
        .into_iter()
        .filter_map(|(path, p)| BOOKMARK.get_or_none(p).filter(|id| known.items.contains_key(id)).map(|id| (path, id)))
        .collect()
}

/// Keep each bookmark on its first paragraph only (a split paragraph's second part, or a pasted
/// copy, loses it).
pub fn keep_unique(doc: &mut Document) {
    let mut seen = std::collections::HashSet::new();
    let dupes: Vec<Path> =
        body_paragraphs(doc).into_iter().filter(|(_, p)| BOOKMARK.get_or_none(*p).is_some_and(|id| !seen.insert(id))).map(|(path, _)| path).collect();
    for path in dupes {
        if let Ok(p) = doc.para_mut(StoryRef::Body, &path) {
            BOOKMARK.remove(p);
        }
    }
}

/// `bookmark.list`: the bookmarks with their names, places, pages and opening text.
fn list(s: &mut Session, _: &Value) -> CmdResult {
    let names = Bookmarks::of_doc(&s.doc);
    let pages = planned_pages(&s.doc);
    let labels = crate::locks::labels_or_numbers(&s.doc, &pages);
    let items: Vec<Value> = placed(&s.doc)
        .into_iter()
        .filter_map(|(path, id)| {
            let b = names.items.get(&id)?;
            let block = path.0.first().map_or(0, |b| *b as usize);
            let page = page_of(&pages, block, 0).and_then(|i| labels.get(i).cloned());
            let text = s.doc.para(StoryRef::Body, &path).map(|p| p.text.chars().take(80).collect::<String>()).unwrap_or_default();
            Some(json!({"id": id, "name": b.name, "label": b.label, "path": path.0, "page": page, "text": text}))
        })
        .collect();
    Ok(json!({"bookmarks": items}))
}

/// `bookmark.add {name?, label?}`: bookmark the caret's paragraph (renaming its bookmark if it
/// has one).
fn add(s: &mut Session, v: &Value) -> CmdResult {
    let at = s.sel.focus.clone();
    if at.story != StoryRef::Body {
        return Err(CmdError::Failed("bookmarks mark the document's main text".into()));
    }
    let para = s.doc.para(StoryRef::Body, &at.path).ok_or_else(|| CmdError::Failed("the caret is not in a paragraph".into()))?;
    let para_text = para.text.clone();
    let mut marks = Bookmarks::of_doc(&s.doc);
    let existing = BOOKMARK.get_or_none(para).filter(|id| marks.items.contains_key(id));
    let given = p::str(v, "name").map(|n| clean_line(n, MAX_NAME_CHARS)).filter(|n| !n.is_empty());
    let name = given.unwrap_or_else(|| {
        let t = clean_line(&para_text, 40);
        if t.is_empty() { format!("Bookmark {}", marks.items.len() + 1) } else { t }
    });
    let label = p::bool(v, "label").unwrap_or(false);
    let id = match existing {
        Some(id) => id,
        None => {
            if marks.items.len() >= MAX_BOOKMARKS {
                return Err(CmdError::Failed(format!("a document keeps at most {MAX_BOOKMARKS} bookmarks")));
            }
            next_id(marks.items.keys().copied())
        }
    };
    marks.items.insert(id, Bookmark { name: name.clone(), label });
    marks.store(&mut s.doc)?;
    let para = s.doc.para_mut(StoryRef::Body, &at.path)?;
    BOOKMARK.set(para, &id).map_err(|e| CmdError::Failed(e.to_string()))?;
    Ok(json!({"id": id, "name": name, "label": label}))
}

fn id_param(s: &Session, v: &Value) -> Result<u32, CmdError> {
    if let Some(id) = p::u64(v, "id") {
        return u32::try_from(id).map_err(|_| CmdError::Params("`id` is out of range".into()));
    }
    if let Some(name) = p::str(v, "name") {
        let marks = Bookmarks::of_doc(&s.doc);
        return marks
            .items
            .iter()
            .find(|(_, b)| b.name.eq_ignore_ascii_case(name.trim()))
            .map(|(k, _)| *k)
            .ok_or_else(|| CmdError::Failed(format!("no bookmark `{name}`")));
    }
    s.doc
        .para_at(&s.sel.focus)
        .and_then(|p| BOOKMARK.get_or_none(p))
        .ok_or_else(|| CmdError::Params("give `id` or `name`, or put the caret on a bookmarked paragraph".into()))
}

fn goto_path(s: &mut Session, path: &Path) -> CmdResult {
    s.sel = Selection::caret(pos_at(path, 0));
    s.clamp_selection();
    sel_result(s)
}

/// `bookmark.goto {id | name}`.
fn goto(s: &mut Session, v: &Value) -> CmdResult {
    let id = id_param(s, v)?;
    let path = placed(&s.doc)
        .into_iter()
        .find(|(_, b)| *b == id)
        .map(|(p, _)| p)
        .ok_or_else(|| CmdError::Failed("that bookmark is no longer in the document".into()))?;
    goto_path(s, &path)
}

/// `bookmark.delete {id | name}`, or the caret paragraph's bookmark.
fn delete(s: &mut Session, v: &Value) -> CmdResult {
    let id = id_param(s, v)?;
    let mut marks = Bookmarks::of_doc(&s.doc);
    marks.items.remove(&id);
    marks.store(&mut s.doc)?;
    let paths: Vec<Path> = body_paragraphs(&s.doc).into_iter().filter(|(_, p)| BOOKMARK.get_or_none(*p) == Some(id)).map(|(p, _)| p).collect();
    for path in paths {
        let para = s.doc.para_mut(StoryRef::Body, &path)?;
        BOOKMARK.remove(para);
    }
    Ok(Value::Null)
}

/// The next (or previous) bookmark from the caret, wrapping round.
fn step(s: &mut Session, forward: bool) -> CmdResult {
    let here = s.sel.focus.path.clone();
    let all = placed(&s.doc);
    let pick = if forward {
        all.iter().find(|(p, _)| *p > here).or_else(|| all.first())
    } else {
        all.iter().rev().find(|(p, _)| *p < here).or_else(|| all.last())
    };
    let Some((path, _)) = pick.cloned() else { return Err(CmdError::Failed("the document has no bookmarks".into())) };
    goto_path(s, &path)
}

fn has_bookmarks(s: &Session) -> Option<&'static str> {
    if placed(&s.doc).is_empty() { Some("the document has no bookmarks") } else { None }
}

/// The bookmark commands.
pub fn commands() -> Vec<CommandSpec> {
    const LOC: &str = "Go";
    vec![
        CommandSpec::new("bookmark.add", "Add Bookmark", LOC, add).params(r#"{"name"?: string, "label"?: bool}"#),
        CommandSpec::new("bookmark.list", "Bookmarks", LOC, list).pure(),
        CommandSpec::new("bookmark.goto", "Go to Bookmark", LOC, goto).params(r#"{"id"?: number, "name"?: string}"#).pure(),
        CommandSpec::new("bookmark.delete", "Delete Bookmark", LOC, delete).params(r#"{"id"?: number, "name"?: string}"#),
        CommandSpec::new("bookmark.next", "Next Bookmark", LOC, |s, _| step(s, true)).pure().when(has_bookmarks),
        CommandSpec::new("bookmark.previous", "Previous Bookmark", LOC, |s, _| step(s, false)).pure().when(has_bookmarks),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::session_with;

    #[test]
    fn bookmarks_follow_their_paragraph_and_wrap() {
        let mut s = session_with(&["A heading", "Rain.", "Sun."]);
        s.sel = Selection::caret(pos_at(&Path::top(1), 0));
        let b = s.run("bookmark.add", &json!({"name": "rain"})).unwrap();
        assert_eq!(b["name"], "rain");
        // A paragraph inserted before it moves it down.
        s.sel = Selection::caret(pos_at(&Path::top(0), 0));
        s.run("text.insert", &json!({"text": "First.\r"})).unwrap();
        let listed = s.run("bookmark.list", &json!({})).unwrap();
        assert_eq!(listed["bookmarks"][0]["path"], json!([2]));
        assert_eq!(listed["bookmarks"][0]["page"], "1");
        s.sel = Selection::caret(pos_at(&Path::top(3), 0));
        s.run("bookmark.next", &json!({})).unwrap();
        assert_eq!(s.sel.focus.path, Path::top(2), "wraps round");
        s.run("bookmark.delete", &json!({"name": "RAIN"})).unwrap();
        assert!(placed(&s.doc).is_empty());
        assert!(s.run("bookmark.next", &json!({})).is_err());
    }

    #[test]
    fn splitting_keeps_the_bookmark_on_the_first_part() {
        let mut s = session_with(&["A heading", "Rain falls."]);
        s.sel = Selection::caret(pos_at(&Path::top(1), 0));
        s.run("bookmark.add", &json!({"label": true})).unwrap();
        s.sel = Selection::caret(pos_at(&Path::top(1), 4));
        s.run("text.newParagraph", &json!({})).unwrap();
        assert_eq!(placed(&s.doc), vec![(Path::top(1), 1)]);
        let listed = s.run("bookmark.list", &json!({})).unwrap();
        assert_eq!(listed["bookmarks"][0]["label"], true);
        assert_eq!(listed["bookmarks"][0]["name"], "Rain falls.");
    }

    #[test]
    fn hostile_names_are_bounded() {
        let mut doc = Document::new();
        doc.ext.set("core.bookmarks", json!({"1": {"name": "x".repeat(10_000)}, "nope": {}, "2": 7})).unwrap();
        let b = Bookmarks::of_doc(&doc);
        assert_eq!(b.items.len(), 1);
        assert!(b.items.values().all(|m| m.name.chars().count() <= MAX_NAME_CHARS));
    }
}
