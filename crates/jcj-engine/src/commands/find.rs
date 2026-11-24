// SPDX-License-Identifier: MIT OR Apache-2.0
//! Literal find and replace in the body.
//!
//! The text is found as typed (no wildcards or special codes); "match case" and "whole word" are
//! options, and a search can be limited to paragraphs of some styles. Matches lie within one
//! paragraph, in tables too. Find Next and Find Previous wrap round the body. Replace All is one
//! undo step, and while changes are tracked each replacement is a tracked change.
//!
//! The engine's `edit.find`, `edit.findNext`, `edit.replaceAll` (Word's find, with wildcards,
//! regular expressions and `^p` codes) stay available beside these.

use std::ops::Range;

use jcj_doc::{Document, Pos, StoryRef};
use serde_json::{Value, json};

use crate::cmd::{paste_text, pos_json, sel_result};
use crate::{CmdError, CmdResult, CommandSpec, Selection, Session, params as p};

/// Longest search or replacement text, in bytes.
pub const MAX_QUERY: usize = 1_000;
/// Most matches one search returns (and Replace All replaces).
pub const MAX_MATCHES: usize = 100_000;
/// Most styles a search may be limited to.
pub const MAX_STYLE_FILTER: usize = 128;

/// What to look for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
    /// The text, on one line.
    pub text: String,
    /// Upper and lower case must match.
    pub match_case: bool,
    /// Only whole words match: no letter, digit or apostrophe just before or after.
    pub whole_word: bool,
    /// Look only in paragraphs of these styles (ids or names); empty means every paragraph.
    pub styles: Vec<String>,
}

impl Query {
    /// A query from command parameters: `query`, and optionally `matchCase`, `wholeWord`,
    /// `styles`.
    pub fn from_params(v: &Value) -> Result<Query, CmdError> {
        let text = p::req_str(v, "query")?;
        if text.is_empty() || text.len() > MAX_QUERY || text.contains(['\n', '\r']) {
            return Err(CmdError::Params(format!("`query` is 1 to {MAX_QUERY} bytes on one line")));
        }
        Ok(Query {
            text: text.to_string(),
            match_case: p::bool(v, "matchCase").unwrap_or(false),
            whole_word: p::bool(v, "wholeWord").unwrap_or(false),
            styles: v
                .get("styles")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).take(MAX_STYLE_FILTER).map(str::to_string).collect())
                .unwrap_or_default(),
        })
    }
}

fn same(a: char, b: char, case: bool) -> bool {
    if case { a == b } else { a == b || a.to_lowercase().eq(b.to_lowercase()) }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '\'' || c == '\u{2019}'
}

/// The byte ranges of `q` in `text`, in order, not overlapping. The style filter is not used.
pub fn find_in(text: &str, q: &Query) -> Vec<Range<usize>> {
    let needle: Vec<char> = q.text.chars().collect();
    let hay: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    if needle.is_empty() {
        return out;
    }
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        let hit = needle.iter().enumerate().all(|(k, &n)| hay.get(i + k).is_some_and(|&(_, c)| same(c, n, q.match_case)));
        if hit {
            let start = hay.get(i).map_or(0, |h| h.0);
            let end = hay.get(i + needle.len()).map_or(text.len(), |h| h.0);
            let before_ok = i == 0 || hay.get(i - 1).is_none_or(|&(_, c)| !is_word(c));
            let after_ok = hay.get(i + needle.len()).is_none_or(|&(_, c)| !is_word(c));
            if !q.whole_word || (before_ok && after_ok) {
                out.push(start..end);
                i += needle.len();
                continue;
            }
        }
        i += 1;
    }
    out
}

/// The style ids `q` is limited to (names looked up in `doc`'s styles), or `None` for every
/// paragraph.
fn style_ids(doc: &Document, q: &Query) -> Option<Vec<String>> {
    if q.styles.is_empty() {
        return None;
    }
    Some(q.styles.iter().map(|s| doc.styles.find(s).map_or_else(|| s.clone(), |st| st.id.clone())).collect())
}

/// Every match of `q` in `doc`'s body, in document order, at most [`MAX_MATCHES`].
pub fn find_all(doc: &Document, q: &Query) -> Vec<(Pos, Pos)> {
    let styles = style_ids(doc, q);
    let mut out = Vec::new();
    for path in doc.para_paths(StoryRef::Body) {
        let Some(para) = doc.para(StoryRef::Body, &path) else { continue };
        if let Some(ids) = &styles {
            let id = para.props.style.as_deref().unwrap_or("Normal");
            if !ids.iter().any(|s| s == id) {
                continue;
            }
        }
        // Text deleted while changes were tracked is not there to find.
        let deleted: Vec<Range<usize>> = para.run_ranges().filter(|(_, c)| c.del.is_some()).map(|(r, _)| r).collect();
        for r in find_in(&para.text, q) {
            if deleted.iter().any(|d| d.start < r.end && r.start < d.end) {
                continue;
            }
            let at = |off| Pos { story: StoryRef::Body, path: path.clone(), off };
            out.push((at(r.start), at(r.end)));
            if out.len() >= MAX_MATCHES {
                return out;
            }
        }
    }
    out
}

fn match_json(a: &Pos, b: &Pos) -> Value {
    json!({"start": pos_json(a), "end": pos_json(b)})
}

/// `find.all`: every match, without moving the selection.
fn all(s: &mut Session, v: &Value) -> CmdResult {
    let q = Query::from_params(v)?;
    let found: Vec<Value> = find_all(&s.doc, &q).iter().map(|(a, b)| match_json(a, b)).collect();
    Ok(json!({"count": found.len(), "matches": found}))
}

/// Select the next (or previous) match from the selection, wrapping round the body.
fn step(s: &mut Session, q: &Query, forward: bool) -> CmdResult {
    let found = find_all(&s.doc, q);
    if found.is_empty() {
        return Err(CmdError::Failed(format!("`{}` was not found", q.text)));
    }
    let (a, b) = s.sel.ordered();
    // From a selected match, move past it; otherwise start at the selection.
    let on_match = found.iter().any(|(ma, mb)| *ma == a && *mb == b);
    let pick = if forward {
        found.iter().find(|(m, _)| if on_match { *m >= b && *m != a } else { *m >= a }).or_else(|| found.first())
    } else {
        found.iter().rev().find(|(m, _)| *m < a).or_else(|| found.last())
    };
    if let Some((ma, mb)) = pick.cloned() {
        s.sel = Selection { anchor: ma, focus: mb };
        s.pending = None;
    }
    let mut r = sel_result(s)?;
    if let Some(o) = r.as_object_mut() {
        o.insert("count".into(), json!(found.len()));
    }
    Ok(r)
}

fn replacement(v: &Value) -> Result<String, CmdError> {
    let with = p::req_str(v, "with")?;
    if with.len() > MAX_QUERY || with.contains(['\n', '\r']) {
        return Err(CmdError::Params(format!("`with` is at most {MAX_QUERY} bytes on one line")));
    }
    Ok(with.to_string())
}

/// Replace `a..b` with `text` as typing over a selection would: the first replaced character's
/// formatting, and a tracked change while changes are tracked. Leaves the caret after it.
pub fn replace_range(s: &mut Session, a: &Pos, b: &Pos, text: &str) -> Result<(), CmdError> {
    s.sel = Selection { anchor: a.clone(), focus: b.clone() };
    s.pending = None;
    paste_text(s, text)
}

/// `find.replace`: replace the selected match (if the selection is one), then select the next.
fn replace_one(s: &mut Session, v: &Value) -> CmdResult {
    let q = Query::from_params(v)?;
    let with = replacement(v)?;
    let (a, b) = s.sel.ordered();
    let replaced = find_all(&s.doc, &q).iter().any(|(ma, mb)| *ma == a && *mb == b);
    if replaced {
        replace_range(s, &a, &b, &with)?;
    }
    // Then on to the next match, if any is left.
    let _ = step(s, &q, true);
    let mut r = sel_result(s)?;
    if let Some(o) = r.as_object_mut() {
        o.insert("replaced".into(), json!(replaced));
    }
    Ok(r)
}

/// Replace every match of `q`, last first, with the text `with` gives for it (`with` sees the
/// document and the match's start; apps use it to apply a paragraph style's casing). Returns how
/// many were replaced. Run inside a command, this is one undo step.
pub fn replace_all_with(s: &mut Session, q: &Query, with: &dyn Fn(&Document, &Pos) -> String) -> Result<usize, CmdError> {
    let found = find_all(&s.doc, q);
    for (a, b) in found.iter().rev() {
        let text = with(&s.doc, a);
        replace_range(s, a, b, &text)?;
    }
    s.clamp_selection();
    Ok(found.len())
}

/// `find.replaceAll`: every match, as one undo step.
fn replace_all(s: &mut Session, v: &Value) -> CmdResult {
    let q = Query::from_params(v)?;
    let with = replacement(v)?;
    let n = replace_all_with(s, &q, &|_, _| with.clone())?;
    Ok(json!({"replaced": n}))
}

/// The find and replace commands.
pub fn specs() -> Vec<CommandSpec> {
    const LOC: &str = "Edit › Find";
    const FIND: &str = r#"{"query": string, "matchCase"?: bool, "wholeWord"?: bool, "styles"?: [style id or name]}"#;
    const REPLACE: &str = r#"{"query": string, "with": string, "matchCase"?: bool, "wholeWord"?: bool, "styles"?: [style id or name]}"#;
    vec![
        CommandSpec::new("find.all", "Find All", LOC, all).params(FIND).pure(),
        CommandSpec::new("find.next", "Find Next", LOC, |s, v| step(s, &Query::from_params(v)?, true)).params(FIND).pure(),
        CommandSpec::new("find.previous", "Find Previous", LOC, |s, v| step(s, &Query::from_params(v)?, false)).params(FIND).pure(),
        CommandSpec::new("find.replace", "Replace", LOC, replace_one).params(REPLACE),
        CommandSpec::new("find.replaceAll", "Replace All", LOC, replace_all).params(REPLACE),
    ]
}
