// SPDX-License-Identifier: MIT OR Apache-2.0
//! Page images: a page, or part of one, drawn into RGBA pixels.

use jcj_doc::Document;
use jcj_layout::{DocLayout, Rect};
use jcj_upstream::render::{RenderOptions, Rendered};

/// Largest image side, pixels: an image that would be larger is cut off at this size.
pub const MAX_SIDE: u32 = jcj_upstream::render::MAX_SIDE;

/// An RGBA image, 8 bits a channel, rows from the top, alpha premultiplied.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct PageImage {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes.
    pub pixels: Vec<u8>,
}

impl std::fmt::Debug for PageImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PageImage({}x{})", self.width, self.height)
    }
}

impl PageImage {
    fn from_rendered(r: Rendered) -> Self {
        PageImage { width: r.width, height: r.height, pixels: r.pixels }
    }

    fn rendered(&self) -> Rendered {
        Rendered { width: self.width, height: self.height, pixels: self.pixels.clone() }
    }

    /// The pixels with straight (not premultiplied) alpha, as most image formats want them.
    pub fn straight_rgba(&self) -> Vec<u8> {
        self.rendered().to_straight()
    }

    /// The image as PNG bytes.
    pub fn to_png(&self) -> Vec<u8> {
        self.rendered().to_png()
    }

    /// The pixel at (`x`, `y`) as premultiplied RGBA, or `None` outside the image.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        let p = self.pixels.get(i..i + 4)?;
        Some([p[0], p[1], p[2], p[3]])
    }
}

/// How pages are drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderSettings {
    /// Draw formatting marks (paragraph marks, spaces, tabs) as the editor shows them.
    pub marks: bool,
    /// Draw text for a screen: thickened and hinted as the platform draws text. Off, the image
    /// is exact, the same on every machine.
    pub screen_text: bool,
    /// Dark page: every color but pictures has its lightness turned around.
    pub dark: bool,
    /// Show tracked changes as markup instead of the final text.
    pub markup: bool,
}

impl RenderSettings {
    fn options(&self) -> RenderOptions {
        let mut display = jcj_upstream::pdf::PdfOptions { include_markup: self.markup, ..Default::default() }.display_options();
        display.marks = self.marks;
        RenderOptions { display, text_darkening: self.screen_text, hinting: self.screen_text, dark: self.dark, ..Default::default() }
    }
}

/// Draw page `page` (0-based) of `layout` at `scale` pixels per point (1.0: 72 pixels per inch).
/// `None` when there is no such page.
pub fn render_page(doc: &Document, layout: &DocLayout, page: usize, scale: f32, settings: &RenderSettings) -> Option<PageImage> {
    let p = layout.pages.get(page)?;
    Some(PageImage::from_rendered(jcj_upstream::render::render_page(doc, p, scale, &settings.options())))
}

/// Draw `area` (points) of page `page` at `scale` pixels per point: a thumbnail or a preview of
/// part of a page. `None` when there is no such page.
pub fn render_area(doc: &Document, layout: &DocLayout, page: usize, area: Rect, scale: f32, settings: &RenderSettings) -> Option<PageImage> {
    let p = layout.pages.get(page)?;
    Some(PageImage::from_rendered(jcj_upstream::render::render_area(doc, p, area, scale, &settings.options())))
}
