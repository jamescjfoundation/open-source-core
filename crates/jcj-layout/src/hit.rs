// SPDX-License-Identifier: MIT OR Apache-2.0
//! Hit testing: from a point on a page to a document position, and from a position to where the
//! caret is drawn. Page coordinates are points from the page's top left, y down.

use jcj_doc::{Document, Pos, StoryRef};

use crate::{DocLayout, Rect};

/// The position nearest to (`x`, `y`) on page `page` in the body text, or `None` when the page
/// does not exist or holds no body text.
pub fn hit(layout: &DocLayout, page: usize, x: f32, y: f32) -> Option<Pos> {
    hit_in(layout, page, x, y, story_at(layout, page, x, y).unwrap_or(StoryRef::Body))
}

/// The position nearest to (`x`, `y`) on page `page` in `story` (the body, a note, a header or
/// footer part, a text box).
pub fn hit_in(layout: &DocLayout, page: usize, x: f32, y: f32, story: StoryRef) -> Option<Pos> {
    layout.hit(page, x, y, story)
}

/// The story whose text is under a point: the body, a footnote or endnote, a text box. Headers
/// and footers are not reported (they are edited on purpose, by a double click).
pub fn story_at(layout: &DocLayout, page: usize, x: f32, y: f32) -> Option<StoryRef> {
    layout.story_at(page, x, y)
}

/// Where the caret for a position is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaretRect {
    /// Page index.
    pub page: usize,
    /// The caret bar on that page. Upright text gives a bar of width 0 as tall as the text;
    /// turned text (a table cell's text direction) a bar of height 0 lying across the page.
    pub rect: Rect,
}

/// The caret for `pos`, or `None` when the position is not laid out (hidden text, a story not
/// shown on any page).
pub fn caret_rect(layout: &DocLayout, pos: &Pos) -> Option<CaretRect> {
    caret_rect_near(layout, pos, 0)
}

/// [`caret_rect`] for a story shown on many pages (a header): the copy on or after page `hint`.
pub fn caret_rect_near(layout: &DocLayout, pos: &Pos, hint: usize) -> Option<CaretRect> {
    layout.caret_on(pos, hint).map(|c| CaretRect { page: c.page, rect: Rect::new(c.x, c.top, c.width, c.height) })
}

/// The first page showing `pos`.
pub fn page_of(layout: &DocLayout, pos: &Pos) -> Option<usize> {
    layout.page_of(pos)
}

/// The highlight rectangles of the selection between `a` and `b` (either order), as
/// (page index, rectangle) pairs.
pub fn selection_rects(layout: &DocLayout, doc: &Document, a: &Pos, b: &Pos) -> Vec<(usize, Rect)> {
    layout.selection_rects(doc, a, b, 0)
}
