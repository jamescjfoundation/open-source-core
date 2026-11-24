// SPDX-License-Identifier: MIT OR Apache-2.0
//! Where pages start and end: which top-level body blocks (and which of a paragraph's lines)
//! each page of a layout shows.

use std::ops::Range;

use jcj_doc::StoryRef;

use crate::{DocLayout, Placed};

/// A top-level body block, or the part of one, shown on a page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PagePiece {
    /// Index of the block in the body.
    pub block: usize,
    /// A paragraph's lines on this page (line indices) and their bytes in its text; `None` for a
    /// table.
    pub lines: Option<Range<usize>>,
    pub bytes: Option<Range<usize>>,
    /// The block started on an earlier page.
    pub continued: bool,
    /// The block goes on to a later page.
    pub continues: bool,
}

/// Each page's body pieces, in order: what the page starts with and ends with.
pub fn body_pieces(layout: &DocLayout) -> Vec<Vec<PagePiece>> {
    let mut pages: Vec<Vec<PagePiece>> = Vec::with_capacity(layout.pages.len());
    for page in &layout.pages {
        let mut out: Vec<PagePiece> = Vec::new();
        for it in &page.items {
            let Placed::Lines { story: StoryRef::Body, path, para, l0, l1, .. } = it else { continue };
            let Some(&first) = path.0.first() else { continue };
            let block = first as usize;
            if path.0.len() == 1 {
                let start = para.lines.get(*l0).map_or(0, |l| l.start);
                let end = l1.checked_sub(1).and_then(|i| para.lines.get(i)).map_or(start, |l| l.stop.max(start));
                out.push(PagePiece { block, lines: Some(*l0..*l1), bytes: Some(start..end), continued: false, continues: false });
            } else if out.last().is_none_or(|p| p.block != block || p.lines.is_some()) {
                // A table's paragraphs: one piece for the table.
                out.push(PagePiece { block, lines: None, bytes: None, continued: false, continues: false });
            }
        }
        pages.push(out);
    }
    // A block on two pages: the earlier piece continues, the later is continued.
    for i in 1..pages.len() {
        let (before, after) = pages.split_at_mut(i);
        if let (Some(prev), Some(next)) = (before.last_mut().and_then(|p| p.last_mut()), after.first_mut().and_then(|p| p.first_mut()))
            && prev.block == next.block
        {
            prev.continues = true;
            next.continued = true;
        }
    }
    pages
}

/// The first and last body piece of each page (`None` for a page without body text).
pub fn page_ends(layout: &DocLayout) -> Vec<Option<(PagePiece, PagePiece)>> {
    body_pieces(layout).into_iter().map(|p| Some((p.first()?.clone(), p.last()?.clone()))).collect()
}
