// SPDX-License-Identifier: MIT OR Apache-2.0
//! The pages the features see: which body blocks each page holds.
//!
//! Some commands need pages: Lock Pages records where each page starts, the bookmark list shows
//! each bookmark's page. They ask [`planned_pages`], which plans the pages the way the app does.
//! By default that is the core's pagination ([`jcj_layout::paginate`]) with the page locks'
//! pass ([`crate::locks::LockPass`]) and no continuation lines. An app with its own
//! `PaginationPass` sets [`set_pages`] once at start, so
//! the commands see the pages its users see; its pass should wrap its own in a `LockPass`.

use std::sync::RwLock;

use jcj_doc::Document;
use jcj_layout::measure::{MeasureCache, measure_cached};
use jcj_layout::paginate::{NoContinuations, PaginationPass, Piece, plan};

use crate::locks::LockPass;

/// A function that plans a document's pages: each page's pieces, in order.
pub type PagesFn = fn(&Document) -> Vec<Vec<Piece>>;

static PAGES: RwLock<Option<PagesFn>> = RwLock::new(None);

/// Plan pages with `f` from now on (the whole process); `None` goes back to [`default_pages`].
pub fn set_pages(f: Option<PagesFn>) {
    if let Ok(mut g) = PAGES.write() {
        *g = f;
    }
}

/// The pages of `doc` as the app plans them ([`set_pages`]), or [`default_pages`].
pub fn planned_pages(doc: &Document) -> Vec<Vec<Piece>> {
    let f = PAGES.read().ok().and_then(|g| *g);
    match f {
        Some(f) => f(doc),
        None => default_pages(doc),
    }
}

/// The pages of `doc` planned with `pass`, without building or laying out the output.
pub fn pages_with(doc: &Document, pass: &mut dyn PaginationPass) -> Vec<Vec<Piece>> {
    let mut src = doc.clone();
    pass.prepare(&mut src);
    let measured = measure_cached(&src, &mut MeasureCache::new());
    plan(&src, &measured, pass)
}

/// The pages of `doc` planned by the core's pagination with page locks and nothing else.
pub fn default_pages(doc: &Document) -> Vec<Vec<Piece>> {
    pages_with(doc, &mut LockPass::new(NoContinuations))
}

/// Where a page starts: the body block and byte of its first source piece.
pub fn page_start(page: &[Piece]) -> Option<(usize, usize)> {
    page.iter().find_map(|p| match p {
        Piece::Source { part, .. } => Some((part.block, part.start)),
        Piece::Added { .. } => None,
    })
}

/// The page (0-based) holding byte `off` of top-level body block `block`.
pub fn page_of(pages: &[Vec<Piece>], block: usize, off: usize) -> Option<usize> {
    pages.iter().position(|pg| {
        pg.iter().any(|pc| match pc {
            Piece::Source { part, .. } => part.block == block && (part.start <= off && (off < part.end || part.start == part.end || off == part.end)),
            Piece::Added { .. } => false,
        })
    })
}
