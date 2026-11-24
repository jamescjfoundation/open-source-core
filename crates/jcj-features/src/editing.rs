// SPDX-License-Identifier: MIT OR Apache-2.0
//! Typing and deleting that keep the features' marks right.
//!
//! The engine's typing gives new text the formatting before the caret, marks included. For the
//! features' marks that is not always right, so this crate registers its own `text.insert`,
//! `text.newParagraph`, `text.backspace` and `text.delete`, which run the engine's and then:
//!
//! - **notes**: text typed at the edge of a noted range is not part of the note; typed inside
//!   it, it is;
//! - **revisions**: in revision mode typed text takes the current set's mark and a deletion marks
//!   its paragraph; out of it, text typed next to revised text is not revised;
//! - **page locks**: text typed at the very start of a locked page stays on it; a lock mark whose
//!   text was deleted moves to the character now at the caret, so the locked page still starts
//!   where its text goes on;
//! - **bookmarks**: a paragraph split in two keeps its bookmark on the first part only.
//!
//! Everything else (tracked changes, fields, content controls) is the engine's. Pasted text keeps
//! the marks it was copied with.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use jcj_doc::props::CharProps;
use jcj_doc::{Block, StoryRef};
use jcj_engine::{CmdError, CmdResult, CommandSpec, Session};
use serde_json::Value;

use crate::locks::{PAGE_LOCK, keep_marks, mark_ids};
use crate::notes::NOTE;
use crate::revisions::{REVISED, typing_set};

type RunFn = fn(&mut Session, &Value) -> CmdResult;

/// The ids this module takes over from the engine.
pub const WRAPPED: [&str; 4] = ["text.insert", "text.newParagraph", "text.backspace", "text.delete"];

fn engine(id: &str) -> Result<RunFn, CmdError> {
    static ENGINE: OnceLock<HashMap<&'static str, RunFn>> = OnceLock::new();
    let map = ENGINE.get_or_init(|| jcj_engine::engine_specs().into_iter().filter(|s| WRAPPED.contains(&s.id)).map(|s| (s.id, s.run)).collect());
    map.get(id).copied().ok_or_else(|| CmdError::Unknown(id.to_string()))
}

/// The formatting text typed now takes: the engine's ([`Session::typing_props`]) with the
/// features' marks put right (see the module documentation).
pub fn typing_props(s: &Session) -> CharProps {
    let mut c = s.typing_props();
    let (a, b) = s.sel.ordered();
    let pa = s.doc.para_at(&a);
    let pb = s.doc.para_at(&b);
    let after =
        |c: &CharProps| -> bool { pb.is_some_and(|p| b.off < p.len() && p.props_of_char(b.off).ext.get(NOTE.key()) == c.ext.get(NOTE.key())) };
    if NOTE.is_set(&c) {
        let before = a.off > 0 && pa.is_some_and(|p| p.props_at(a.off).ext.get(NOTE.key()) == c.ext.get(NOTE.key()));
        if !(before && after(&c)) {
            NOTE.remove(&mut c);
        }
    }
    if PAGE_LOCK.is_set(&c) {
        let at_start =
            a.off == 0 && pb.is_some_and(|p| b.off < p.len() && p.props_of_char(b.off).ext.get(PAGE_LOCK.key()) == c.ext.get(PAGE_LOCK.key()));
        if !at_start {
            PAGE_LOCK.remove(&mut c);
        }
    }
    match typing_set(&s.doc).filter(|_| a.story == StoryRef::Body) {
        Some(set) => {
            let _ = REVISED.set(&mut c, &set);
        }
        None => REVISED.remove(&mut c),
    }
    c
}

/// Whether the body changed: blocks the edit touched are copies, so their addresses differ.
fn body_changed(before: &[Arc<Block>], after: &[Arc<Block>]) -> bool {
    before.len() != after.len() || before.iter().zip(after).any(|(x, y)| !Arc::ptr_eq(x, y))
}

/// Lock ids before an edit, when the document is locked.
fn locks_before(s: &Session) -> Option<Vec<u32>> {
    crate::locks::Locks::of_doc(&s.doc).is_locked().then(|| mark_ids(&s.doc))
}

/// Put lock marks the edit deleted back at the caret.
fn restore_locks(s: &mut Session, before: Option<Vec<u32>>) {
    let Some(before) = before else { return };
    let now = mark_ids(&s.doc);
    let gone: Vec<u32> = before.into_iter().filter(|id| !now.contains(id)).collect();
    if !gone.is_empty() {
        let at = s.sel.focus.clone();
        keep_marks(&mut s.doc, &gone, &at);
    }
}

fn insert(s: &mut Session, v: &Value) -> CmdResult {
    let locks = locks_before(s);
    let props = typing_props(s);
    let held = s.pending.replace(props);
    let out = engine("text.insert").and_then(|run| run(s, v));
    s.pending = held;
    let out = out?;
    restore_locks(s, locks);
    crate::bookmarks::keep_unique(&mut s.doc);
    Ok(out)
}

fn new_paragraph(s: &mut Session, v: &Value) -> CmdResult {
    let out = engine("text.newParagraph").and_then(|run| run(s, v))?;
    crate::bookmarks::keep_unique(&mut s.doc);
    Ok(out)
}

fn delete_with(s: &mut Session, v: &Value, id: &str) -> CmdResult {
    let locks = locks_before(s);
    let before = s.doc.body.clone();
    let out = engine(id).and_then(|run| run(s, v))?;
    if body_changed(&before, &s.doc.body) {
        let at = s.sel.focus.clone();
        crate::revisions::touch(&mut s.doc, &at);
        restore_locks(s, locks);
    }
    Ok(out)
}

/// The engine's typing commands with the features' marks kept right (see the module
/// documentation). Labels, keys and parameters are the engine's.
pub fn commands() -> Vec<CommandSpec> {
    let mut out = Vec::new();
    for spec in jcj_engine::engine_specs().into_iter().filter(|s| WRAPPED.contains(&s.id)) {
        let run: RunFn = match spec.id {
            "text.insert" => insert,
            "text.newParagraph" => new_paragraph,
            "text.backspace" => |s, v| delete_with(s, v, "text.backspace"),
            _ => |s, v| delete_with(s, v, "text.delete"),
        };
        out.push(CommandSpec { run, ..spec });
    }
    out
}
