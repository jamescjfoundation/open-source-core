// SPDX-License-Identifier: MIT OR Apache-2.0
//! Revision mode and revision sets: marking what changed in each round of revisions.
//!
//! The settings live in the document under [`REVISIONS`] (`core.revisions`): the sets (name,
//! color, text styles, own margin mark), the current set, whether revision mode is on, the margin
//! mark and where it sits, whether pages are colored, and the color cycle new sets take. Revised
//! text carries a run mark ([`REVISED`], `core.revised`: the set's index), so it moves with the
//! text; a deletion marks its paragraph (the same key on the paragraph mark). The original draft
//! carries no marks.
//!
//! **The color cycle** new sets follow is a setting. Left empty it is [`DEFAULT_CYCLE`], the
//! revision color sequence long used in production work (White, Blue, Pink, Yellow, ...); an app
//! or a document may give its own list instead (`revision.configure {"cycle": [...]}`, or
//! [`Revisions::cycle`]).
//!
//! Typing and deleting in revision mode are marked by the editing commands this crate puts in
//! place of the engine's (`crate::editing`). For pages, [`page_sets`] finds the newest set on
//! each page and [`decorate_output`] applies the sets' text styles and page colors to a
//! paginated output document.
//!
//! Ported from the first foundation app's revisions (its decision 0024), with the marks moved into
//! extension data and the cycle made a setting.

use std::collections::{BTreeSet, HashSet};

use jcj_doc::fill::Fill;
use jcj_doc::para::{Anchor, Float, ShapeKind, Wrap};
use jcj_doc::props::{CharProps, Underline};
use jcj_doc::walk::body_paragraphs;
use jcj_doc::{Block, Document, ExtKey, InlineObject, Path, Pos, StoryRef};
use jcj_engine::{CmdError, CmdResult, CommandSpec, Selection, Session, cmd::sel_result, params as p};
use jcj_layout::paginate::Piece;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::data::{clean_line, load, parse_color, pos_at, store};
use crate::marks::{mark_range, unmark_everywhere, unmark_range};

/// Run mark (and paragraph mark, for deletions): the index of the revision set in
/// [`Revisions::sets`].
pub const REVISED: ExtKey<u32> = ExtKey::new("core.revised");
/// Document data: the revision settings ([`Revisions`]).
pub const REVISIONS: ExtKey<Revisions> = ExtKey::new("core.revisions");
/// Most sets a document keeps.
pub const MAX_SETS: usize = 100;
/// Most colors a cycle holds.
pub const MAX_CYCLE: usize = 100;
/// Longest set name, characters.
pub const MAX_NAME_CHARS: usize = 40;

/// The default color cycle: the revision color sequence long used in production work. Names
/// and the colors pages and PDF use for them. The first is the original draft.
pub const DEFAULT_CYCLE: [(&str, &str); 10] = [
    ("White", "#FFFFFF"),
    ("Blue", "#A9C8F2"),
    ("Pink", "#F6B8C8"),
    ("Yellow", "#FBF09A"),
    ("Green", "#B9E3B0"),
    ("Goldenrod", "#E9C46A"),
    ("Buff", "#F1DFB3"),
    ("Salmon", "#F6A98E"),
    ("Cherry", "#E3707F"),
    ("Tan", "#D5B892"),
];

/// Words for the later rounds of a cycle ("Second Blue").
const ROUNDS: [&str; 10] = ["", "Second", "Third", "Fourth", "Fifth", "Sixth", "Seventh", "Eighth", "Ninth", "Tenth"];

/// One color of a cycle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CycleColor {
    pub name: String,
    /// `#RRGGBB`.
    pub color: String,
}

/// [`DEFAULT_CYCLE`] as a list.
pub fn default_cycle() -> Vec<CycleColor> {
    DEFAULT_CYCLE.iter().map(|(n, c)| CycleColor { name: (*n).into(), color: (*c).into() }).collect()
}

/// One revision set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RevisionSet {
    pub name: String,
    /// `#RRGGBB`.
    pub color: String,
    pub bold: bool,
    pub underline: bool,
    pub strike: bool,
    /// This set's own margin mark; empty for the document's.
    pub mark: String,
}

impl Default for RevisionSet {
    fn default() -> Self {
        cycle_set(&default_cycle(), 1)
    }
}

/// The set at position `n` of `cycle` (0 is its first color; after the last color the cycle
/// starts again with "Second": with the default cycle, 10 is Second White).
pub fn cycle_set(cycle: &[CycleColor], n: usize) -> RevisionSet {
    let fallback = CycleColor { name: "White".into(), color: "#FFFFFF".into() };
    let len = cycle.len().max(1);
    let c = cycle.get(n % len).unwrap_or(&fallback);
    let round = n / len;
    let name = match ROUNDS.get(round) {
        Some(&"") => c.name.clone(),
        Some(r) => format!("{r} {}", c.name),
        None => format!("{} {}", c.name, round + 1),
    };
    RevisionSet { name, color: c.color.clone(), bold: false, underline: false, strike: false, mark: String::new() }
}

/// The revision settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Revisions {
    /// The sets made so far, oldest first; marks refer to them by index.
    pub sets: Vec<RevisionSet>,
    /// The current set's name (empty: the original draft).
    pub current: String,
    /// Revision mode: typing and deleting are marked in the current set.
    pub mode: bool,
    /// The margin mark.
    pub mark: String,
    /// Where the mark sits, inches from the page's left edge.
    pub mark_at: f32,
    /// Color whole pages in output with the newest set on them.
    pub color_pages: bool,
    /// Show the margin marks (off: print without marks).
    pub show_marks: bool,
    /// Show only the current set's marks, styles and page colors; off shows every set.
    pub active_only: bool,
    /// The colors new sets take, in order; empty for [`DEFAULT_CYCLE`].
    pub cycle: Vec<CycleColor>,
}

impl Default for Revisions {
    fn default() -> Self {
        Revisions {
            sets: Vec::new(),
            current: String::new(),
            mode: false,
            mark: "*".into(),
            mark_at: 8.0,
            color_pages: false,
            show_marks: true,
            active_only: false,
            cycle: Vec::new(),
        }
    }
}

/// The first printable character of `s`, if any.
fn one_char(s: &str) -> Option<String> {
    s.chars().find(|c| !c.is_control() && !c.is_whitespace()).map(|c| c.to_string())
}

impl Revisions {
    /// The settings `doc` holds, or the defaults; bad values are brought within limits.
    pub fn of_doc(doc: &Document) -> Self {
        let mut r = load(doc, REVISIONS);
        r.sets.truncate(MAX_SETS);
        for s in &mut r.sets {
            s.name = clean_line(&s.name, MAX_NAME_CHARS);
            s.mark = one_char(&s.mark).unwrap_or_default();
            if parse_color(&s.color).is_none() {
                s.color = "#FFFFFF".into();
            }
        }
        r.cycle.truncate(MAX_CYCLE);
        r.cycle.retain(|c| parse_color(&c.color).is_some());
        for c in &mut r.cycle {
            c.name = clean_line(&c.name, MAX_NAME_CHARS);
        }
        r.mark = one_char(&r.mark).unwrap_or_else(|| "*".into());
        if !r.mark_at.is_finite() {
            r.mark_at = 8.0;
        }
        r.mark_at = r.mark_at.clamp(0.1, 8.0);
        if r.current_index().is_none() {
            r.current = String::new();
        }
        r
    }

    pub fn store(&self, doc: &mut Document) -> Result<(), CmdError> {
        store(doc, REVISIONS, self)
    }

    /// The cycle new sets follow.
    pub fn cycle(&self) -> Vec<CycleColor> {
        if self.cycle.is_empty() { default_cycle() } else { self.cycle.clone() }
    }

    /// The current set's index, if there is one.
    pub fn current_index(&self) -> Option<u32> {
        self.sets.iter().position(|s| s.name == self.current).and_then(|i| u32::try_from(i).ok())
    }

    /// Add the next set in the cycle and make it current.
    pub fn start_next(&mut self) -> Option<u32> {
        if self.sets.len() >= MAX_SETS {
            return None;
        }
        let set = cycle_set(&self.cycle(), self.sets.len() + 1);
        self.current.clone_from(&set.name);
        self.sets.push(set);
        u32::try_from(self.sets.len() - 1).ok()
    }

    /// The set marks with index `i` belong to.
    pub fn set(&self, i: u32) -> Option<&RevisionSet> {
        self.sets.get(i as usize)
    }

    /// Whether marks of set `i` show (all sets, or only the current one).
    pub fn shows(&self, i: u32) -> bool {
        !self.active_only || self.current_index() == Some(i)
    }
}

/// The set new text takes now: the current one, in revision mode.
pub fn typing_set(doc: &Document) -> Option<u32> {
    let r = Revisions::of_doc(doc);
    if r.mode { r.current_index() } else { None }
}

/// In revision mode, mark the paragraph at `at` as changed (text was deleted there).
pub fn touch(doc: &mut Document, at: &Pos) {
    let Some(set) = typing_set(doc) else { return };
    if at.story != StoryRef::Body {
        return;
    }
    if let Ok(p) = doc.para_mut(at.story, &at.path) {
        let _ = REVISED.set(&mut p.mark, &set);
        p.touch();
    }
}

/// Every revision set with marks in the document, oldest first.
pub fn sets_in_use(doc: &Document) -> BTreeSet<u32> {
    let mut used = BTreeSet::new();
    for (_, p) in body_paragraphs(doc) {
        used.extend(REVISED.get_or_none(&p.mark));
        used.extend(p.run_ranges().filter_map(|(_, c)| REVISED.get_or_none(c)));
    }
    used
}

/// The names of the sets with marks in the document, oldest first, joined with commas.
pub fn collated(doc: &Document) -> String {
    let rev = Revisions::of_doc(doc);
    sets_in_use(doc).into_iter().filter_map(|i| rev.set(i).map(|s| s.name.clone())).collect::<Vec<_>>().join(", ")
}

/// The newest set with marks on each planned page (shown sets only); `None` for a page with none.
pub fn page_sets(doc: &Document, pages: &[Vec<Piece>]) -> Vec<Option<u32>> {
    let rev = Revisions::of_doc(doc);
    pages
        .iter()
        .map(|pg| {
            pg.iter()
                .filter_map(|pc| match pc {
                    Piece::Source { part, .. } => doc.body.get(part.block).and_then(|b| b.as_para()).map(|p| (p, part.start, part.end)),
                    Piece::Added { .. } => None,
                })
                .flat_map(|(p, a, b)| {
                    let whole = b >= p.len();
                    let runs = p.run_ranges().filter(move |(r, _)| r.start < b && r.end > a).filter_map(|(_, c)| REVISED.get_or_none(c));
                    runs.chain(if whole { REVISED.get_or_none(&p.mark) } else { None }).collect::<Vec<_>>()
                })
                .filter(|i| rev.shows(*i))
                .max()
        })
        .collect()
}

/// A rectangle the size of the page, behind the text, in `color`.
fn page_fill(out: &Document, color: jcj_doc::Rgb) -> InlineObject {
    let s = &out.last_section;
    let float = Float { wrap: Wrap::BehindText, h_rel: Anchor::Page, v_rel: Anchor::Page, x: 0.0, y: 0.0, ..Float::default() };
    InlineObject::Shape {
        kind: ShapeKind::Rectangle,
        w: s.page_w,
        h: s.page_h,
        fill: Some(Fill::Solid { color, transparency: 0.0 }),
        stroke: None,
        stroke_width: 0.0,
        float,
        story: None,
        freeform: None,
        effects: Default::default(),
        extra: Default::default(),
    }
}

/// Apply the revision sets to a paginated output document (in a pagination pass's `finish`):
/// each revised run takes its set's text styles (bold, underline, strike), and with
/// [`Revisions::color_pages`] each page with revised text gets a page-size fill in its newest
/// set's color. Pages start at the first paragraph and at each paragraph that breaks the page.
/// Text offsets are not changed; the page fill is an object added at the end of the page's first
/// paragraph.
pub fn decorate_output(out: &mut Document) {
    let rev = Revisions::of_doc(out);
    if rev.sets.is_empty() {
        return;
    }
    let mut page_first: Option<usize> = None;
    let mut page_newest: Option<u32> = None;
    let mut fills: Vec<(usize, u32)> = Vec::new();
    for i in 0..out.body.len() {
        let Some(Block::Para(p)) = out.body.get_mut(i).map(std::sync::Arc::make_mut) else { continue };
        if i == 0 || p.props.page_break_before == Some(true) {
            if let (Some(f), Some(n)) = (page_first, page_newest) {
                fills.push((f, n));
            }
            page_first = Some(i);
            page_newest = None;
        }
        let mut changed = false;
        let len = p.len();
        let marks: Vec<(std::ops::Range<usize>, u32)> = p.run_ranges().filter_map(|(r, c)| REVISED.get_or_none(c).map(|s| (r, s))).collect();
        for (r, set) in marks.into_iter().filter(|(_, s)| rev.shows(*s)) {
            page_newest = page_newest.max(Some(set));
            let Some(st) = rev.set(set) else { continue };
            if st.bold || st.underline || st.strike {
                let (b, u, k) = (st.bold, st.underline, st.strike);
                let _ = p.format(r.start, r.end.min(len), &|c: &mut CharProps| {
                    if b {
                        c.bold = Some(true);
                    }
                    if u {
                        c.underline = Some(Underline::Single);
                    }
                    if k {
                        c.strike = Some(true);
                    }
                });
                changed = true;
            }
        }
        if let Some(set) = REVISED.get_or_none(&p.mark).filter(|s| rev.shows(*s)) {
            page_newest = page_newest.max(Some(set));
        }
        if changed {
            p.touch();
        }
    }
    if let (Some(f), Some(n)) = (page_first, page_newest) {
        fills.push((f, n));
    }
    if !rev.color_pages {
        return;
    }
    for (block, set) in fills {
        let Some(color) = rev.set(set).and_then(|s| parse_color(&s.color)) else { continue };
        let fill = page_fill(out, color);
        if let Some(Block::Para(p)) = out.body.get_mut(block).map(std::sync::Arc::make_mut) {
            let at = p.len();
            let _ = p.insert_object(at, fill, &CharProps::default());
            p.touch();
        }
    }
}

/// `revision.settings`: the settings.
fn settings(s: &mut Session, _: &Value) -> CmdResult {
    Ok(serde_json::to_value(Revisions::of_doc(&s.doc)).unwrap_or_default())
}

/// `revision.mode {on?}`: switch revision mode (on, off, or the other way). Turning it on with no
/// set starts the first.
fn mode(s: &mut Session, v: &Value) -> CmdResult {
    let mut r = Revisions::of_doc(&s.doc);
    r.mode = p::bool(v, "on").unwrap_or(!r.mode);
    if r.mode && r.current_index().is_none() {
        r.start_next();
    }
    r.store(&mut s.doc)?;
    Ok(json!({"mode": r.mode, "current": r.current}))
}

/// `revision.next`: start the next set in the cycle, make it current, and turn revision mode on.
fn next(s: &mut Session, _: &Value) -> CmdResult {
    let mut r = Revisions::of_doc(&s.doc);
    if r.start_next().is_none() {
        return Err(CmdError::Failed(format!("a document keeps at most {MAX_SETS} revision sets")));
    }
    r.mode = true;
    r.store(&mut s.doc)?;
    Ok(json!({"current": r.current}))
}

/// `revision.setCurrent {name}`: make an existing set current.
fn set_current(s: &mut Session, v: &Value) -> CmdResult {
    let name = p::req_str(v, "name")?;
    let mut r = Revisions::of_doc(&s.doc);
    if !r.sets.iter().any(|x| x.name == name) {
        return Err(CmdError::Params(format!("there is no revision set `{name}`")));
    }
    r.current = name.to_string();
    r.store(&mut s.doc)?;
    Ok(json!({"current": r.current}))
}

fn configure_set(r: &mut Revisions, set: &Value) -> Result<(), CmdError> {
    let i = set.get("index").and_then(Value::as_u64).and_then(|i| usize::try_from(i).ok());
    let current = r.current.clone();
    let target = i.and_then(|i| r.sets.get_mut(i)).ok_or_else(|| CmdError::Params("`set.index` must name a set".into()))?;
    let was_current = target.name == current;
    if let Some(n) = set.get("name").and_then(Value::as_str) {
        let n = clean_line(n, MAX_NAME_CHARS);
        if n.is_empty() {
            return Err(CmdError::Params("a set needs a name".into()));
        }
        target.name = n;
    }
    if let Some(c) = set.get("color").and_then(Value::as_str) {
        parse_color(c).ok_or_else(|| CmdError::Params("`color` must be #RRGGBB".into()))?;
        target.color = c.trim().to_uppercase();
    }
    if let Some(m) = set.get("mark").and_then(Value::as_str) {
        target.mark = one_char(m).unwrap_or_default();
    }
    for (key, field) in [("bold", &mut target.bold), ("underline", &mut target.underline), ("strike", &mut target.strike)] {
        if let Some(b) = set.get(key).and_then(Value::as_bool) {
            *field = b;
        }
    }
    if was_current {
        r.current = target.name.clone();
    }
    let mut names = HashSet::new();
    if !r.sets.iter().all(|x| names.insert(&x.name)) {
        return Err(CmdError::Params("two sets cannot share a name".into()));
    }
    Ok(())
}

fn parse_cycle(v: &Value) -> Result<Vec<CycleColor>, CmdError> {
    let list = v.as_array().ok_or_else(|| CmdError::Params("`cycle` is a list of {name, color}".into()))?;
    if list.len() > MAX_CYCLE {
        return Err(CmdError::Params(format!("a cycle holds at most {MAX_CYCLE} colors")));
    }
    list.iter()
        .map(|c| {
            let name = c.get("name").and_then(Value::as_str).map(|n| clean_line(n, MAX_NAME_CHARS)).filter(|n| !n.is_empty());
            let color = c.get("color").and_then(Value::as_str).filter(|c| parse_color(c).is_some());
            match (name, color) {
                (Some(name), Some(color)) => Ok(CycleColor { name, color: color.trim().to_uppercase() }),
                _ => Err(CmdError::Params("each cycle color needs a name and a #RRGGBB color".into())),
            }
        })
        .collect()
}

/// `revision.configure {mark?, markAt?, colorPages?, showMarks?, activeOnly?, cycle?, set?}`.
fn configure(s: &mut Session, v: &Value) -> CmdResult {
    let mut r = Revisions::of_doc(&s.doc);
    if let Some(m) = p::str(v, "mark") {
        r.mark = one_char(m).ok_or_else(|| CmdError::Params("`mark` must be a character".into()))?;
    }
    if let Some(x) = v.get("markAt").and_then(Value::as_f64) {
        if !(0.1..=8.0).contains(&x) {
            return Err(CmdError::Params("`markAt` is inches from the page's left edge, 0.1 to 8".into()));
        }
        r.mark_at = x as f32;
    }
    for (key, field) in [("colorPages", &mut r.color_pages), ("showMarks", &mut r.show_marks), ("activeOnly", &mut r.active_only)] {
        if let Some(b) = p::bool(v, key) {
            *field = b;
        }
    }
    if let Some(c) = v.get("cycle") {
        r.cycle = if c.is_null() { Vec::new() } else { parse_cycle(c)? };
    }
    if let Some(set) = v.get("set") {
        configure_set(&mut r, set)?;
    }
    r.store(&mut s.doc)?;
    Ok(serde_json::to_value(&r).unwrap_or_default())
}

/// The selection, or with a caret its whole paragraph.
fn range_or_paragraph(s: &Session) -> (Pos, Pos, bool) {
    let (mut a, mut b) = s.sel.ordered();
    let caret = a == b;
    if caret {
        a.off = 0;
        b.off = s.doc.para_at(&b).map_or(0, |p| p.len());
    }
    (a, b, caret)
}

/// `revision.markRevised`: mark the selection (or, with a caret, its paragraph) revised in the
/// current set.
fn mark_revised(s: &mut Session, _: &Value) -> CmdResult {
    let mut r = Revisions::of_doc(&s.doc);
    if r.current_index().is_none() {
        r.start_next();
        r.store(&mut s.doc)?;
    }
    let set = r.current_index().ok_or_else(|| CmdError::Failed("there is no revision set to mark with".into()))?;
    let (a, b, caret) = range_or_paragraph(s);
    if caret && let Ok(p) = s.doc.para_mut(a.story, &a.path) {
        let _ = REVISED.set(&mut p.mark, &set);
        p.touch();
    }
    mark_range(&mut s.doc, &a, &b, REVISED, &set)?;
    Ok(json!({"set": r.current}))
}

/// `revision.clearRevised {all?}`: clear revision marks in the selection (or its paragraph), or
/// everywhere. Revision mode stays as it is.
fn clear_revised(s: &mut Session, v: &Value) -> CmdResult {
    if p::bool(v, "all") == Some(true) {
        unmark_everywhere(&mut s.doc, REVISED, &|_| true)?;
        return Ok(Value::Null);
    }
    let (a, b, _) = range_or_paragraph(s);
    unmark_range(&mut s.doc, &a, &b, REVISED, &|_| true)?;
    Ok(Value::Null)
}

/// `revision.rollback {to}`: go back to set `to` (an empty name: the original draft): later sets'
/// marks are cleared and the sets removed. The text is not changed.
fn rollback(s: &mut Session, v: &Value) -> CmdResult {
    let to = p::req_str(v, "to")?;
    let mut r = Revisions::of_doc(&s.doc);
    let keep = if to.is_empty() {
        0
    } else {
        r.sets.iter().position(|x| x.name == to).map(|i| i + 1).ok_or_else(|| CmdError::Params(format!("there is no revision set `{to}`")))?
    };
    let keep_u = u32::try_from(keep).unwrap_or(u32::MAX);
    unmark_everywhere(&mut s.doc, REVISED, &|set| *set >= keep_u)?;
    r.sets.truncate(keep);
    r.current = to.to_string();
    if keep == 0 {
        r.mode = false;
    }
    r.store(&mut s.doc)?;
    Ok(json!({"current": r.current}))
}

/// Every stretch of revised text in the body, in order: (paragraph, start, end). A paragraph
/// whose only mark is on its paragraph mark (a deletion) is one empty stretch at its start.
pub fn revised_spots(doc: &Document) -> Vec<(Path, usize, usize)> {
    let mut spots: Vec<(Path, usize, usize)> = Vec::new();
    for (path, p) in body_paragraphs(doc) {
        let runs: Vec<std::ops::Range<usize>> = p.run_ranges().filter(|(_, c)| REVISED.is_set(*c)).map(|(r, _)| r).collect();
        if runs.is_empty() {
            if REVISED.is_set(&p.mark) {
                spots.push((path, 0, 0));
            }
            continue;
        }
        let mut last_end = None;
        for r in runs {
            match spots.last_mut() {
                Some(x) if x.0 == path && last_end == Some(r.start) => x.2 = r.end,
                _ => spots.push((path.clone(), r.start, r.end)),
            }
            last_end = Some(r.end);
        }
    }
    spots
}

/// The next (or previous) revised text from the caret, wrapping round.
fn step(s: &mut Session, forward: bool) -> CmdResult {
    let spots = revised_spots(&s.doc);
    let here = (s.sel.focus.path.clone(), s.sel.focus.off);
    let (a, _) = s.sel.ordered();
    let start = (a.path, a.off);
    let pick = if forward {
        spots.iter().find(|x| (x.0.clone(), x.1) > here).or_else(|| spots.first())
    } else {
        spots.iter().rev().find(|x| (x.0.clone(), x.1) < start).or_else(|| spots.last())
    };
    let Some((path, a, b)) = pick.cloned() else { return Err(CmdError::Failed("nothing is marked revised".into())) };
    s.sel = Selection { anchor: pos_at(&path, a), focus: pos_at(&path, b) };
    sel_result(s)
}

fn has_revised(s: &Session) -> Option<&'static str> {
    if revised_spots(&s.doc).is_empty() { Some("nothing is marked revised") } else { None }
}

/// The revision commands.
pub fn commands() -> Vec<CommandSpec> {
    const LOC: &str = "Review › Revisions";
    vec![
        CommandSpec::new("revision.gotoNext", "Next Revised Text", LOC, |s, _| step(s, true)).pure().when(has_revised),
        CommandSpec::new("revision.gotoPrevious", "Previous Revised Text", LOC, |s, _| step(s, false)).pure().when(has_revised),
        CommandSpec::new("revision.settings", "Revision Settings", LOC, settings).pure(),
        CommandSpec::new("revision.mode", "Revision Mode", LOC, mode).params(r#"{"on"?: bool}"#),
        CommandSpec::new("revision.next", "Next Revision", LOC, next),
        CommandSpec::new("revision.setCurrent", "Set Current Revision", LOC, set_current).params(r#"{"name": string}"#),
        CommandSpec::new("revision.configure", "Revision Options", LOC, configure).params(
            r##"{"mark"?: char, "markAt"?: inches, "colorPages"?: bool, "showMarks"?: bool, "activeOnly"?: bool, "cycle"?: [{"name", "color": "#RRGGBB"}] | null, "set"?: {"index": number, "name"?, "color"?: "#RRGGBB", "bold"?, "underline"?, "strike"?, "mark"?: char}}"##,
        ),
        CommandSpec::new("revision.markRevised", "Mark Revised", LOC, mark_revised),
        CommandSpec::new("revision.clearRevised", "Clear Revised", LOC, clear_revised).params(r#"{"all"?: bool}"#),
        CommandSpec::new("revision.rollback", "Roll Back Revisions", LOC, rollback).params(r#"{"to": set name, or "" for the original}"#),
    ]
}

#[cfg(test)]
mod tests;
