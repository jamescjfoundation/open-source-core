// SPDX-License-Identifier: MIT OR Apache-2.0
//! PDF facts: what a print service or a preflight check asks of a PDF.
//!
//! [`read_facts`] reads a PDF's page boxes (media, crop, trim, bleed), annotations, images with
//! their pixel size and placed size (so their resolution), fonts with whether each is embedded,
//! and its bookmarks. Apps judge the facts against their own rules; nothing here decides what
//! is acceptable.
//!
//! Files are not trusted: the input size and page count are bounded ([`ReadLimits`]), as are
//! the fonts, images, annotations and bookmarks counted, and a file the reader cannot read gives
//! a [`ReadError`], never a panic.

use std::collections::BTreeMap;

use jcj_upstream::pdf_read::interpret::font::Glyph;
use jcj_upstream::pdf_read::interpret::{
    BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache, InterpreterSettings, Paint, PathDrawMode, SoftMask,
};
use jcj_upstream::pdf_read::syntax::object::{Array, Dict, Name, Rect as PdfRect};
use jcj_upstream::pdf_read::syntax::page::Page;
use jcj_upstream::pdf_read::syntax::xref::XRef;
use jcj_upstream::pdf_read::{OpenOptions, interpret_page, open_slice_with_options};
use kurbo::{Affine, BezPath, Rect, Vec2};

use crate::read::{ReadError, ReadLimits};

/// Most of anything counted on one page, or in the file (fonts, images, annotations,
/// bookmarks). A count at this bound means "this many or more".
pub const MAX_COUNTED: usize = 100_000;

/// Deepest nesting of forms or bookmarks followed.
const MAX_DEPTH: usize = 16;

/// A page box, in PDF units (points), from the page's bottom left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageBox {
    /// Left edge.
    pub x0: f64,
    /// Bottom edge.
    pub y0: f64,
    /// Right edge.
    pub x1: f64,
    /// Top edge.
    pub y1: f64,
}

impl PageBox {
    /// Its width.
    pub fn width(&self) -> f64 {
        (self.x1 - self.x0).abs()
    }

    /// Its height.
    pub fn height(&self) -> f64 {
        (self.y1 - self.y0).abs()
    }

    /// How far it reaches past `inner` on each side (left, bottom, right, top); negative where
    /// it falls short.
    pub fn margins_around(&self, inner: &PageBox) -> [f64; 4] {
        [inner.x0 - self.x0, inner.y0 - self.y0, self.x1 - inner.x1, self.y1 - inner.y1]
    }

    fn from_pdf(r: PdfRect) -> Self {
        Self { x0: r.x0.min(r.x1), y0: r.y0.min(r.y1), x1: r.x0.max(r.x1), y1: r.y0.max(r.y1) }
    }
}

/// An image drawn on a page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PdfImage {
    /// Its width in pixels.
    pub pixel_width: u32,
    /// Its height in pixels.
    pub pixel_height: u32,
    /// The width it is drawn at, in points.
    pub placed_width: f64,
    /// The height it is drawn at, in points.
    pub placed_height: f64,
    /// Whether it is a stencil mask (one-bit shape painted in the fill colour).
    pub mask: bool,
}

impl PdfImage {
    /// Its resolution as drawn, in pixels per inch: the lower of across and down. Zero when it
    /// is drawn with no size.
    pub fn ppi(&self) -> f64 {
        let across = if self.placed_width > 0.0 { f64::from(self.pixel_width) * 72.0 / self.placed_width } else { 0.0 };
        let down = if self.placed_height > 0.0 { f64::from(self.pixel_height) * 72.0 / self.placed_height } else { 0.0 };
        across.min(down)
    }
}

/// A font a PDF uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PdfFont {
    /// Its name as the PDF gives it (`BaseFont`), without a subset prefix.
    pub name: String,
    /// Its PDF type (`Type1`, `TrueType`, `Type0`, `Type3`, ...).
    pub kind: String,
    /// Whether its glyphs are in the file (a font file, or a `Type3` font's own drawings).
    pub embedded: bool,
    /// Whether it is a subset (its name carries a six-letter subset prefix).
    pub subset: bool,
}

/// The facts of one page.
#[derive(Clone, Debug, PartialEq)]
pub struct PageFacts {
    /// The media box: the whole sheet.
    pub media: PageBox,
    /// The crop box: what viewers show (the media box when the page has none).
    pub crop: PageBox,
    /// The trim box, when the page has one: the finished page after cutting.
    pub trim: Option<PageBox>,
    /// The bleed box, when the page has one: how far printing runs past the trim.
    pub bleed: Option<PageBox>,
    /// Its rotation in degrees (0, 90, 180 or 270).
    pub rotation: u16,
    /// Its annotations' types (`Link`, `Widget`, `Text`, ...), in order.
    pub annotations: Vec<String>,
    /// The images it draws, in drawing order (images in forms included).
    pub images: Vec<PdfImage>,
    /// The names of the fonts its resources hold (forms included), sorted.
    pub fonts: Vec<String>,
}

/// The facts of a PDF.
#[derive(Clone, Debug, PartialEq)]
pub struct PdfFacts {
    /// Every page, in order.
    pub pages: Vec<PageFacts>,
    /// Every font the pages use, once each, sorted by name.
    pub fonts: Vec<PdfFont>,
    /// How many bookmarks (outline entries) it has, at every level.
    pub bookmarks: usize,
}

impl PdfFacts {
    /// The fonts that are not embedded.
    pub fn unembedded_fonts(&self) -> Vec<&PdfFont> {
        self.fonts.iter().filter(|f| !f.embedded).collect()
    }

    /// How many annotations the pages have in all.
    pub fn annotation_count(&self) -> usize {
        self.pages.iter().map(|p| p.annotations.len()).sum()
    }
}

/// The facts of a PDF: its pages' boxes, annotations, images and fonts, and its bookmarks.
///
/// Images are found by drawing each page (as [`crate::read_pages`] does), so an image drawn
/// twice counts twice and an image only in the resources is not counted.
pub fn read_facts(bytes: &[u8], limits: &ReadLimits) -> Result<PdfFacts, ReadError> {
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
    let mut fonts: BTreeMap<String, PdfFont> = BTreeMap::new();
    let mut out = Vec::with_capacity(pages.len());
    for (i, page) in pages.iter().enumerate() {
        let mut ctx = Context::new(Affine::IDENTITY, Rect::new(0.0, 0.0, 1.0, 1.0), &cache, file.xref(), InterpreterSettings::default());
        let mut images = Images { images: Vec::new() };
        interpret_page(page, &mut ctx, &mut images).map_err(|_| ReadError::BadPage(i + 1))?;
        let mut names = Vec::new();
        page_fonts(&page.resources().fonts, &page.resources().x_objects, 0, &mut names, &mut fonts);
        names.sort();
        names.dedup();
        out.push(PageFacts {
            media: PageBox::from_pdf(page.media_box()),
            crop: PageBox::from_pdf(page.crop_box()),
            trim: page.raw().get::<PdfRect>(b"TrimBox").map(PageBox::from_pdf),
            bleed: page.raw().get::<PdfRect>(b"BleedBox").map(PageBox::from_pdf),
            rotation: rotation(page),
            annotations: annotations(page),
            images: images.images,
            fonts: names,
        });
    }
    Ok(PdfFacts { pages: out, fonts: fonts.into_values().collect(), bookmarks: bookmarks(file.xref()) })
}

fn rotation(page: &Page<'_>) -> u16 {
    let r = page.raw().get::<i64>(b"Rotate").unwrap_or(0).rem_euclid(360);
    u16::try_from(r - r % 90).unwrap_or(0)
}

fn annotations(page: &Page<'_>) -> Vec<String> {
    let Some(list) = page.raw().get::<Array<'_>>(b"Annots") else { return Vec::new() };
    list.iter::<Dict<'_>>()
        .take(MAX_COUNTED)
        .map(|a| a.get::<Name<'_>>(b"Subtype").map_or_else(|| "Unknown".to_owned(), |n| n.as_str().to_owned()))
        .collect()
}

/// The fonts in a resources dictionary, and in its forms' resources, recorded once each.
fn page_fonts(font_dict: &Dict<'_>, x_objects: &Dict<'_>, depth: usize, names: &mut Vec<String>, all: &mut BTreeMap<String, PdfFont>) {
    if depth > MAX_DEPTH || all.len() >= MAX_COUNTED {
        return;
    }
    for key in font_dict.keys().collect::<Vec<_>>() {
        let Some(font) = font_dict.get::<Dict<'_>>(key.as_ref()) else { continue };
        let f = font_facts(&font);
        names.push(f.name.clone());
        all.entry(f.name.clone()).or_insert(f);
    }
    for key in x_objects.keys().collect::<Vec<_>>() {
        let Some(form) = x_objects.get::<jcj_upstream::pdf_read::syntax::object::Stream<'_>>(key.as_ref()) else { continue };
        let dict = form.dict();
        if dict.get::<Name<'_>>(b"Subtype").is_none_or(|s| s.as_str() != "Form") {
            continue;
        }
        let Some(res) = dict.get::<Dict<'_>>(b"Resources") else { continue };
        let fonts = res.get::<Dict<'_>>(b"Font").unwrap_or_else(Dict::empty);
        let forms = res.get::<Dict<'_>>(b"XObject").unwrap_or_else(Dict::empty);
        page_fonts(&fonts, &forms, depth + 1, names, all);
    }
}

fn font_facts(font: &Dict<'_>) -> PdfFont {
    let kind = font.get::<Name<'_>>(b"Subtype").map_or_else(String::new, |n| n.as_str().to_owned());
    let raw_name = font.get::<Name<'_>>(b"BaseFont").map_or_else(|| "(unnamed)".to_owned(), |n| n.as_str().to_owned());
    let subset = raw_name.len() > 7 && raw_name.as_bytes()[6] == b'+' && raw_name.as_bytes()[..6].iter().all(u8::is_ascii_uppercase);
    let name = if subset { raw_name[7..].to_owned() } else { raw_name };
    let embedded = match kind.as_str() {
        "Type3" => true,
        "Type0" => font.get::<Array<'_>>(b"DescendantFonts").and_then(|a| a.iter::<Dict<'_>>().next()).is_some_and(|d| has_font_file(&d)),
        _ => has_font_file(font),
    };
    PdfFont { name, kind, embedded, subset }
}

fn has_font_file(font: &Dict<'_>) -> bool {
    font.get::<Dict<'_>>(b"FontDescriptor")
        .is_some_and(|d| d.contains_key(b"FontFile") || d.contains_key(b"FontFile2") || d.contains_key(b"FontFile3"))
}

/// How many outline entries the document has, following `First` and `Next`, bounded.
fn bookmarks(xref: &XRef) -> usize {
    let Some(catalog) = xref.get::<Dict<'_>>(xref.root_id()) else { return 0 };
    let Some(outlines) = catalog.get::<Dict<'_>>(b"Outlines") else { return 0 };
    let mut count = 0;
    count_items(outlines.get::<Dict<'_>>(b"First"), 0, &mut count);
    count
}

fn count_items(mut item: Option<Dict<'_>>, depth: usize, count: &mut usize) {
    if depth > MAX_DEPTH {
        return;
    }
    while let Some(entry) = item {
        if *count >= MAX_COUNTED {
            return;
        }
        *count += 1;
        count_items(entry.get::<Dict<'_>>(b"First"), depth + 1, count);
        item = entry.get::<Dict<'_>>(b"Next");
    }
}

/// A drawing device that keeps only images.
struct Images {
    images: Vec<PdfImage>,
}

impl Device<'_> for Images {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'_>>) {}
    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn draw_path(&mut self, _: &BezPath, _: Affine, _: &Paint<'_>, _: &PathDrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'_>>, _: BlendMode) {}
    fn draw_glyph(&mut self, _: &Glyph<'_>, _: Affine, _: Affine, _: &Paint<'_>, _: &GlyphDrawMode) {}
    fn draw_image(&mut self, image: Image<'_, '_>, t: Affine) {
        if self.images.len() >= MAX_COUNTED {
            return;
        }
        let (w, h) = (image.width(), image.height());
        // The transform maps the image's pixels onto the page.
        let across = t * Vec2::new(f64::from(w), 0.0).to_point() - t * kurbo::Point::ZERO;
        let down = t * Vec2::new(0.0, f64::from(h)).to_point() - t * kurbo::Point::ZERO;
        self.images.push(PdfImage {
            pixel_width: w,
            pixel_height: h,
            placed_width: across.hypot(),
            placed_height: down.hypot(),
            mask: matches!(image, Image::Stencil(_)),
        });
    }
    fn pop_clip_path(&mut self) {}
    fn pop_transparency_group(&mut self) {}
}
