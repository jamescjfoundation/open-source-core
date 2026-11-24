// SPDX-License-Identifier: MIT OR Apache-2.0
//! Pagination above the layout, with a hook for continuation lines.
//!
//! The layout engine breaks lines and pages by itself. Some documents need more at a page break:
//! a line at the foot of a page saying the text goes on, a line at the head of the next page
//! repeating what it continues. A [`PaginationPass`] adds them. Because those lines take room,
//! the pages are decided here, above the layout:
//!
//! 1. every body block is measured ([`crate::measure`]): its lines, their heights, its spacing
//!    and keep settings;
//! 2. pages are filled ([`plan`]): space before is dropped at the top of a page; page break
//!    before starts a page; keep with next holds a paragraph with the start of the next; keep
//!    lines together never splits; widow control leaves at least two lines on each side of a
//!    split; the pass says where a paragraph may split and what lines a break adds, and room is
//!    kept for them;
//! 3. an output document is built ([`build`]) with each page's first paragraph starting a page
//!    and every keep rule turned off, so the layout follows the plan, split paragraphs cut into
//!    their parts, the added lines in place, and a map from each output paragraph back to the
//!    source ([`Paginated::to_source`], [`Paginated::to_output`]);
//! 4. the output is laid out.
//!
//! Ported from Wordsmith Script's pagination (`paginate/`), with its screenplay rules taken out
//! and made the pass's business.
//!
//! **Limits.** The plan reads the body only: footnotes, floating objects that push text, and
//! multiple columns are not planned for, and a table is kept whole. When the layout of the
//! output does not land on the planned pages, [`Paginated::matches_plan`] is false (the output
//! is still a valid document, laid out by the engine's own rules).

use std::collections::VecDeque;
use std::sync::Arc;

use jcj_doc::{Block, Document, Paragraph, Path, Pos, StoryRef, para_block};
use jcj_upstream::doc::section::SectionStart;

use crate::measure::{MeasureCache, MeasuredBlock, measure_cached, measure_text, paragraph_height};
use crate::{DocLayout, LayoutCache, print_layout};

/// Most pages a plan makes. Bounds hostile input.
pub const MAX_PAGES: usize = 100_000;

/// Slack when comparing heights, points.
const EPS: f32 = 0.01;

/// A source block, or part of one, as the pages hold it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part {
    /// Index into the measured blocks (and into [`PageBreak::measured`]).
    pub index: usize,
    /// Index of the block in the source body.
    pub block: usize,
    /// The part's bytes in the paragraph's text (a table: `0..0`).
    pub start: usize,
    pub end: usize,
}

/// A page break the plan is about to make, as the pass sees it.
pub struct PageBreak<'a> {
    /// The page that ends (0-based).
    pub page: usize,
    /// The last source part on the page that ends; `None` when it holds none.
    pub before: Option<Part>,
    /// The first source part on the next page; `None` at the end of the document.
    pub after: Option<Part>,
    /// Every measured block, for looking before and after the break.
    pub measured: &'a [MeasuredBlock],
}

impl PageBreak<'_> {
    /// The break falls inside one paragraph: the page ends with its first part and the next
    /// starts with the rest.
    pub fn splits_paragraph(&self) -> bool {
        matches!((self.before, self.after), (Some(a), Some(b)) if a.index == b.index)
    }
}

/// The lines a page break adds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Continuation {
    /// At the foot of the page that ends, after its text.
    pub foot: Vec<Paragraph>,
    /// At the head of the next page, before its text.
    pub head: Vec<Paragraph>,
}

/// What an app decides about its pages. Every method has a default; the empty pass
/// ([`NoContinuations`]) paginates by the paragraphs' own keep and widow settings.
pub trait PaginationPass {
    /// Change the working copy of the document before anything is measured: add the styles the
    /// added lines use, for example. The source document itself is never changed.
    fn prepare(&mut self, doc: &mut Document) {
        let _ = doc;
    }

    /// May this block be split across pages? Default: a paragraph that does not keep its lines
    /// together. A paragraph taller than a page is split anyway.
    fn may_split(&self, doc: &Document, block: &MeasuredBlock) -> bool {
        let _ = doc;
        block.is_paragraph && !block.keep_lines
    }

    /// Byte offsets in a paragraph's text where the rest of a split may start (spaces after it
    /// are skipped). `None`: at the start of any line.
    fn split_points(&self, doc: &Document, block: &MeasuredBlock) -> Option<Vec<usize>> {
        let _ = (doc, block);
        None
    }

    /// The lines to add at a page break. Called while deciding where to break (so it may be
    /// called more than once for a page) and must depend only on its arguments.
    fn continuation(&self, doc: &Document, at: &PageBreak<'_>) -> Continuation {
        let _ = (doc, at);
        Continuation::default()
    }

    /// Change the output document after it is built (a header, page decorations). Text offsets
    /// must stay as they are, and a paragraph changed here must be given a new revision
    /// ([`Paragraph::touch`]) so the layout cache does not reuse its old lines.
    fn finish(&mut self, out: &mut Document, map: &[Option<Source>]) {
        let _ = (out, map);
    }
}

/// A pass that adds nothing: pages decided by the paragraphs' own keep and widow settings.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoContinuations;

impl PaginationPass for NoContinuations {}

/// One thing on a planned page, in order.
#[derive(Clone, Debug, PartialEq)]
pub enum Piece {
    /// A source block or part of one, with the number of its lines placed and whether its space
    /// before applies (not at the top of a page, nor for the rest of a split paragraph).
    Source { part: Part, lines: usize, space_before: bool },
    /// A line the pass added at a page break.
    Added { paragraph: Box<Paragraph>, foot: bool },
}

/// A block, or what is left of one, waiting to be placed.
#[derive(Clone, Debug)]
struct Item {
    idx: usize,
    start: usize,
    end: usize,
    lines: Vec<(usize, usize)>,
    heights: Vec<f32>,
    space_before: bool,
}

struct Planner<'a> {
    doc: &'a Document,
    m: &'a [MeasuredBlock],
    pass: &'a dyn PaginationPass,
    /// Body height of the page for each measured block (its section's page).
    caps: Vec<f32>,
    /// The block ends a section whose next one starts a new page.
    section_ends: Vec<bool>,
    cap: f32,
    pages: Vec<Vec<Piece>>,
    cur: Vec<Piece>,
    /// For each piece on the current page: the item it placed whole and the height it used, so a
    /// paragraph kept with the next can follow it to the next page.
    placed: Vec<Option<(Item, f32)>>,
    used: f32,
}

/// Body height of a section's page, points.
fn body_height(s: &jcj_doc::SectionProps) -> f32 {
    (s.page_h - s.margin_top - s.margin_bottom).max(crate::measure::FALLBACK_LINE)
}

impl Planner<'_> {
    fn room(&self) -> f32 {
        self.cap - self.used
    }

    fn top(&self) -> bool {
        !self.cur.iter().any(|p| matches!(p, Piece::Source { .. }))
    }

    fn sb(&self, item: &Item) -> f32 {
        if item.space_before && !self.top() { self.m.get(item.idx).map_or(0.0, |p| p.space_before) } else { 0.0 }
    }

    fn min_lines(p: &MeasuredBlock) -> usize {
        if p.widow_control { 2 } else { 1 }
    }

    /// The least height block `j` needs to start on this page, with whatever it keeps with.
    fn min_start(&self, j: usize, depth: usize) -> f32 {
        let Some(p) = self.m.get(j) else { return 0.0 };
        let n = Self::min_lines(p);
        let body = if self.pass.may_split(self.doc, p) && p.lines.len() >= 2 * n { p.heights.iter().take(n).sum() } else { p.height() };
        let keep = if p.keep_next && depth < 4 { self.min_start(j + 1, depth + 1) } else { 0.0 };
        p.space_before + body + keep
    }

    fn part(&self, item: &Item) -> Part {
        Part { index: item.idx, block: self.m.get(item.idx).map_or(0, |p| p.block), start: item.start, end: item.end }
    }

    /// Place the first `n` lines of `item`, ending at byte `end`.
    fn place(&mut self, item: &Item, n: usize, end: usize) {
        let n = n.min(item.lines.len());
        let whole = n >= item.lines.len();
        let h: f32 = item.heights.iter().take(n).sum();
        let sb = self.sb(item);
        let sa = if whole { self.m.get(item.idx).map_or(0.0, |p| p.space_after) } else { 0.0 };
        let part = Part { end: end.clamp(item.start, item.end), ..self.part(item) };
        self.cur.push(Piece::Source { part, lines: n, space_before: sb > 0.0 });
        self.placed.push(whole.then(|| (item.clone(), sb + h + sa)));
        self.used += sb + h + sa;
    }

    fn last_part(&self) -> Option<Part> {
        self.cur.iter().rev().find_map(|p| match p {
            Piece::Source { part, .. } => Some(*part),
            Piece::Added { .. } => None,
        })
    }

    fn continuation(&self, before: Option<Part>, after: Option<Part>) -> Continuation {
        self.pass.continuation(self.doc, &PageBreak { page: self.pages.len(), before, after, measured: self.m })
    }

    fn height_of(&self, paras: &[Paragraph]) -> f32 {
        paras.iter().map(|p| paragraph_height(self.doc, p)).sum()
    }

    /// End the page before `next`, adding the pass's lines on both sides. A foot that does not
    /// fit sends the page's last whole blocks to the next page with `next` (they go back on
    /// `queue`), as long as the page keeps one.
    fn end_page(&mut self, mut next: Option<Part>, queue: &mut VecDeque<Item>) {
        let mut c = self.continuation(self.last_part(), next);
        for _ in 0..4 {
            let foot = self.height_of(&c.foot);
            let sources = self.cur.iter().filter(|p| matches!(p, Piece::Source { .. })).count();
            if c.foot.is_empty() || self.used + foot <= self.cap + EPS || sources <= 1 {
                break;
            }
            let (Some(Some((item, h))), Some(Piece::Source { .. })) = (self.placed.last(), self.cur.last()) else { break };
            let (item, h) = (item.clone(), *h);
            self.cur.pop();
            self.placed.pop();
            self.used -= h;
            next = Some(self.part(&item));
            queue.push_front(item);
            c = self.continuation(self.last_part(), next);
        }
        for paragraph in c.foot {
            self.cur.push(Piece::Added { paragraph: Box::new(paragraph), foot: true });
        }
        self.pages.push(std::mem::take(&mut self.cur));
        self.placed.clear();
        self.used = 0.0;
        if let Some(cap) = next.and_then(|n| self.caps.get(n.index)) {
            self.cap = *cap;
        }
        let head = self.height_of(&c.head);
        for paragraph in c.head {
            self.cur.push(Piece::Added { paragraph: Box::new(paragraph), foot: false });
            self.placed.push(None);
        }
        self.used += head;
    }

    /// Where `item` can split so its first part fits in `avail`: the lines kept here, the byte
    /// the first part ends at and the byte the rest starts at.
    fn split_point(&self, item: &Item, avail: f32, forced: bool) -> Option<(usize, usize, usize)> {
        let p = self.m.get(item.idx)?;
        let mut fit = 0;
        let mut h = 0.0;
        for lh in &item.heights {
            if h + lh > avail + EPS {
                break;
            }
            h += lh;
            fit += 1;
        }
        if fit == 0 {
            return None;
        }
        let total = item.lines.len();
        let n = Self::min_lines(p);
        let line_of = |b: usize| item.lines.iter().rposition(|(s, _)| *s <= b).unwrap_or(0);
        let kept_before = |b: usize| {
            let l = line_of(b);
            if item.lines.get(l).is_some_and(|(s, _)| *s == b) { l } else { l + 1 }
        };
        let points: Vec<usize> = match self.pass.split_points(self.doc, p) {
            Some(v) => v,
            None => item.lines.iter().skip(1).map(|(s, _)| *s).collect(),
        };
        let best = points
            .into_iter()
            .filter(|&b| b > item.start && b < item.end)
            .filter(|&b| {
                let kept = kept_before(b);
                kept >= 1 && kept <= fit && kept >= n.min(total) && total.saturating_sub(line_of(b)) >= n
            })
            .max();
        let b = match best {
            Some(b) => b,
            None if forced && total > 1 => item.lines.get(fit.min(total - 1).max(1)).map_or(item.end, |l| l.0),
            None => return None,
        };
        let kept = kept_before(b);
        let text = &p.text;
        let end = text.get(..b).map_or(b, |s| s.trim_end().len()).max(item.start);
        let rest = text.get(b..item.end).map_or(b, |r| b + (r.len() - r.trim_start().len()));
        Some((kept, end, rest))
    }

    /// The rest of `item` from byte `rest`, measured as its own paragraph.
    fn rest_of(&self, item: &Item, rest: usize) -> Item {
        let src = self.m.get(item.idx).and_then(|p| self.doc.body.get(p.block)).and_then(|b| b.as_para());
        let text = self.m.get(item.idx).and_then(|p| p.text.get(rest..item.end)).unwrap_or("");
        let (lines, heights) = match src {
            Some(sp) => measure_text(self.doc, sp, text),
            None => (vec![(0, text.len())], vec![crate::measure::FALLBACK_LINE]),
        };
        let lines = lines.into_iter().map(|(s, e)| (s + rest, e + rest)).collect();
        Item { idx: item.idx, start: rest, end: item.end, lines, heights, space_before: false }
    }

    /// Try to split `item` here. Returns the rest when it split.
    fn try_split(&mut self, item: &Item, sb: f32, forced: bool) -> Option<Item> {
        // Room for what a break inside this paragraph adds at the foot.
        let part = self.part(item);
        let foot = self.height_of(&self.continuation(Some(part), Some(part)).foot);
        let (kept, end, rest) = self.split_point(item, self.room() - sb - foot, forced)?;
        self.place(item, kept, end);
        Some(self.rest_of(item, rest))
    }

    /// Paragraphs kept with `item` leave the page with it, as long as the page keeps one.
    fn take_back_kept(&mut self, item: Item) -> Vec<Item> {
        let mut moved = vec![item];
        while let (Some(Some((prev, h))), Some(Piece::Source { .. })) = (self.placed.last(), self.cur.last()) {
            let keeps = self.m.get(prev.idx).is_some_and(|p| p.keep_next) && moved.first().is_some_and(|f| prev.idx + 1 == f.idx);
            let here = self.cur.iter().filter(|p| matches!(p, Piece::Source { .. })).count();
            if !keeps || here <= 1 {
                break;
            }
            let (prev, h) = (prev.clone(), *h);
            self.cur.pop();
            self.placed.pop();
            self.used -= h;
            moved.insert(0, prev);
        }
        moved
    }

    fn step(&mut self, item: Item, queue: &mut VecDeque<Item>) {
        let Some(p) = self.m.get(item.idx) else { return };
        let starts_page =
            item.start == 0 && (p.page_break_before || item.idx.checked_sub(1).is_some_and(|i| self.section_ends.get(i) == Some(&true)));
        if starts_page && !self.top() {
            let next = self.part(&item);
            // Pushed first: blocks the break sends back from this page go before it.
            queue.push_front(item);
            self.end_page(Some(next), queue);
            return;
        }
        let sb = self.sb(&item);
        let whole: f32 = item.heights.iter().sum();
        let keep = if p.keep_next { self.min_start(item.idx + 1, 0) } else { 0.0 };
        if sb + whole + keep <= self.room() + EPS || (self.top() && sb + whole <= self.room() + EPS) {
            self.place(&item, item.lines.len(), item.end);
            return;
        }
        let top = self.top();
        let can_split = self.pass.may_split(self.doc, p) && !p.keep_next;
        let forced = top && whole > self.room() && p.is_paragraph;
        if (can_split || forced)
            && let Some(rest) = self.try_split(&item, sb, forced)
        {
            let next = self.part(&rest);
            queue.push_front(rest);
            self.end_page(Some(next), queue);
            return;
        }
        if top {
            // Nothing fits and nothing can split: place it anyway rather than loop.
            self.place(&item, item.lines.len(), item.end);
            return;
        }
        let moved = self.take_back_kept(item);
        let first = moved.first().map(|i| self.part(i));
        for it in moved.into_iter().rev() {
            queue.push_front(it);
        }
        self.end_page(first, queue);
    }

    fn run(mut self) -> Vec<Vec<Piece>> {
        let mut queue: VecDeque<Item> = self
            .m
            .iter()
            .enumerate()
            .map(|(idx, p)| Item { idx, start: 0, end: p.text.len(), lines: p.lines.clone(), heights: p.heights.clone(), space_before: true })
            .collect();
        let mut guard = 0usize;
        while let Some(item) = queue.pop_front() {
            guard += 1;
            if self.pages.len() >= MAX_PAGES || guard > 10 * MAX_PAGES {
                break;
            }
            self.step(item, &mut queue);
        }
        if !self.cur.is_empty() || self.pages.is_empty() {
            // The end of the document: the pass may still add a foot line to the last page.
            let c = self.continuation(self.last_part(), None);
            for paragraph in c.foot {
                self.cur.push(Piece::Added { paragraph: Box::new(paragraph), foot: true });
            }
            self.pages.push(self.cur);
        }
        self.pages
    }
}

/// Plan the pages of `doc` from its measured blocks (see the module documentation).
pub fn plan(doc: &Document, measured: &[MeasuredBlock], pass: &dyn PaginationPass) -> Vec<Vec<Piece>> {
    let sections = doc.sections();
    let section_of = |block: usize| sections.iter().find(|(end, _)| *end >= block).map(|(_, s)| *s).unwrap_or(&doc.last_section);
    let caps: Vec<f32> = measured.iter().map(|m| body_height(section_of(m.block))).collect();
    // A block that ends a section, when the next section starts on a new page.
    let section_ends: Vec<bool> = measured
        .iter()
        .map(|m| {
            let Some(i) = sections.iter().position(|(end, _)| *end == m.block) else { return false };
            let next = sections.get(i + 1).map(|(_, s)| s.start);
            doc.body.get(m.block).and_then(|b| b.as_para()).is_some_and(|p| p.section.is_some())
                && next.is_some_and(|s| s != SectionStart::Continuous)
        })
        .collect();
    let cap = caps.first().copied().unwrap_or_else(|| body_height(&doc.last_section));
    Planner { doc, m: measured, pass, caps, section_ends, cap, pages: Vec::new(), cur: Vec::new(), placed: Vec::new(), used: 0.0 }.run()
}

/// Where an output paragraph came from: body block `block` of the source, from byte `start`.
/// Lines the pass added have no source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source {
    pub block: usize,
    pub start: usize,
}

/// A revision that is the same every time the same source part is shown the same way, so an
/// unchanged page lays out from the cache. The high bit keeps these apart from the counter
/// `Paragraph::touch` uses.
fn stable_rev(parts: &[u64]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    parts.hash(&mut h);
    h.finish() | (1 << 63)
}

fn content_hash(p: &Paragraph) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(p).unwrap_or_default().hash(&mut h);
    h.finish()
}

/// The output paragraph for bytes `start..end` of `src`. Only the last part keeps a section
/// break the paragraph carries.
fn part_of(src: &Paragraph, start: usize, end: usize) -> Paragraph {
    let mut p = src.clone();
    let len = p.len();
    let end = end.min(len);
    if end < len {
        let _ = p.delete(end, len);
        p.section = None;
    }
    if start > 0 {
        let _ = p.delete(0, start.min(end));
    }
    p
}

/// The output document for planned `pages` of `doc` (already prepared by the pass), and each
/// output body block's source. A table is copied whole; one that should start a page relies on
/// the layout breaking there.
pub fn build(doc: &Document, pages: &[Vec<Piece>]) -> (Document, Vec<Option<Source>>) {
    let mut out = doc.clone();
    let mut body = Vec::new();
    let mut map = Vec::new();
    for (n, page) in pages.iter().enumerate() {
        let first = body.len();
        for piece in page {
            let (mut para, src, mut identity) = match piece {
                Piece::Source { part, space_before, .. } => match doc.body.get(part.block).map(|b| &**b) {
                    Some(Block::Para(sp)) => {
                        let mut p = part_of(sp, part.start, part.end);
                        if !space_before {
                            p.props.space_before = Some(0.0);
                        }
                        let id = vec![1, sp.rev, part.start as u64, part.end as u64, u64::from(*space_before)];
                        (p, Some(Source { block: part.block, start: part.start }), id)
                    }
                    Some(Block::Table(_)) => {
                        if let Some(b) = doc.body.get(part.block) {
                            body.push(b.clone());
                            map.push(Some(Source { block: part.block, start: 0 }));
                        }
                        continue;
                    }
                    None => continue,
                },
                Piece::Added { paragraph, foot } => ((**paragraph).clone(), None, vec![2, content_hash(paragraph), u64::from(*foot)]),
            };
            // The plan decided the breaks: the layout must not move anything.
            para.props.keep_next = Some(false);
            para.props.keep_lines = Some(false);
            para.props.widow_control = Some(false);
            para.props.page_break_before = Some(body.len() == first && n > 0);
            if body.len() == first {
                para.props.space_before = Some(0.0);
            }
            identity.extend([u64::from(body.len() == first), u64::from(n > 0)]);
            para.rev = stable_rev(&identity);
            body.push(para_block(para));
            map.push(src);
        }
    }
    if body.is_empty() {
        body.push(para_block(Paragraph::new()));
        map.push(None);
    }
    out.body = body;
    (out, map)
}

/// A paginated document.
pub struct Paginated {
    /// The output document: what the page view shows, PDF export writes and printing prints.
    pub doc: Document,
    /// The plan: what each page holds.
    pub pages: Vec<Vec<Piece>>,
    /// Each output body block's source, by output block index.
    pub map: Vec<Option<Source>>,
    /// The output's print layout.
    pub layout: Arc<DocLayout>,
    /// The output was laid out on exactly the planned pages.
    pub matches_plan: bool,
}

impl Paginated {
    /// The output position showing source position `pos` (body story), if it is shown.
    pub fn to_output(&self, pos: &Pos) -> Option<Pos> {
        if pos.story != StoryRef::Body || pos.path.0.len() != 1 {
            return None;
        }
        let block = pos.path.last();
        let (out, src) = self
            .map
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.map(|s| (i, s)))
            .filter(|(_, s)| s.block == block && s.start <= pos.off)
            .max_by_key(|(_, s)| s.start)?;
        let len = self.doc.para(StoryRef::Body, &Path::top(out)).map_or(0, Paragraph::len);
        Some(Pos { story: StoryRef::Body, path: Path::top(out), off: (pos.off - src.start).min(len) })
    }

    /// The source position an output position shows (a click on the page view). A line the pass
    /// added maps to the start of the next paragraph that has a source.
    pub fn to_source(&self, pos: &Pos) -> Option<Pos> {
        if pos.story != StoryRef::Body || pos.path.0.is_empty() {
            return None;
        }
        let i = pos.path.0.first().map(|&b| b as usize)?;
        let mut j = i;
        loop {
            match self.map.get(j)? {
                Some(src) => {
                    let off = if j == i { pos.off } else { 0 };
                    return Some(Pos { story: StoryRef::Body, path: Path::top(src.block), off: src.start + off });
                }
                None => j += 1,
            }
        }
    }

    /// The output body block index each page starts with.
    pub fn page_starts(&self) -> Vec<usize> {
        let mut at = 0;
        self.pages
            .iter()
            .map(|p| {
                let start = at;
                at += p.len();
                start
            })
            .collect()
    }
}

/// What pagination keeps between calls: the measured lines of the source, and the output's
/// layout cache.
#[derive(Default)]
pub struct Paginator {
    pub measure: MeasureCache,
    pub layout: LayoutCache,
}

impl Paginator {
    /// Empty caches.
    pub fn new() -> Self {
        Self::default()
    }

    /// Paginate `doc` with `pass`: prepare, measure, plan, build the output and lay it out.
    pub fn paginate(&mut self, doc: &Document, pass: &mut dyn PaginationPass) -> Paginated {
        let mut src = doc.clone();
        pass.prepare(&mut src);
        let measured = measure_cached(&src, &mut self.measure);
        let pages = plan(&src, &measured, pass);
        let (mut out, map) = build(&src, &pages);
        pass.finish(&mut out, &map);
        let layout = Arc::new(print_layout(&out, &mut self.layout));
        let matches_plan = lands_on_plan(&layout, &pages);
        if !matches_plan {
            log::warn!("pagination: planned {} pages, the layout made {}", pages.len(), layout.pages.len());
        }
        Paginated { doc: out, pages, map, layout, matches_plan }
    }
}

/// Did each planned page's first block land at the top of that page?
fn lands_on_plan(layout: &DocLayout, pages: &[Vec<Piece>]) -> bool {
    if layout.pages.len() != pages.len() {
        return false;
    }
    let mut at = 0;
    for (n, page) in pages.iter().enumerate() {
        let first = layout.index.get(&(StoryRef::Body, Path::top(at))).and_then(|v| v.first()).map(|(pg, _)| *pg);
        // A page starting with a table has no paragraph at this path: nothing to check.
        if first.is_some_and(|pg| pg != n) {
            return false;
        }
        at += page.len();
    }
    true
}

/// Paginate `doc` once with `pass`, with fresh caches.
pub fn paginate(doc: &Document, pass: &mut dyn PaginationPass) -> Paginated {
    Paginator::new().paginate(doc, pass)
}
