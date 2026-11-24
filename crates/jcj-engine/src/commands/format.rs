// SPDX-License-Identifier: MIT OR Apache-2.0
//! Formatting commands the engine lacks.
//!
//! - `format.plain`: remove the text styles set by hand (bold, italic, underlines, strike, raised
//!   and lowered text, capitals, small capitals, highlight, shading, color), keeping the font, the
//!   size, the character style and every mark (extension data). The engine's
//!   `format.resetChar` also drops the font and size.
//! - `format.resetToStyle`: give the paragraphs the selection touches their style's formatting:
//!   direct paragraph and character formatting goes; the style, the paragraph's extension data
//!   and the runs' marks stay. The engine's `para.reset` resets only paragraph formatting, and
//!   `format.clear` also returns the paragraph to Normal (both keep extension data, through our
//!   fork's patch).
//! - `format.toggleCapitals`: the selection in capitals, or in small letters when it has no small
//!   letters. The engine's `format.changeCase {"mode": "toggle"}` swaps each letter instead.
//!
//! Like the engine's formatting commands, `format.plain` formats the selection, the word at a
//! caret inside a word, or else what is typed next.

use jcj_doc::props::CharProps;
use jcj_doc::{ParaProps, Pos};
use serde_json::Value;

use crate::cmd::sel_result;
use crate::{CmdError, CmdResult, CommandSpec, Session};

/// `c` without the text styles set by hand (see `format.plain`).
pub fn plain(c: &mut CharProps) {
    c.bold = None;
    c.bold_cs = None;
    c.italic = None;
    c.italic_cs = None;
    c.underline = None;
    c.underline_color = None;
    c.strike = None;
    c.double_strike = None;
    c.vert_align = None;
    c.caps = None;
    c.small_caps = None;
    c.highlight = None;
    c.shading = None;
    c.color = None;
}

fn plain_cmd(s: &mut Session, _: &Value) -> CmdResult {
    jcj_upstream::engine::cmd::format::apply(s, &plain)
}

/// `p` reset to its style: only the style and the extension data stay.
pub fn reset_paragraph(p: &ParaProps) -> ParaProps {
    ParaProps { style: p.style.clone(), ext: p.ext.clone(), ..Default::default() }
}

fn reset_to_style(s: &mut Session, _: &Value) -> CmdResult {
    let (a, b) = s.sel.ordered();
    let paths = if s.sel.is_collapsed() { vec![a.path.clone()] } else { s.doc.para_paths_in(&a, &b) };
    for path in paths {
        let para = s.doc.para_mut(a.story, &path)?;
        para.props = reset_paragraph(&para.props);
        let len = para.len();
        let start = Pos { story: a.story, path: path.clone(), off: 0 };
        let end = Pos { off: len, ..start.clone() };
        // Links, tracked changes and marks are not formatting: `cleared` keeps them.
        s.doc.format_range(&start, &end, &|c: &mut CharProps| *c = c.cleared())?;
        let para = s.doc.para_mut(a.story, &path)?;
        para.mark = para.mark.cleared();
        para.touch();
    }
    s.pending = None;
    sel_result(s)
}

fn has_selection(s: &Session) -> Option<&'static str> {
    if s.sel.is_collapsed() { Some("nothing is selected") } else { None }
}

fn toggle_capitals(s: &mut Session, _: &Value) -> CmdResult {
    if s.sel.is_collapsed() {
        return Err(CmdError::Failed("select some text first".into()));
    }
    let (a, b) = s.sel.ordered();
    let to_upper = s.selected_text().chars().any(char::is_lowercase);
    for path in s.doc.para_paths_in(&a, &b) {
        let para = s.doc.para_mut(a.story, &path)?;
        let from = if path == a.path { a.off } else { 0 };
        let to = if path == b.path { b.off } else { para.len() };
        let Some(piece) = para.text.get(from..to) else { continue };
        let changed = if to_upper { piece.to_uppercase() } else { piece.to_lowercase() };
        if changed == piece {
            continue;
        }
        if changed.len() == piece.len() {
            // Nearly always: the letters change in place and the runs are untouched.
            para.text.replace_range(from..to, &changed);
        } else {
            // A letter whose other case is longer (ß to SS): the piece takes its first
            // character's formatting.
            let props = para.props_of_char(from).clone();
            para.delete(from, to)?;
            para.insert_text(from, &changed, &props)?;
        }
        para.touch();
    }
    s.clamp_selection();
    sel_result(s)
}

/// The formatting commands.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("format.plain", "Plain Text", "Format › Font", plain_cmd),
        CommandSpec::new("format.resetToStyle", "Reset to Style", "Format › Paragraph", reset_to_style),
        CommandSpec::new("format.toggleCapitals", "Toggle Capitals", "Format › Font", toggle_capitals).when(has_selection),
    ]
}
