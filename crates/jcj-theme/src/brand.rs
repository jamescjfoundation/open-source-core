// SPDX-License-Identifier: MIT OR Apache-2.0
//! A product's brand files, as its app embeds them, and what every app does with them.
//!
//! The brand itself is the app's: its `tokens.json`, its icon and the pictures of its mark live
//! in the app's repository and are compiled in with `include_str!` and `include_bytes!`. What is
//! the same in every app is here:
//!
//! - [`Brand`] reads the tokens (the neutral set when the file cannot be read, so a window
//!   always opens), the app's own section of the tokens file, and the window's icon;
//! - [`Mask`] is a picture kept as a white shape and tinted when it is painted, so one file
//!   draws a mark in any theme;
//! - [`contrast`] and its helpers are the WCAG figures a brand's tests hold its colors to;
//! - [`ico_sizes`] reads the sizes a Windows icon file holds, for the test of the file a build
//!   script embeds.

use std::sync::Arc;

use egui::{Color32, Context, IconData, Id, Painter, Rect, TextureHandle, TextureOptions, pos2};
use serde::de::DeserializeOwned;

use crate::{Mode, Tokens, TokensError};

/// The largest side of a picture decoded here, pixels. An app's own files are trusted, but a
/// bound costs nothing and keeps a wrong file from asking for gigabytes.
pub const MAX_SIDE: u32 = 4096;

/// A product's brand files.
///
/// ```
/// use jcj_theme::Brand;
///
/// // In an app: `Brand::new(include_str!("../assets/brand/tokens.json"), include_bytes!("../assets/brand/icon-256.png"))`.
/// const BRAND: Brand = Brand::NEUTRAL;
/// assert_eq!(BRAND.tokens(), jcj_theme::Tokens::default());
/// assert!(BRAND.icon().is_none());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Brand {
    /// The tokens file's text (`TOKENS.md`); empty for the neutral set.
    pub tokens_json: &'static str,
    /// The app icon as a PNG, for the window; empty for none.
    pub icon_png: &'static [u8],
}

impl Brand {
    /// No brand: the neutral tokens and no icon.
    pub const NEUTRAL: Brand = Brand { tokens_json: "", icon_png: &[] };

    /// The brand in these files.
    pub const fn new(tokens_json: &'static str, icon_png: &'static [u8]) -> Brand {
        Brand { tokens_json, icon_png }
    }

    /// The tokens, or why the file does not read. A test of the app calls this, so a damaged
    /// file fails the build and never reaches a user.
    pub fn read_tokens(&self) -> Result<Tokens, TokensError> {
        if self.tokens_json.trim().is_empty() { Ok(Tokens::default()) } else { Tokens::from_json(self.tokens_json) }
    }

    /// The tokens; the neutral set if the file does not read, so the window still opens.
    pub fn tokens(&self) -> Tokens {
        self.read_tokens().unwrap_or_default()
    }

    /// The app's own part of the tokens file: the value under the top-level key `name`, read
    /// into `T` (an editor's page colors, say). `None` when the key is missing or does not fit.
    pub fn section<T: DeserializeOwned>(&self, name: &str) -> Option<T> {
        let mut file: serde_json::Map<String, serde_json::Value> = serde_json::from_str(self.tokens_json).ok()?;
        serde_json::from_value(file.remove(name)?).ok()
    }

    /// The window's icon, decoded from the PNG; `None` when there is none or it does not decode.
    pub fn icon(&self) -> Option<Arc<IconData>> {
        let (width, height, rgba) = decode_png(self.icon_png)?;
        Some(Arc::new(IconData { rgba, width, height }))
    }

    /// Install the brand's fonts and theme in a context the shell does not manage (a test, an
    /// offscreen picture). The shell does this itself for the app's window.
    pub fn apply(&self, ctx: &Context, mode: Mode) {
        let tokens = self.tokens();
        let (names, faces) = crate::with_defaults(&tokens.fonts, &[]);
        crate::install_fonts(ctx, &names, &faces);
        crate::apply(ctx, &tokens.theme(mode));
    }
}

/// A PNG's width, height and pixels as straight RGBA; `None` when it does not decode or a side
/// is zero or over [`MAX_SIDE`].
pub fn decode_png(png: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    if png.is_empty() {
        return None;
    }
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(png), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    reader.limits(limits);
    let picture = reader.decode().ok()?.into_rgba8();
    let (width, height) = picture.dimensions();
    (width > 0 && height > 0).then(|| (width, height, picture.into_raw()))
}

/// A picture kept as a mask: a PNG whose alpha is the shape. It is painted in any color, so one
/// file draws a mark on a light ground and on a dark one.
///
/// The texture is made once for each egui context and kept there under `key`, which must be
/// the mask's own (two masks with one key would share a picture).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mask {
    /// The mask's name among the app's pictures.
    pub key: &'static str,
    /// The PNG.
    pub png: &'static [u8],
}

impl Mask {
    /// The mask in `png`, kept as `key`.
    pub const fn new(key: &'static str, png: &'static [u8]) -> Mask {
        Mask { key, png }
    }

    /// The mask's texture in `ctx`: white with the PNG's alpha. `None` when the PNG does not
    /// decode (the answer is kept too, so a bad file is not decoded every frame).
    pub fn texture(&self, ctx: &Context) -> Option<TextureHandle> {
        let id = Id::unique(("jcj-theme-mask", self.key));
        if let Some(kept) = ctx.data(|d| d.get_temp::<Option<TextureHandle>>(id)) {
            return kept;
        }
        let texture = decode_png(self.png).and_then(|(width, height, rgba)| {
            let size = [usize::try_from(width).ok()?, usize::try_from(height).ok()?];
            // White with the mask's coverage, so a tint paints it in any color.
            let pixels = rgba.as_chunks::<4>().0.iter().map(|p| Color32::from_white_alpha(p[3])).collect();
            Some(ctx.load_texture(format!("mask-{}", self.key), egui::ColorImage::new(size, pixels), TextureOptions::LINEAR))
        });
        ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
        texture
    }

    /// Paint the whole mask into `rect` in `ink`. Returns false when there is nothing to paint.
    pub fn paint(&self, painter: &Painter, rect: Rect, ink: Color32) -> bool {
        self.paint_part(painter, rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), ink)
    }

    /// Paint the part `uv` of the mask (0 to 1 across and down) into `rect` in `ink`.
    pub fn paint_part(&self, painter: &Painter, rect: Rect, uv: Rect, ink: Color32) -> bool {
        let Some(texture) = self.texture(painter.ctx()) else { return false };
        painter.image(texture.id(), rect, uv, ink);
        true
    }
}

/// WCAG 2.x relative luminance of a color, 0 (black) to 1 (white). Alpha is ignored.
pub fn luminance(c: Color32) -> f32 {
    let lin = |v: u8| {
        let v = f32::from(v) / 255.0;
        if v <= 0.040_45 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
}

/// WCAG 2.x contrast ratio of two colors, 1 to 21. Text needs 4.5 (AA) and marks that are not
/// text, such as a focus ring, need 3.
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// A color's hue in degrees, 0 to 360 (0 for a gray).
pub fn hue(c: Color32) -> f32 {
    let [r, g, b] = [c.r(), c.g(), c.b()].map(|v| f32::from(v) / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let d = max - min;
    if d == 0.0 {
        return 0.0;
    }
    let h = if max == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    h * 60.0
}

/// How far apart two colors' hues are, in degrees, 0 to 180.
pub fn hue_distance(a: Color32, b: Color32) -> f32 {
    let apart = (hue(a) - hue(b)).rem_euclid(360.0);
    apart.min(360.0 - apart)
}

/// The sizes of the pictures in a Windows icon file (`.ico`), in the file's order, in pixels:
/// the file a build script embeds in the program. Each picture must be 32-bit and lie inside
/// the file; anything else is an error that says what is wrong.
pub fn ico_sizes(ico: &[u8]) -> Result<Vec<u32>, String> {
    let word = |at: usize| ico.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let long = |at: usize| ico.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    if (word(0), word(2)) != (Some(0), Some(1)) {
        return Err("not an icon file".into());
    }
    let count = usize::from(word(4).ok_or("the file ends in its header")?);
    let mut sizes = Vec::with_capacity(count);
    for i in 0..count {
        let entry = 6 + 16 * i;
        let side = ico.get(entry).map(|&b| if b == 0 { 256 } else { u32::from(b) }).ok_or("the file ends in its list of pictures")?;
        let (Some(bits), Some(len), Some(start)) = (word(entry + 6), long(entry + 8), long(entry + 12)) else {
            return Err("the file ends in its list of pictures".into());
        };
        if bits != 32 {
            return Err(format!("the {side} px picture is {bits}-bit, not 32-bit"));
        }
        if u64::from(start) + u64::from(len) > ico.len() as u64 {
            return Err(format!("the {side} px picture lies outside the file"));
        }
        sizes.push(side);
    }
    Ok(sizes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4 by 2 PNG: opaque on the left half, clear on the right.
    fn half_mask() -> Vec<u8> {
        let mut picture = image::RgbaImage::new(4, 2);
        for (x, _, p) in picture.enumerate_pixels_mut() {
            *p = image::Rgba([10, 20, 30, if x < 2 { 255 } else { 0 }]);
        }
        let mut out = std::io::Cursor::new(Vec::new());
        picture.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    const TOKENS: &str = r##"{
        "name": "sample",
        "light": { "text": "#102030", "surface": "#FAFAF0" },
        "editor": { "font": "Sample Serif", "page": "#F4ECD8" }
    }"##;

    #[test]
    fn a_brand_reads_its_tokens_and_falls_back_to_the_neutral_set() {
        let brand = Brand::new(TOKENS, &[]);
        assert_eq!(brand.tokens().light.text, Color32::from_rgb(0x10, 0x20, 0x30));
        assert_eq!(brand.tokens().name, "sample");
        // A file that does not read: an error for the app's test, the neutral set for its window.
        let damaged = Brand::new("{ not json", &[]);
        assert!(damaged.read_tokens().is_err());
        assert_eq!(damaged.tokens(), Tokens::default());
        assert_eq!(Brand::NEUTRAL.read_tokens().unwrap(), Tokens::default());
    }

    #[test]
    fn the_app_s_own_section_is_read_from_the_same_file() {
        #[derive(serde::Deserialize, PartialEq, Debug)]
        struct Editor {
            font: String,
            page: String,
        }
        let brand = Brand::new(TOKENS, &[]);
        assert_eq!(brand.section::<Editor>("editor"), Some(Editor { font: "Sample Serif".into(), page: "#F4ECD8".into() }));
        assert_eq!(brand.section::<Editor>("missing"), None);
        assert_eq!(brand.section::<Editor>("light"), None, "a section of another shape does not fit");
        assert_eq!(Brand::NEUTRAL.section::<Editor>("editor"), None);
    }

    #[test]
    fn the_icon_is_decoded_for_the_window() {
        let png: &'static [u8] = half_mask().leak();
        let icon = Brand::new("", png).icon().unwrap();
        assert_eq!((icon.width, icon.height, icon.rgba.len()), (4, 2, 32));
        assert_eq!(&icon.rgba[..4], [10, 20, 30, 255]);
        assert!(Brand::new("", b"not a png").icon().is_none());
        assert!(Brand::NEUTRAL.icon().is_none());
    }

    #[test]
    fn a_picture_larger_than_the_bound_is_refused() {
        let mut out = std::io::Cursor::new(Vec::new());
        image::RgbaImage::new(MAX_SIDE + 1, 1).write_to(&mut out, image::ImageFormat::Png).unwrap();
        assert!(decode_png(&out.into_inner()).is_none());
    }

    #[test]
    fn a_mask_is_white_with_the_picture_s_alpha_and_is_made_once() {
        let ctx = Context::default();
        let png: &'static [u8] = half_mask().leak();
        let mask = Mask::new("sample-mark", png);
        let first = mask.texture(&ctx).unwrap();
        assert_eq!(first.size(), [4, 2]);
        assert_eq!(mask.texture(&ctx).unwrap().id(), first.id(), "kept in the context");
        // A file that does not decode is asked for once, and paints nothing.
        let bad = Mask::new("bad-mark", b"nothing");
        assert!(bad.texture(&ctx).is_none());
        assert!(bad.texture(&ctx).is_none());
    }

    #[test]
    fn a_brand_can_dress_a_context_the_shell_does_not_manage() {
        let ctx = Context::default();
        Brand::new(TOKENS, &[]).apply(&ctx, Mode::Light);
        assert_eq!(crate::Theme::get(&ctx).text, Color32::from_rgb(0x10, 0x20, 0x30));
    }

    #[test]
    fn contrast_is_the_wcag_ratio() {
        assert!((contrast(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 0.01);
        assert!((contrast(Color32::WHITE, Color32::WHITE) - 1.0).abs() < 0.001);
        assert_eq!(contrast(Color32::BLACK, Color32::RED), contrast(Color32::RED, Color32::BLACK));
        // #767676 on white is the well-known edge of AA for text.
        let edge = contrast(Color32::from_rgb(0x76, 0x76, 0x76), Color32::WHITE);
        assert!((4.5..4.6).contains(&edge), "{edge}");
        assert!(luminance(Color32::WHITE) > 0.999 && luminance(Color32::BLACK) < 0.001);
    }

    #[test]
    fn hue_is_in_degrees() {
        assert_eq!(hue(Color32::RED), 0.0);
        assert_eq!(hue(Color32::GREEN), 120.0);
        assert_eq!(hue(Color32::BLUE), 240.0);
        assert_eq!(hue(Color32::GRAY), 0.0);
        assert_eq!(hue_distance(Color32::RED, Color32::BLUE), 120.0);
        assert_eq!(hue_distance(Color32::BLUE, Color32::RED), 120.0);
    }

    /// An icon file with one picture entry per size; the pictures themselves are a byte each.
    fn ico(sizes: &[u32], bits: u16) -> Vec<u8> {
        let mut out = vec![0, 0, 1, 0];
        out.extend((sizes.len() as u16).to_le_bytes());
        let data = 6 + 16 * sizes.len();
        for (i, side) in sizes.iter().enumerate() {
            let byte = if *side == 256 { 0 } else { *side as u8 };
            out.extend([byte, byte, 0, 0, 1, 0]);
            out.extend(bits.to_le_bytes());
            out.extend(1u32.to_le_bytes());
            out.extend(((data + i) as u32).to_le_bytes());
        }
        out.extend(std::iter::repeat_n(0u8, sizes.len()));
        out
    }

    #[test]
    fn a_windows_icon_s_sizes_are_read() {
        let sizes = [16, 24, 32, 48, 64, 128, 256];
        assert_eq!(ico_sizes(&ico(&sizes, 32)).unwrap(), sizes);
        assert!(ico_sizes(&ico(&sizes, 8)).unwrap_err().contains("8-bit"));
        assert!(ico_sizes(b"PNG.").is_err());
        let mut cut = ico(&sizes, 32);
        cut.truncate(cut.len() - 1);
        assert!(ico_sizes(&cut).unwrap_err().contains("outside the file"));
        assert!(ico_sizes(&cut[..20]).is_err());
    }
}
