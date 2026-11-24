// SPDX-License-Identifier: MIT OR Apache-2.0
//! Laying a document out into pages.
//!
//! Apps name layout types through this crate, never through WordCraft (decision 0005). The
//! layout engine is WordCraft's `wordcraft-layout` at the pinned release; this crate re-exports
//! the types apps hold on to ([`DocLayout`], [`Page`], [`Placed`], [`ParaLayout`]) and adds:
//!
//! - entry points that say what they are for ([`layout`], [`print_layout`], [`page_count`],
//!   [`page_boxes`]);
//! - [`hit`]: point → position and position → caret rectangle;
//! - [`fonts`]: the bundled font set and font registration;
//! - [`measure`] and [`paginate`]: the primitives a pass that works above the layout needs (each
//!   paragraph's lines and heights, where pages start and end, an output document with explicit
//!   page breaks and a map back to the source), and [`paginate::PaginationPass`], the hook an
//!   app implements to add continuation lines at page ends and starts.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod fonts;
pub mod hit;
pub mod measure;
pub mod pages;
pub mod paginate;

use jcj_doc::Document;
use jcj_upstream::layout as up;

// The layout types apps hold. Each name is part of the core's interface.
pub use up::{DocLayout, FieldCtx, LayoutCache, LayoutOptions, Page, ParaLayout, Placed, ViewMode};

/// Geometry: page coordinates are points (1/72 inch), y down, origin at the page's top left.
pub use jcj_upstream::geom::{Point, Rect};

/// Laying out one paragraph on its own (without placing it on a page).
pub mod para {
    pub use jcj_upstream::layout::para::{Grid, Line, ParaEnv, layout_para};
}

/// Lay out the whole document with `opts`, reusing paragraph layouts kept in `cache`.
///
/// The cache may be kept between calls on edited versions of the same document: paragraphs
/// whose revision ([`jcj_doc::Paragraph::rev`]) is unchanged are not laid out again.
pub fn layout(doc: &Document, cache: &mut LayoutCache, opts: &LayoutOptions) -> DocLayout {
    up::layout(doc, cache, opts)
}

/// Print layout: pages as PDF export and printing make them (tracked deletions shown).
pub fn print_layout(doc: &Document, cache: &mut LayoutCache) -> DocLayout {
    layout(doc, cache, &print_options())
}

/// The options of a print layout: [`ViewMode::Print`], everything else default.
pub fn print_options() -> LayoutOptions {
    LayoutOptions { view: ViewMode::Print, ..Default::default() }
}

/// The number of pages in a layout.
pub fn page_count(layout: &DocLayout) -> usize {
    layout.pages.len()
}

/// Where a page is and what it shows, in page coordinates (points).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageBox {
    /// Index in [`DocLayout::pages`] (0-based).
    pub index: usize,
    /// The page number as printed on it (section restarts applied).
    pub number: u32,
    /// The section it belongs to (index into `Document::sections()`).
    pub section: usize,
    /// The whole page: `(0, 0)` to its width and height. Wider than the paper when the layout
    /// shows comment balloons ([`LayoutOptions::comment_balloons`]); see `markup`.
    pub page: Rect,
    /// The body text area (inside the margins).
    pub body: Rect,
    /// Width of the comment balloon area at the page's right; 0 without one.
    pub markup: f32,
}

/// Each page's box: its size, body area and number.
pub fn page_boxes(layout: &DocLayout) -> Vec<PageBox> {
    layout
        .pages
        .iter()
        .enumerate()
        .map(|(index, p)| PageBox {
            index,
            number: p.number,
            section: p.section,
            page: Rect::new(0.0, 0.0, p.w, p.h),
            body: p.body,
            markup: p.markup,
        })
        .collect()
}

#[cfg(test)]
mod tests;
