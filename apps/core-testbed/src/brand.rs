// SPDX-License-Identifier: MIT OR Apache-2.0
//! The test app's brand, loaded the way an app loads its own (`jcj_theme::brand`): a tokens
//! file with the neutral colors and a section of the testbed's own, an icon for the window and
//! a mark painted from a mask.
//!
//! An app embeds picture files. The core's repository holds none, so the testbed draws its two
//! pictures in code and encodes them as PNG once; everything after that is what an app does.

use std::sync::OnceLock;

use jcj_shell::egui::{Color32, Painter, Rect};
use jcj_shell::snapshot::Picture;
use jcj_shell::theme::{Brand, Mask, parse_hex};

/// The testbed's tokens file: the neutral set by leaving every color out, and its own section.
pub const TOKENS_JSON: &str = include_str!("../brand/tokens.json");

/// The side of the icon and of the mark's mask, pixels.
const SIDE: u32 = 64;

/// The testbed's own section of the tokens file.
#[derive(serde::Deserialize)]
struct Own {
    mark: String,
}

/// The color of the mark on the icon, from the tokens file's `testbed` section.
pub fn mark_color() -> Color32 {
    Brand::new(TOKENS_JSON, &[]).section::<Own>("testbed").and_then(|own| parse_hex(&own.mark).ok()).unwrap_or(Color32::DARK_GRAY)
}

/// Whether pixel (x, y) is inside the mark: a sheet of paper with its corner folded.
fn in_mark(x: u32, y: u32) -> bool {
    let (left, right, top, bottom, fold) = (16, 48, 8, 56, 12);
    (left..right).contains(&x) && (top..bottom).contains(&y) && (x - left) + (bottom - 1 - y) < (right - left) + (bottom - top) - fold
}

fn png(pixel: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let rgba = (0..SIDE).flat_map(|y| (0..SIDE).map(move |x| (x, y))).flat_map(|(x, y)| pixel(x, y)).collect();
    Picture { width: SIDE, height: SIDE, rgba }.to_png()
}

/// The icon: the mark in white on a tile of the mark's color.
pub fn icon_png() -> &'static [u8] {
    static ICON: OnceLock<Vec<u8>> = OnceLock::new();
    ICON.get_or_init(|| {
        let tile = mark_color();
        png(|x, y| if in_mark(x, y) { [255, 255, 255, 255] } else { [tile.r(), tile.g(), tile.b(), 255] })
    })
}

/// The mark's mask: the shape in its alpha, to be painted in any color.
fn mask_png() -> &'static [u8] {
    static MASK: OnceLock<Vec<u8>> = OnceLock::new();
    MASK.get_or_init(|| png(|x, y| [255, 255, 255, if in_mark(x, y) { 255 } else { 0 }]))
}

/// The testbed's brand files.
pub fn brand() -> Brand {
    Brand::new(TOKENS_JSON, icon_png())
}

/// The testbed's mark.
pub fn mark() -> Mask {
    Mask::new("core-testbed-mark", mask_png())
}

/// The mark's width at `height` points (it is square).
pub fn mark_width(height: f32) -> f32 {
    height
}

/// Paint the mark in `rect` in `ink`, as the shell asks for it in the quick-access row and on
/// the About page.
pub fn paint_mark(painter: &Painter, rect: Rect, ink: Color32) {
    let side = rect.width().min(rect.height());
    mark().paint(painter, Rect::from_center_size(rect.center(), jcj_shell::egui::vec2(side, side)), ink);
}

#[cfg(test)]
mod tests {
    use jcj_shell::theme::brand::{contrast, decode_png, hue, hue_distance, ico_sizes, luminance};
    use jcj_shell::theme::{Mode, Tokens};

    use super::*;

    #[test]
    fn the_tokens_file_reads_and_is_the_neutral_set_with_the_testbed_s_section() {
        let tokens = brand().read_tokens().unwrap();
        assert_eq!(tokens.name, "core-testbed");
        assert_eq!(tokens.light, Tokens::default().light);
        assert_eq!(brand().tokens(), tokens);
        assert_eq!(mark_color(), Color32::from_rgb(0x2F, 0x5D, 0x8C));
    }

    #[test]
    fn the_icon_and_the_mark_decode() {
        let icon = brand().icon().unwrap();
        assert_eq!((icon.width, icon.height), (SIDE, SIDE));
        let (width, height, rgba) = decode_png(mask_png()).unwrap();
        assert_eq!((width, height), (SIDE, SIDE));
        let alpha = |x: u32, y: u32| rgba[((y * SIDE + x) * 4 + 3) as usize];
        assert_eq!((alpha(20, 40), alpha(2, 2), alpha(46, 9)), (255, 0, 0), "the sheet, the ground, the folded corner");
        let ctx = jcj_shell::egui::Context::default();
        assert_eq!(mark().texture(&ctx).map(|t| t.size()), Some([SIDE as usize, SIDE as usize]));
        assert_eq!(mark_width(18.0), 18.0);
    }

    /// The checks an app's own brand test makes, on the testbed's colors.
    #[test]
    fn the_mark_s_color_is_readable_on_the_neutral_surfaces() {
        let mark = mark_color();
        for mode in [Mode::Light, Mode::Dark, Mode::ExtraDark] {
            let theme = brand().tokens().theme(mode);
            assert!(contrast(theme.text, theme.surface) >= 4.5, "{mode:?}: text");
            assert!(hue_distance(theme.danger, mark) >= 15.0, "{mode:?}: the mark is not taken for a problem");
        }
        assert!(contrast(Color32::WHITE, mark) >= 4.5, "the white mark on its tile");
        assert!(luminance(mark) < luminance(Color32::WHITE));
        assert!((180.0..240.0).contains(&hue(mark)), "a blue: {}", hue(mark));
    }

    /// A Windows icon file is held to its sizes the same way; here one made around the icon.
    #[test]
    fn an_icon_file_s_sizes_are_checked() {
        let png = icon_png();
        let mut ico = vec![0, 0, 1, 0, 1, 0, SIDE as u8, SIDE as u8, 0, 0, 1, 0, 32, 0];
        ico.extend((png.len() as u32).to_le_bytes());
        ico.extend(22u32.to_le_bytes());
        ico.extend(png);
        assert_eq!(ico_sizes(&ico), Ok(vec![SIDE]));
        assert!(ico_sizes(png).is_err());
    }

    #[test]
    fn the_brand_dresses_a_context_the_shell_does_not_manage() {
        let ctx = jcj_shell::egui::Context::default();
        brand().apply(&ctx, Mode::Dark);
        assert_eq!(jcj_shell::theme::Theme::get(&ctx).mode, Mode::Dark);
    }
}
