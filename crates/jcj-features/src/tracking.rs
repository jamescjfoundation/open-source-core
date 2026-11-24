// SPDX-License-Identifier: MIT OR Apache-2.0
//! Change tracking over the model's own tracked changes.
//!
//! While tracking is on (the document's `settings.track_changes`, the switch the engine's typing
//! and deleting already follow), typed text is marked inserted and deleted text is kept, marked
//! deleted, each with who made the change and when (the document's tracked-change records,
//! `Document::revisions`). This module lists those changes, steps through them, and accepts or
//! rejects them one at a time or all at once, and gives each writer a color.
//!
//! **Scope.** Changes to text runs are listed and settled here. Tracked paragraph marks, moves
//! and formatting changes are left to the engine's own `review.*` commands, which handle every
//! kind of tracked change the way a word processor does.
//!
//! **Writers' colors** come from the order writers first made a change, taken round a palette:
//! [`DEFAULT_WRITER_COLORS`] unless the app gives its own ([`writer_colors_from`]).
//!
//! Ported from the first foundation app's change tracking.

use jcj_doc::props::Rgb;
use jcj_doc::walk::body_paragraphs;
use jcj_doc::{Document, Path, StoryRef};
use jcj_engine::{CmdError, CmdResult, CommandSpec, Selection, Session, cmd::sel_result, params as p};
use serde_json::{Value, json};

use crate::data::{color_hex, pos_at};

/// The default writer palette: eight colors that read apart on a white page.
pub const DEFAULT_WRITER_COLORS: [Rgb; 8] = [
    Rgb(0x1F, 0x6F, 0xC5),
    Rgb(0xC0, 0x39, 0x2B),
    Rgb(0x2E, 0x8B, 0x57),
    Rgb(0x8E, 0x44, 0xAD),
    Rgb(0xD3, 0x6B, 0x00),
    Rgb(0x00, 0x83, 0x8F),
    Rgb(0xB0, 0x3A, 0x7A),
    Rgb(0x6D, 0x6D, 0x00),
];

/// Whether change tracking is on.
pub fn is_on(doc: &Document) -> bool {
    doc.settings.track_changes
}

/// Turn change tracking on or off.
pub fn set_on(doc: &mut Document, on: bool) {
    doc.settings.track_changes = on;
}

/// One tracked change in the body: where, what kind, and its record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub path: Path,
    pub start: usize,
    pub end: usize,
    /// Inserted (else deleted).
    pub inserted: bool,
    /// Index into `Document::revisions`.
    pub record: u32,
}

/// Every tracked change to text in the body (tables included), in order; runs next to each other
/// with the same record are one change.
pub fn changes(doc: &Document) -> Vec<Change> {
    let mut out: Vec<Change> = Vec::new();
    for (path, p) in body_paragraphs(doc) {
        for (r, c) in p.run_ranges() {
            let (inserted, record) = match (c.ins, c.del) {
                (_, Some(d)) => (false, d),
                (Some(i), None) => (true, i),
                _ => continue,
            };
            if let Some(last) = out.last_mut()
                && last.path == path
                && last.end == r.start
                && last.inserted == inserted
                && last.record == record
            {
                last.end = r.end;
                continue;
            }
            out.push(Change { path: path.clone(), start: r.start, end: r.end, inserted, record });
        }
    }
    out
}

/// The writers who made tracked changes, in the order of their first record, each with a color
/// from `palette` (taken round when there are more writers than colors).
pub fn writer_colors_from(doc: &Document, palette: &[Rgb]) -> Vec<(String, Rgb)> {
    let mut out: Vec<(String, Rgb)> = Vec::new();
    for r in &doc.revisions {
        if out.iter().any(|(a, _)| *a == r.author) {
            continue;
        }
        let color = palette.get(out.len() % palette.len().max(1)).copied().unwrap_or(Rgb(0, 0, 0));
        out.push((r.author.clone(), color));
    }
    out
}

/// [`writer_colors_from`] with [`DEFAULT_WRITER_COLORS`].
pub fn writer_colors(doc: &Document) -> Vec<(String, Rgb)> {
    writer_colors_from(doc, &DEFAULT_WRITER_COLORS)
}

/// Accept or reject one change.
pub fn settle(doc: &mut Document, c: &Change, accept: bool) -> Result<(), CmdError> {
    let remove = c.inserted != accept;
    if remove {
        let para = doc.para_mut(StoryRef::Body, &c.path)?;
        para.delete(c.start, c.end)?;
        para.touch();
    } else {
        doc.format_range(&pos_at(&c.path, c.start), &pos_at(&c.path, c.end), &|p| {
            p.ins = None;
            p.del = None;
        })?;
    }
    Ok(())
}

/// Accept or reject `list` (in any order).
pub fn settle_all(doc: &mut Document, mut list: Vec<Change>, accept: bool) -> Result<usize, CmdError> {
    // Last first, so earlier offsets stay right.
    list.sort_by(|a, b| (&b.path, b.start).cmp(&(&a.path, a.start)));
    for c in &list {
        settle(doc, c, accept)?;
    }
    Ok(list.len())
}

fn toggle(s: &mut Session, v: &Value) -> CmdResult {
    let on = p::bool(v, "on").unwrap_or(!is_on(&s.doc));
    set_on(&mut s.doc, on);
    Ok(json!({"on": on}))
}

fn list(s: &mut Session, _: &Value) -> CmdResult {
    let colors = writer_colors(&s.doc);
    let items: Vec<Value> = changes(&s.doc)
        .iter()
        .map(|c| {
            let r = s.doc.revisions.get(c.record as usize);
            let text = s.doc.para(StoryRef::Body, &c.path).and_then(|p| p.text.get(c.start..c.end)).unwrap_or("").to_string();
            let color = r.and_then(|r| colors.iter().find(|(a, _)| *a == r.author)).map(|(_, c)| color_hex(*c));
            json!({
                "path": c.path.0, "start": c.start, "end": c.end,
                "kind": if c.inserted { "inserted" } else { "deleted" },
                "author": r.map(|r| r.author.clone()), "date": r.map(|r| r.date.clone()), "color": color, "text": text,
            })
        })
        .collect();
    Ok(json!({"on": is_on(&s.doc), "changes": items}))
}

/// The changes the selection touches, or the one at the caret.
fn picked(s: &Session) -> Vec<Change> {
    let (a, b) = s.sel.ordered();
    if a.story != StoryRef::Body {
        return Vec::new();
    }
    let touches = |c: &Change| (&c.path, c.start) <= (&b.path, b.off) && (&c.path, c.end) >= (&a.path, a.off);
    changes(&s.doc).into_iter().filter(touches).collect()
}

fn settle_cmd(s: &mut Session, v: &Value, accept: bool) -> CmdResult {
    let all = p::bool(v, "all") == Some(true);
    let list = if all { changes(&s.doc) } else { picked(s) };
    if list.is_empty() {
        return Err(CmdError::Failed("there is no tracked change here".into()));
    }
    let n = settle_all(&mut s.doc, list, accept)?;
    s.clamp_selection();
    Ok(json!({"settled": n}))
}

fn step(s: &mut Session, forward: bool) -> CmdResult {
    let here = (s.sel.focus.path.clone(), s.sel.focus.off);
    let all = changes(&s.doc);
    let pick = if forward {
        all.iter().find(|c| (c.path.clone(), c.start) > here).or_else(|| all.first())
    } else {
        all.iter().rev().find(|c| (c.path.clone(), c.start) < here).or_else(|| all.last())
    };
    let Some(c) = pick.cloned() else { return Err(CmdError::Failed("there are no tracked changes".into())) };
    s.sel = Selection { anchor: pos_at(&c.path, c.start), focus: pos_at(&c.path, c.end) };
    sel_result(s)
}

fn has_changes(s: &Session) -> Option<&'static str> {
    if changes(&s.doc).is_empty() { Some("there are no tracked changes") } else { None }
}

/// The change-tracking commands.
pub fn commands() -> Vec<CommandSpec> {
    const LOC: &str = "Review › Changes";
    vec![
        CommandSpec::new("track.toggle", "Track Changes", LOC, toggle).params(r#"{"on"?: bool}"#),
        CommandSpec::new("track.list", "Tracked Changes", LOC, list).pure(),
        CommandSpec::new("track.accept", "Accept Change", LOC, |s, v| settle_cmd(s, v, true)).params(r#"{"all"?: bool}"#),
        CommandSpec::new("track.reject", "Reject Change", LOC, |s, v| settle_cmd(s, v, false)).params(r#"{"all"?: bool}"#),
        CommandSpec::new("track.acceptAll", "Accept All Changes", LOC, |s, _| settle_cmd(s, &json!({"all": true}), true)).when(has_changes),
        CommandSpec::new("track.rejectAll", "Reject All Changes", LOC, |s, _| settle_cmd(s, &json!({"all": true}), false)).when(has_changes),
        CommandSpec::new("track.next", "Next Change", LOC, |s, _| step(s, true)).pure().when(has_changes),
        CommandSpec::new("track.previous", "Previous Change", LOC, |s, _| step(s, false)).pure().when(has_changes),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{lines, session_with};

    fn session() -> Session {
        let mut s = session_with(&["A heading", "Rain falls hard."]);
        s.author = "Writer One".into();
        s.run("track.toggle", &json!({"on": true})).unwrap();
        s
    }

    fn at(block: usize, off: usize) -> jcj_doc::Pos {
        pos_at(&Path::top(block), off)
    }

    #[test]
    fn typing_and_deleting_are_tracked_with_who() {
        let mut s = session();
        s.sel = Selection::caret(at(1, 4));
        s.run("text.insert", &json!({"text": " still"})).unwrap();
        s.sel = Selection { anchor: at(1, 16), focus: at(1, 21) };
        s.run("text.backspace", &json!({})).unwrap();
        // Nothing is gone yet: the deletion is marked.
        assert_eq!(lines(&s)[1], "Rain still falls hard.");
        let list = s.run("track.list", &json!({})).unwrap();
        let ch = list["changes"].as_array().cloned().unwrap_or_default();
        assert_eq!(ch.len(), 2);
        assert_eq!(
            (ch[0]["kind"].as_str(), ch[0]["text"].as_str(), ch[0]["author"].as_str()),
            (Some("inserted"), Some(" still"), Some("Writer One"))
        );
        assert_eq!(ch[0]["color"], color_hex(DEFAULT_WRITER_COLORS[0]));
        assert_eq!((ch[1]["kind"].as_str(), ch[1]["text"].as_str()), (Some("deleted"), Some(" hard")));
    }

    #[test]
    fn accept_reject_and_step() {
        let mut s = session();
        s.sel = Selection::caret(at(1, 0));
        s.run("text.insert", &json!({"text": "Cold "})).unwrap();
        s.sel = Selection { anchor: at(1, 15), focus: at(1, 20) };
        s.run("text.delete", &json!({})).unwrap();
        s.sel = Selection::caret(at(0, 0));
        s.run("track.next", &json!({})).unwrap();
        assert_eq!(s.selected_text(), "Cold ");
        s.run("track.accept", &json!({})).unwrap();
        s.run("track.next", &json!({})).unwrap();
        assert_eq!(s.selected_text(), " hard");
        s.run("track.reject", &json!({})).unwrap();
        assert_eq!(lines(&s)[1], "Cold Rain falls hard.");
        assert!(changes(&s.doc).is_empty());
        // Accept all removes deletions for good.
        s.sel = Selection { anchor: at(1, 0), focus: at(1, 5) };
        s.run("text.backspace", &json!({})).unwrap();
        s.run("track.acceptAll", &json!({})).unwrap();
        assert_eq!(lines(&s)[1], "Rain falls hard.");
        assert!(s.run("track.accept", &json!({})).is_err());
        s.run("track.toggle", &json!({})).unwrap();
        assert!(!is_on(&s.doc));
    }

    #[test]
    fn writers_take_colors_in_order() {
        let mut doc = Document::new();
        for (who, kind) in [("B", jcj_doc::RevisionKind::Insert), ("A", jcj_doc::RevisionKind::Delete), ("B", jcj_doc::RevisionKind::Delete)] {
            doc.revisions.push(jcj_doc::Revision { kind, author: who.into(), date: String::new(), move_name: None });
        }
        let c = writer_colors_from(&doc, &[Rgb(1, 1, 1)]);
        assert_eq!(c, vec![("B".to_string(), Rgb(1, 1, 1)), ("A".to_string(), Rgb(1, 1, 1))]);
        assert_eq!(writer_colors(&doc)[1].1, DEFAULT_WRITER_COLORS[1]);
        assert!(writer_colors_from(&doc, &[]).iter().all(|(_, c)| *c == Rgb(0, 0, 0)));
    }
}
