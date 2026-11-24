// SPDX-License-Identifier: MIT OR Apache-2.0
//! Editing commands the engine lacks.
//!
//! - `edit.newPage`: start a new page at the caret as a paragraph property ("page break
//!   before"), splitting the paragraph when the caret is inside it. At the start of a paragraph
//!   that already starts a page, the break is removed. The engine's `text.pageBreak` inserts a
//!   page-break character instead; apps whose pagination reads paragraph properties use this.
//! - `edit.deleteParagraph`: delete the caret's paragraph, as a tracked change while changes are
//!   tracked. A story always keeps one paragraph.
//! - `select.nextParagraph`, `select.previousParagraph`: select the next (previous) paragraph
//!   whole, for stepping through a document one paragraph at a time. At the last (first)
//!   paragraph the selection stays on it.

use jcj_doc::Pos;
use serde_json::Value;

use crate::cmd::{paste_text, sel_result};
use crate::{CmdError, CmdResult, CommandSpec, Selection, Session};

fn new_page(s: &mut Session, _: &Value) -> CmdResult {
    if !s.sel.is_collapsed() {
        paste_text(s, "")?;
    }
    let at = s.sel.focus.clone();
    let para = s.doc.para_at(&at).ok_or_else(|| CmdError::Failed("the caret is not in a paragraph".into()))?;
    let path = if at.off == 0 {
        if para.props.page_break_before == Some(true) {
            let p = s.doc.para_mut(at.story, &at.path)?;
            p.props.page_break_before = None;
            p.touch();
            return sel_result(s);
        }
        at.path.clone()
    } else {
        let new = s.doc.split_paragraph(&at)?;
        s.sel = Selection::caret(new.clone());
        new.path
    };
    let p = s.doc.para_mut(at.story, &path)?;
    p.props.page_break_before = Some(true);
    p.touch();
    sel_result(s)
}

fn delete_paragraph(s: &mut Session, _: &Value) -> CmdResult {
    let at = s.sel.focus.clone();
    let len = s.doc.para_at(&at).map(|p| p.len()).ok_or_else(|| CmdError::Failed("the caret is not in a paragraph".into()))?;
    if s.doc.settings.track_changes {
        // The paragraph and its mark, as one tracked deletion: up to the next paragraph's start,
        // or for the last paragraph from the previous one's end.
        let start = Pos { off: 0, ..at.clone() };
        let (a, b) = match (s.doc.next_para(at.story, &at.path), s.doc.prev_para(at.story, &at.path)) {
            (Some(next), _) => (start, Pos { story: at.story, path: next, off: 0 }),
            (None, Some(prev)) => {
                let off = s.doc.para(at.story, &prev).map_or(0, |p| p.len());
                (Pos { story: at.story, path: prev, off }, Pos { off: len, ..at.clone() })
            }
            (None, None) => (start, Pos { off: len, ..at.clone() }),
        };
        s.sel = Selection { anchor: a, focus: b };
        paste_text(s, "")?;
    } else {
        s.doc.remove_block(at.story, &at.path)?;
        s.clamp_selection();
        let to = Pos { off: 0, ..s.sel.focus.clone() };
        s.sel = Selection::caret(s.doc.clamp(&to));
    }
    s.pending = None;
    sel_result(s)
}

fn step_paragraph(s: &mut Session, forward: bool) -> CmdResult {
    let here = s.sel.focus.clone();
    let next = if forward { s.doc.next_para(here.story, &here.path) } else { s.doc.prev_para(here.story, &here.path) };
    let path = next.unwrap_or(here.path);
    let len = s.doc.para(here.story, &path).map_or(0, |p| p.len());
    s.sel = Selection { anchor: Pos { story: here.story, path: path.clone(), off: 0 }, focus: Pos { story: here.story, path, off: len } };
    s.pending = None;
    sel_result(s)
}

/// The editing commands.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("edit.newPage", "New Page Here", "Insert › Pages", new_page),
        CommandSpec::new("edit.deleteParagraph", "Delete Paragraph", "Edit", delete_paragraph),
        CommandSpec::new("select.nextParagraph", "Select Next Paragraph", "Navigation", |s, _| step_paragraph(s, true)).pure(),
        CommandSpec::new("select.previousParagraph", "Select Previous Paragraph", "Navigation", |s, _| step_paragraph(s, false)).pure(),
    ]
}
