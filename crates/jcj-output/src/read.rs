// SPDX-License-Identifier: MIT OR Apache-2.0
//! Reading PDFs: the text of each page and where each glyph lands.
//!
//! [`read_pages`] gives every page's glyphs in the order the page draws them, with their
//! position in PDF coordinates (points from the bottom left; y grows upward) and their text.
//! Apps group the glyphs into lines themselves: by [`PdfGlyph::y`] (the baseline) for a PDF's
//! lines, by [`PdfGlyph::x`] for where a line starts.
//!
//! Files are not trusted: the input size, page count and glyphs per page are bounded
//! ([`ReadLimits`]), and a file the reader cannot read gives a [`ReadError`], never a panic.
//!
//! Wraps craft-libs' `craft-pdf` reader (its Hayro snapshot).

use jcj_upstream::pdf_read::interpret::font::Glyph;
use jcj_upstream::pdf_read::interpret::hayro_cmap::BfString;
use jcj_upstream::pdf_read::interpret::{
    BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache, InterpreterSettings, Paint, PathDrawMode, SoftMask,
};
use jcj_upstream::pdf_read::{OpenOptions, interpret_page, open_slice_with_options};
use kurbo::{Affine, BezPath, Rect};

/// How a glyph is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GlyphMode {
    /// Filled: ordinary visible text.
    Fill,
    /// Outlined.
    Stroke,
    /// Text that shapes a clipping area; nothing is painted.
    Clip,
    /// Not drawn (for example the hidden text layer over a scanned page).
    Invisible,
}

/// One glyph on a page.
#[derive(Clone, Debug, PartialEq)]
pub struct PdfGlyph {
    /// Where the glyph's origin lands, from the page's left edge, in PDF units (points).
    pub x: f64,
    /// Where the glyph's origin lands, from the page's bottom edge (y grows upward).
    pub y: f64,
    /// The y of the transform the glyph is drawn with, before the glyph's own placement. PDFs
    /// that move to each line with a transform give one value per line; PDFs that place every
    /// glyph on its own (the core's export does) give one value for the whole page, so group
    /// lines by [`PdfGlyph::y`] when this does not change.
    pub line_y: f64,
    /// The glyph's text, when the PDF says which characters it shows.
    pub text: Option<String>,
    /// How it is drawn.
    pub mode: GlyphMode,
}

/// One page of a PDF.
#[derive(Clone, Debug, PartialEq)]
pub struct PdfPage {
    /// The page's width in PDF units, before any page rotation.
    pub width: f64,
    /// The page's height in PDF units, before any page rotation.
    pub height: f64,
    /// The page's glyphs, in the order the page draws them.
    pub glyphs: Vec<PdfGlyph>,
}

/// Bounds for reading a file that is not trusted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadLimits {
    /// Largest file read, in bytes.
    pub max_input_bytes: usize,
    /// Most pages read.
    pub max_pages: usize,
    /// Most glyphs on one page.
    pub max_glyphs_per_page: usize,
}

impl Default for ReadLimits {
    /// 256 MB, 5,000 pages, 200,000 glyphs a page.
    fn default() -> Self {
        Self { max_input_bytes: 256 * 1024 * 1024, max_pages: 5_000, max_glyphs_per_page: 200_000 }
    }
}

/// Why a PDF could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadError {
    /// The file is larger than [`ReadLimits::max_input_bytes`] (the limit).
    TooLarge(usize),
    /// The file has more pages than [`ReadLimits::max_pages`] (the limit).
    TooManyPages(usize),
    /// A page (1-based) has more glyphs than [`ReadLimits::max_glyphs_per_page`].
    TooManyGlyphs {
        /// The page, counted from 1.
        page: usize,
        /// The limit.
        limit: usize,
    },
    /// The file is not a PDF the reader can open (the reader's reason).
    NotPdf(String),
    /// A page (1-based) could not be read.
    BadPage(usize),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::TooLarge(limit) => write!(f, "the PDF is larger than {limit} bytes"),
            ReadError::TooManyPages(limit) => write!(f, "the PDF has more than {limit} pages"),
            ReadError::TooManyGlyphs { page, limit } => write!(f, "page {page} has more than {limit} characters"),
            ReadError::NotPdf(e) => write!(f, "the file could not be read as a PDF: {e}"),
            ReadError::BadPage(n) => write!(f, "page {n} of the PDF could not be read"),
        }
    }
}

impl std::error::Error for ReadError {}

/// Every page of a PDF, with its glyphs.
///
/// Each page is read on its own, as if drawn at its natural size with no rotation, so positions
/// are the PDF's own coordinates.
pub fn read_pages(bytes: &[u8], limits: &ReadLimits) -> Result<Vec<PdfPage>, ReadError> {
    if bytes.len() > limits.max_input_bytes {
        return Err(ReadError::TooLarge(limits.max_input_bytes));
    }
    let options = OpenOptions { max_input_bytes: limits.max_input_bytes, ..OpenOptions::default() };
    let file = open_slice_with_options(bytes, None, &options).map_err(|e| ReadError::NotPdf(e.to_string()))?;
    let pages = file.pages();
    if pages.len() > limits.max_pages {
        return Err(ReadError::TooManyPages(limits.max_pages));
    }
    let cache = InterpreterCache::new();
    let mut out = Vec::with_capacity(pages.len());
    for (i, page) in pages.iter().enumerate() {
        let mut ctx = Context::new(Affine::IDENTITY, Rect::new(0.0, 0.0, 1.0, 1.0), &cache, file.xref(), InterpreterSettings::default());
        let mut glyphs = Collector { glyphs: Vec::new(), limit: limits.max_glyphs_per_page, over: false };
        interpret_page(page, &mut ctx, &mut glyphs).map_err(|_| ReadError::BadPage(i + 1))?;
        if glyphs.over {
            return Err(ReadError::TooManyGlyphs { page: i + 1, limit: limits.max_glyphs_per_page });
        }
        let (width, height) = page.base_dimensions();
        out.push(PdfPage { width: f64::from(width), height: f64::from(height), glyphs: glyphs.glyphs });
    }
    Ok(out)
}

/// A drawing device that keeps only glyphs.
struct Collector {
    glyphs: Vec<PdfGlyph>,
    limit: usize,
    over: bool,
}

impl Device<'_> for Collector {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'_>>) {}
    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn draw_path(&mut self, _: &BezPath, _: Affine, _: &Paint<'_>, _: &PathDrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'_>>, _: BlendMode) {}
    fn draw_glyph(&mut self, g: &Glyph<'_>, t: Affine, glyph: Affine, _: &Paint<'_>, mode: &GlyphDrawMode) {
        if self.glyphs.len() >= self.limit {
            self.over = true;
            return;
        }
        let mode = match mode {
            GlyphDrawMode::Fill => GlyphMode::Fill,
            GlyphDrawMode::Stroke(_) => GlyphMode::Stroke,
            GlyphDrawMode::Clip => GlyphMode::Clip,
            GlyphDrawMode::Invisible => GlyphMode::Invisible,
        };
        let text = match g.as_unicode() {
            Some(BfString::Char(c)) => Some(c.to_string()),
            Some(BfString::String(s)) => Some(s),
            None => None,
        };
        // Where the glyph lands: its own placement, then the text line's.
        let at = (t * glyph).translation();
        self.glyphs.push(PdfGlyph { x: at.x, y: at.y, line_y: t.translation().y, text, mode });
    }
    fn draw_image(&mut self, _: Image<'_, '_>, _: Affine) {}
    fn pop_clip_path(&mut self) {}
    fn pop_transparency_group(&mut self) {}
}
