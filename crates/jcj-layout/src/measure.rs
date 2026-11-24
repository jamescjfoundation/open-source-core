// SPDX-License-Identifier: MIT OR Apache-2.0
//! Measuring body blocks: each paragraph's lines (byte ranges and heights) and the resolved
//! spacing and keep settings the layout used, and each table's height.
//!
//! This is what a pass working above the layout reads to decide pages itself. Ported from the
//! first foundation app's pagination, without its app-specific rules.

use std::collections::HashMap;
use std::sync::Arc;

use jcj_doc::{Block, Document, Paragraph, Path, StoryRef, para_block};

use crate::{DocLayout, FieldCtx, LayoutCache, ParaLayout, Placed, print_layout};

/// Height used for a paragraph the layout gave no lines (an empty layout), points.
pub const FALLBACK_LINE: f32 = 12.0;

/// One top-level body block, measured.
#[derive(Clone, Debug, PartialEq)]
pub struct MeasuredBlock {
    /// Index of the block in the body.
    pub block: usize,
    /// `false` for a table: it has one "line", its whole height, and is never split.
    pub is_paragraph: bool,
    /// The paragraph's text (empty for a table).
    pub text: String,
    /// Each line's byte range in `text` (a table: one empty range).
    pub lines: Vec<(usize, usize)>,
    /// Each line's height, points.
    pub heights: Vec<f32>,
    /// Space before and after the paragraph, points (resolved from its style).
    pub space_before: f32,
    pub space_after: f32,
    /// Keep with next: the paragraph stays on the page with the start of the next.
    pub keep_next: bool,
    /// Keep lines together: the paragraph is never split.
    pub keep_lines: bool,
    /// Widow and orphan control: a split leaves at least two lines on each side.
    pub widow_control: bool,
    /// Page break before: the paragraph starts a page.
    pub page_break_before: bool,
}

impl MeasuredBlock {
    /// The height of its lines, without space before or after.
    pub fn height(&self) -> f32 {
        self.heights.iter().sum()
    }
}

/// Byte ranges of a laid-out paragraph's lines.
pub fn line_ranges(p: &ParaLayout) -> Vec<(usize, usize)> {
    p.lines.iter().map(|l| (l.start, l.stop.max(l.start))).collect()
}

/// Heights of a laid-out paragraph's lines, points.
pub fn line_heights(p: &ParaLayout) -> Vec<f32> {
    p.lines.iter().map(|l| l.height).collect()
}

/// The layout of top-level body paragraph `block`, wherever its first piece was placed.
pub fn body_paragraph_layout(layout: &DocLayout, block: usize) -> Option<Arc<ParaLayout>> {
    let pieces = layout.index.get(&(StoryRef::Body, Path::top(block)))?;
    let (page, item) = pieces.first()?;
    match layout.pages.get(*page)?.items.get(*item)? {
        Placed::Lines { para, .. } => Some(para.clone()),
        _ => None,
    }
}

/// The height of top-level body table `block` in `layout`: the extent of its cells on each page
/// it is on, added up. `None` when it is not laid out.
pub fn body_table_height(layout: &DocLayout, block: usize) -> Option<f32> {
    let Ok(b) = u32::try_from(block) else { return None };
    let mut total = 0.0;
    let mut found = false;
    for page in &layout.pages {
        let (mut top, mut bottom) = (f32::MAX, f32::MIN);
        for it in &page.items {
            if let Placed::Cell { rect, table, story: StoryRef::Body, .. } = it
                && table.0.first() == Some(&b)
            {
                top = top.min(rect.y);
                bottom = bottom.max(rect.y + rect.h);
            }
        }
        if bottom > top {
            total += bottom - top;
            found = true;
        }
    }
    found.then_some(total)
}

fn measured_para(doc: &Document, block: usize, p: &Paragraph, lines: Vec<(usize, usize)>, heights: Vec<f32>) -> MeasuredBlock {
    let rp = doc.styles.resolve_para(&p.props);
    let (lines, heights) = if lines.is_empty() { (vec![(0, p.text.len())], vec![FALLBACK_LINE]) } else { (lines, heights) };
    MeasuredBlock {
        block,
        is_paragraph: true,
        text: p.text.clone(),
        lines,
        heights,
        space_before: rp.space_before.max(0.0),
        space_after: rp.space_after.max(0.0),
        keep_next: rp.keep_next,
        keep_lines: rp.keep_lines,
        widow_control: rp.widow_control,
        page_break_before: rp.page_break_before,
    }
}

fn measured_table(block: usize, height: f32) -> MeasuredBlock {
    MeasuredBlock {
        block,
        is_paragraph: false,
        text: String::new(),
        lines: vec![(0, 0)],
        heights: vec![height.max(0.0)],
        space_before: 0.0,
        space_after: 0.0,
        keep_next: false,
        keep_lines: true,
        widow_control: false,
        page_break_before: false,
    }
}

/// Measure every top-level body block of `doc` from `layout` (a print layout of `doc`).
pub fn measure(doc: &Document, layout: &DocLayout) -> Vec<MeasuredBlock> {
    let Some(blocks) = doc.story(StoryRef::Body) else { return Vec::new() };
    let mut out = Vec::with_capacity(blocks.len());
    for (i, block) in blocks.iter().enumerate() {
        match &**block {
            Block::Para(p) => {
                let (lines, heights) = body_paragraph_layout(layout, i).map_or((Vec::new(), Vec::new()), |pl| (line_ranges(&pl), line_heights(&pl)));
                out.push(measured_para(doc, i, p, lines, heights));
            }
            Block::Table(_) => out.push(measured_table(i, body_table_height(layout, i).unwrap_or(0.0))),
        }
    }
    out
}

/// Paragraph lines kept between measurements, so an edit lays out again only the paragraphs it
/// changed. Keyed by each paragraph's revision and a fingerprint of the styles and column width.
#[derive(Default)]
pub struct MeasureCache {
    lines: HashMap<(u64, u64), (Vec<(usize, usize)>, Vec<f32>)>,
    /// The styles the entries were made with, and their fingerprint.
    env: Option<(Arc<jcj_doc::StyleSheet>, u64)>,
}

/// Most paragraphs kept; past this the cache starts over. Bounds memory on huge documents.
const MAX_CACHED: usize = 200_000;

impl MeasureCache {
    /// An empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of paragraphs measured and kept.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether nothing is kept.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    fn env(&mut self, doc: &Document) -> u64 {
        use std::hash::{Hash, Hasher};
        if let Some((styles, key)) = &self.env
            && Arc::ptr_eq(styles, &doc.styles)
        {
            return *key ^ text_width(doc).to_bits() as u64;
        }
        let mut h = std::collections::hash_map::DefaultHasher::new();
        serde_json::to_string(&*doc.styles).unwrap_or_default().hash(&mut h);
        let key = h.finish();
        self.env = Some((doc.styles.clone(), key));
        key ^ text_width(doc).to_bits() as u64
    }
}

/// The body text column's width in `doc`'s last section, points.
pub fn text_width(doc: &Document) -> f32 {
    let s = &doc.last_section;
    (s.page_w - s.margin_left - s.margin_right).max(1.0)
}

/// Lay out paragraph `p` of `doc` on its own in a column `width` points wide (no page, no
/// floats around it): the result a measurement reads.
pub fn layout_alone(doc: &Document, p: &Paragraph, width: f32) -> ParaLayout {
    let fields = FieldCtx::default();
    let env = crate::para::ParaEnv {
        doc,
        width,
        label: None,
        fields: &fields,
        show_hidden: false,
        field_codes: false,
        hide_deleted: Default::default(),
        table: None,
        proofing: None,
        typing_word: None,
        numeral: Default::default(),
        exclusions: &[],
        eq_number: 0,
        read_line_spacing: None,
        grid: Default::default(),
        fit_objects: false,
        fitted: &[],
    };
    crate::para::layout_para(p, &env)
}

/// [`measure`] without laying out the whole document: each paragraph is laid out on its own (its
/// lines do not depend on where it lands) through `cache`; tables are laid out on their own each
/// time. Single-column documents with one page size only; list labels are not measured.
pub fn measure_cached(doc: &Document, cache: &mut MeasureCache) -> Vec<MeasuredBlock> {
    let env_key = cache.env(doc);
    if cache.lines.len() > MAX_CACHED {
        cache.lines.clear();
    }
    let width = text_width(doc);
    let Some(blocks) = doc.story(StoryRef::Body) else { return Vec::new() };
    let mut out = Vec::with_capacity(blocks.len());
    for (i, block) in blocks.iter().enumerate() {
        match &**block {
            Block::Para(p) => {
                let (lines, heights) = cache
                    .lines
                    .entry((p.rev, env_key))
                    .or_insert_with(|| {
                        let pl = layout_alone(doc, p, width);
                        (line_ranges(&pl), line_heights(&pl))
                    })
                    .clone();
                out.push(measured_para(doc, i, p, lines, heights));
            }
            Block::Table(_) => {
                let mut probe = probe_doc(doc);
                probe.body = vec![block.clone()];
                let l = print_layout(&probe, &mut LayoutCache::new());
                out.push(measured_table(i, body_table_height(&l, 0).unwrap_or(0.0)));
            }
        }
    }
    out
}

/// An empty document with `doc`'s styles, numbering and last section: a place to lay out a
/// part of `doc` on its own.
fn probe_doc(doc: &Document) -> Document {
    let mut probe = Document::new();
    probe.styles = doc.styles.clone();
    probe.numbering = doc.numbering.clone();
    probe.last_section = doc.last_section.clone();
    probe
}

/// The lines `text` would take as paragraph `src` (its style and its formatting at the start),
/// for the rest of a split paragraph: byte ranges in `text` and heights.
pub fn measure_text(doc: &Document, src: &Paragraph, text: &str) -> (Vec<(usize, usize)>, Vec<f32>) {
    let mut p = src.clone();
    let len = p.len();
    let props = src.props_at(0).clone();
    if p.delete(0, len).and_then(|()| p.insert_text(0, text, &props).map(|_| ())).is_err() {
        return (vec![(0, text.len())], vec![FALLBACK_LINE]);
    }
    p.props.page_break_before = Some(false);
    p.section = None;
    let mut probe = probe_doc(doc);
    probe.body = vec![para_block(p)];
    let l = print_layout(&probe, &mut LayoutCache::new());
    match body_paragraph_layout(&l, 0) {
        Some(pl) if !pl.lines.is_empty() => (line_ranges(&pl), line_heights(&pl)),
        _ => (vec![(0, text.len())], vec![FALLBACK_LINE]),
    }
}

/// The height paragraph `p` takes in `doc`'s body column, space before and after included.
pub fn paragraph_height(doc: &Document, p: &Paragraph) -> f32 {
    let pl = layout_alone(doc, p, text_width(doc));
    let rp = doc.styles.resolve_para(&p.props);
    let lines: f32 = if pl.lines.is_empty() { FALLBACK_LINE } else { pl.lines.iter().map(|l| l.height).sum() };
    rp.space_before.max(0.0) + lines + rp.space_after.max(0.0)
}
