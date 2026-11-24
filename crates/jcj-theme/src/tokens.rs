// SPDX-License-Identifier: MIT OR Apache-2.0
//! Design tokens: every color, size and space the interface's chrome uses, in three themes.
//!
//! [`Tokens`] holds a product's whole set as read from its `tokens.json` (`TOKENS.md` describes
//! the file); [`Theme`] is the part of it in use: one mode's colors with the shared sizes.

use std::collections::BTreeMap;

use egui::{Color32, Context, Id};
use serde::{Deserialize, Serialize};

use crate::color::{ColorError, hex, parse_hex};

/// Why a token file could not be read.
#[derive(Debug, thiserror::Error)]
pub enum TokensError {
    /// The file is not JSON of the tokens' shape.
    #[error("the tokens are not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// A color is neither `#RRGGBB`, `#RRGGBBAA` nor a name in `palette`.
    #[error("{role}: {source}")]
    Color {
        /// Where the color was, such as `dark.accent`.
        role: String,
        /// What was wrong with it.
        source: ColorError,
    },
}

/// Which of the three themes shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Light surfaces, dark text.
    #[default]
    Light,
    /// Dark surfaces, light text.
    Dark,
    /// Darker still, for dim rooms.
    ExtraDark,
}

impl Mode {
    /// Whether the mode has dark surfaces.
    pub fn is_dark(self) -> bool {
        self != Mode::Light
    }

    /// The mode for the two switches apps usually show: dark, and extra dark within dark.
    pub fn of(dark: bool, extra_dark: bool) -> Mode {
        match (dark, extra_dark) {
            (true, true) => Mode::ExtraDark,
            (true, false) => Mode::Dark,
            _ => Mode::Light,
        }
    }
}

/// Declares the color roles once: the resolved [`Colors`], the file's optional form, and the
/// conversions between them.
macro_rules! roles {
    ($($(#[doc = $doc:literal])* $field:ident,)*) => {
        /// One theme's colors, by role.
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct Colors {
            $($(#[doc = $doc])* pub $field: Color32,)*
        }

        /// One theme's colors as the file has them: hex or a palette name, any of them left out.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        struct RawColors {
            $(#[serde(skip_serializing_if = "Option::is_none")] $field: Option<String>,)*
        }

        impl RawColors {
            /// The colors, each one missing taken from `base`.
            fn resolve(&self, theme: &str, base: &Colors, palette: &BTreeMap<String, String>) -> Result<Colors, TokensError> {
                Ok(Colors {
                    $($field: match &self.$field {
                        Some(v) => color(v, palette).map_err(|source| TokensError::Color { role: format!("{theme}.{}", stringify!($field)), source })?,
                        None => base.$field,
                    },)*
                })
            }

            fn of(c: &Colors) -> Self {
                RawColors { $($field: Some(hex(c.$field)),)* }
            }
        }
    };
}

roles! {
    /// Behind the document's pages.
    canvas,
    /// Ribbon, panels, status bar.
    surface,
    /// The quick-access row, the tab strip, windows and cards on a surface.
    surface_high,
    /// Fields and list rows.
    input,
    /// Lines between parts of the window, and around fields and cards.
    border,
    /// Text.
    text,
    /// Secondary text: captions, hints, group titles.
    text_dim,
    /// The current tab, controls that are on, links, focus.
    accent,
    /// The background of controls that are on and of the selection.
    accent_soft,
    /// Hovered controls.
    hover,
    /// Problems and destructive actions.
    danger,
    /// The File page's column and the File button: the product's own color.
    brand,
    /// Text and icons on `brand`.
    on_brand,
    /// A small mark that picks out the current item (the current File page entry).
    highlight,
}

/// The interface's type sizes, points.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TypeSizes {
    /// Body text: labels, lists, fields.
    pub body: f32,
    /// Captions: group titles, hints, small buttons' second lines.
    pub caption: f32,
    /// Titles: panel and dialog titles.
    pub title: f32,
    /// Page headings, as on the File page.
    pub heading: f32,
}

/// Spacing, points.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Spacing {
    /// Space between items side by side.
    pub item_x: f32,
    /// Space between items one above another.
    pub item_y: f32,
    /// Padding inside buttons, across.
    pub button_x: f32,
    /// Padding inside buttons, up and down.
    pub button_y: f32,
    /// Margin inside panels.
    pub panel_margin: f32,
    /// Indent of nested items.
    pub indent: f32,
}

/// Corner radii, points.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Radius {
    /// Buttons, fields, tiles.
    pub control: u8,
    /// Windows and menus.
    pub window: u8,
    /// Cards and the File page's entries.
    pub card: u8,
    /// Chips in the status bar.
    pub chip: u8,
}

/// The names of the interface's fonts: each is a face the app supplies by that name
/// ([`crate::FontFace`]). A name left out, or a face the app does not supply, falls back to
/// `regular`, then to egui's own fonts.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FontNames {
    /// Text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regular: Option<String>,
    /// A heavier weight for tab names and button labels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub medium: Option<String>,
    /// The heaviest weight, for titles.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semibold: Option<String>,
    /// Monospace.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mono: Option<String>,
}

/// A product's design tokens: colors in three themes, fonts, type sizes, spacing and radii.
#[derive(Clone, Debug, PartialEq)]
pub struct Tokens {
    /// The token set's name (for messages; never shown as a product name by the core).
    pub name: String,
    /// The light theme's colors.
    pub light: Colors,
    /// The dark theme's colors.
    pub dark: Colors,
    /// The extra-dark theme's colors.
    pub extra_dark: Colors,
    /// The interface fonts' names.
    pub fonts: FontNames,
    /// Type sizes.
    pub type_sizes: TypeSizes,
    /// Spacing.
    pub spacing: Spacing,
    /// Corner radii.
    pub radius: Radius,
}

/// The neutral light theme: greys and a plain blue.
const NEUTRAL_LIGHT: Colors = Colors {
    canvas: Color32::from_rgb(0xE8, 0xE9, 0xEC),
    surface: Color32::from_rgb(0xF7, 0xF7, 0xF8),
    surface_high: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    input: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    border: Color32::from_rgb(0xD6, 0xD8, 0xDD),
    text: Color32::from_rgb(0x1E, 0x21, 0x25),
    text_dim: Color32::from_rgb(0x5C, 0x63, 0x70),
    accent: Color32::from_rgb(0x2F, 0x6F, 0xDE),
    accent_soft: Color32::from_rgb(0xE2, 0xEB, 0xFB),
    hover: Color32::from_rgb(0xEE, 0xEF, 0xF2),
    danger: Color32::from_rgb(0xB3, 0x36, 0x2A),
    brand: Color32::from_rgb(0x2B, 0x2F, 0x36),
    on_brand: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    highlight: Color32::from_rgb(0xE0, 0xA5, 0x26),
};

const NEUTRAL_DARK: Colors = Colors {
    canvas: Color32::from_rgb(0x14, 0x15, 0x18),
    surface: Color32::from_rgb(0x1C, 0x1E, 0x22),
    surface_high: Color32::from_rgb(0x23, 0x26, 0x2B),
    input: Color32::from_rgb(0x17, 0x19, 0x1C),
    border: Color32::from_rgb(0x33, 0x36, 0x3C),
    text: Color32::from_rgb(0xE8, 0xE9, 0xEB),
    text_dim: Color32::from_rgb(0x9E, 0xA3, 0xAD),
    accent: Color32::from_rgb(0x7E, 0xA6, 0xF2),
    accent_soft: Color32::from_rgb(0x26, 0x35, 0x52),
    hover: Color32::from_rgb(0x2A, 0x2D, 0x33),
    danger: Color32::from_rgb(0xE5, 0x73, 0x6A),
    brand: Color32::from_rgb(0x30, 0x34, 0x3B),
    on_brand: Color32::from_rgb(0xF2, 0xF3, 0xF5),
    highlight: Color32::from_rgb(0xE0, 0xA5, 0x26),
};

const NEUTRAL_EXTRA_DARK: Colors = Colors {
    canvas: Color32::from_rgb(0x08, 0x09, 0x0A),
    surface: Color32::from_rgb(0x0F, 0x10, 0x12),
    surface_high: Color32::from_rgb(0x14, 0x15, 0x18),
    input: Color32::from_rgb(0x0B, 0x0C, 0x0D),
    border: Color32::from_rgb(0x24, 0x26, 0x2A),
    text: Color32::from_rgb(0xD5, 0xD7, 0xDB),
    text_dim: Color32::from_rgb(0x8A, 0x8F, 0x99),
    accent: Color32::from_rgb(0x6F, 0x95, 0xDE),
    accent_soft: Color32::from_rgb(0x1B, 0x26, 0x40),
    hover: Color32::from_rgb(0x1A, 0x1C, 0x20),
    danger: Color32::from_rgb(0xD9, 0x6A, 0x60),
    brand: Color32::from_rgb(0x2B, 0x2F, 0x36),
    on_brand: Color32::from_rgb(0xF2, 0xF3, 0xF5),
    highlight: Color32::from_rgb(0xD0, 0x98, 0x22),
};

impl Default for TypeSizes {
    fn default() -> Self {
        TypeSizes { body: 12.5, caption: 11.0, title: 13.5, heading: 26.0 }
    }
}

impl Default for Spacing {
    fn default() -> Self {
        Spacing { item_x: 8.0, item_y: 3.0, button_x: 6.0, button_y: 3.0, panel_margin: 8.0, indent: 18.0 }
    }
}

impl Default for Radius {
    fn default() -> Self {
        Radius { control: 4, window: 8, card: 6, chip: 9 }
    }
}

impl Default for Tokens {
    /// The neutral token set: no product's colors, egui's own fonts.
    fn default() -> Self {
        Tokens {
            name: "neutral".into(),
            light: NEUTRAL_LIGHT,
            dark: NEUTRAL_DARK,
            extra_dark: NEUTRAL_EXTRA_DARK,
            fonts: FontNames::default(),
            type_sizes: TypeSizes::default(),
            spacing: Spacing::default(),
            radius: Radius::default(),
        }
    }
}

/// The file's shape (`TOKENS.md`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct RawTokens {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    /// Named colors the themes may refer to.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    palette: BTreeMap<String, String>,
    light: RawColors,
    #[serde(skip_serializing_if = "Option::is_none")]
    dark: Option<RawColors>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra_dark: Option<RawColors>,
    fonts: FontNames,
    #[serde(rename = "type")]
    type_sizes: TypeSizes,
    spacing: Spacing,
    radius: Radius,
}

/// A color as the file writes it: a palette name, else hex.
fn color(v: &str, palette: &BTreeMap<String, String>) -> Result<Color32, ColorError> {
    match palette.get(v.trim()) {
        Some(named) => parse_hex(named),
        None => parse_hex(v),
    }
}

fn bounded(v: f32, lo: f32, hi: f32, default: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { default }
}

impl TypeSizes {
    fn bounded(self) -> Self {
        let d = TypeSizes::default();
        TypeSizes {
            body: bounded(self.body, 6.0, 48.0, d.body),
            caption: bounded(self.caption, 6.0, 48.0, d.caption),
            title: bounded(self.title, 6.0, 72.0, d.title),
            heading: bounded(self.heading, 6.0, 96.0, d.heading),
        }
    }
}

impl Spacing {
    fn bounded(self) -> Self {
        let d = Spacing::default();
        Spacing {
            item_x: bounded(self.item_x, 0.0, 64.0, d.item_x),
            item_y: bounded(self.item_y, 0.0, 64.0, d.item_y),
            button_x: bounded(self.button_x, 0.0, 64.0, d.button_x),
            button_y: bounded(self.button_y, 0.0, 64.0, d.button_y),
            panel_margin: bounded(self.panel_margin, 0.0, 64.0, d.panel_margin),
            indent: bounded(self.indent, 0.0, 96.0, d.indent),
        }
    }
}

impl Radius {
    fn bounded(self) -> Self {
        Radius { control: self.control.min(32), window: self.window.min(32), card: self.card.min(32), chip: self.chip.min(32) }
    }
}

impl Tokens {
    /// Read a `tokens.json` file's text. A color left out of the light theme is the neutral
    /// set's; a dark theme left out is the neutral set's dark theme; an extra-dark theme (or any
    /// of its colors) left out is the dark theme's. Sizes left out are the neutral set's, and
    /// every size is kept within reason, so a damaged file cannot ask for something absurd.
    pub fn from_json(text: &str) -> Result<Tokens, TokensError> {
        let raw: RawTokens = serde_json::from_str(text)?;
        let neutral = Tokens::default();
        let light = raw.light.resolve("light", &neutral.light, &raw.palette)?;
        let dark = match &raw.dark {
            Some(d) => d.resolve("dark", &neutral.dark, &raw.palette)?,
            None => neutral.dark,
        };
        let extra_dark = match &raw.extra_dark {
            Some(d) => d.resolve("extra_dark", &dark, &raw.palette)?,
            None => dark,
        };
        Ok(Tokens {
            name: raw.name.unwrap_or_else(|| "tokens".into()),
            light,
            dark,
            extra_dark,
            fonts: raw.fonts,
            type_sizes: raw.type_sizes.bounded(),
            spacing: raw.spacing.bounded(),
            radius: raw.radius.bounded(),
        })
    }

    /// The tokens as a `tokens.json` file: every color written out as hex.
    pub fn to_json(&self) -> String {
        let raw = RawTokens {
            name: Some(self.name.clone()),
            palette: BTreeMap::new(),
            light: RawColors::of(&self.light),
            dark: Some(RawColors::of(&self.dark)),
            extra_dark: Some(RawColors::of(&self.extra_dark)),
            fonts: self.fonts.clone(),
            type_sizes: self.type_sizes,
            spacing: self.spacing,
            radius: self.radius,
        };
        serde_json::to_string_pretty(&raw).unwrap_or_default()
    }

    /// The colors of `mode`.
    pub fn colors(&self, mode: Mode) -> &Colors {
        match mode {
            Mode::Light => &self.light,
            Mode::Dark => &self.dark,
            Mode::ExtraDark => &self.extra_dark,
        }
    }

    /// The theme for `mode`.
    pub fn theme(&self, mode: Mode) -> Theme {
        let c = self.colors(mode);
        Theme {
            canvas: c.canvas,
            surface: c.surface,
            surface_high: c.surface_high,
            input: c.input,
            border: c.border,
            text: c.text,
            text_dim: c.text_dim,
            accent: c.accent,
            accent_soft: c.accent_soft,
            hover: c.hover,
            danger: c.danger,
            brand: c.brand,
            on_brand: c.on_brand,
            highlight: c.highlight,
            mode,
            dark: mode.is_dark(),
            type_sizes: self.type_sizes,
            spacing: self.spacing,
            radius: self.radius,
        }
    }
}

/// The theme in use: one mode's colors (the same roles as [`Colors`]) with the shared sizes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// Behind the document's pages.
    pub canvas: Color32,
    /// Ribbon, panels, status bar.
    pub surface: Color32,
    /// The quick-access row, the tab strip, windows and cards on a surface.
    pub surface_high: Color32,
    /// Fields and list rows.
    pub input: Color32,
    /// Lines between parts of the window, and around fields and cards.
    pub border: Color32,
    /// Text.
    pub text: Color32,
    /// Secondary text.
    pub text_dim: Color32,
    /// The current tab, controls that are on, links, focus.
    pub accent: Color32,
    /// The background of controls that are on.
    pub accent_soft: Color32,
    /// Hovered controls.
    pub hover: Color32,
    /// Problems and destructive actions.
    pub danger: Color32,
    /// The product's own color.
    pub brand: Color32,
    /// Text and icons on `brand`.
    pub on_brand: Color32,
    /// The mark on the current item.
    pub highlight: Color32,
    /// Which theme this is.
    pub mode: Mode,
    /// Whether it has dark surfaces.
    pub dark: bool,
    /// Type sizes.
    pub type_sizes: TypeSizes,
    /// Spacing.
    pub spacing: Spacing,
    /// Corner radii.
    pub radius: Radius,
}

impl Default for Theme {
    /// The neutral light theme.
    fn default() -> Self {
        Tokens::default().theme(Mode::Light)
    }
}

/// Where the theme in use is kept in the egui context.
fn theme_id() -> Id {
    Id::unique("jcj-theme-in-use")
}

impl Theme {
    /// The theme in use this frame (the neutral light theme until one is set).
    pub fn get(ctx: &Context) -> Theme {
        ctx.data(|d| d.get_temp::<Theme>(theme_id())).unwrap_or_default()
    }

    /// Put this theme in use for [`Theme::get`]. [`crate::apply`] also sets egui's visuals.
    pub fn set(self, ctx: &Context) {
        ctx.data_mut(|d| d.insert_temp(theme_id(), self));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NEUTRAL_FILE: &str = include_str!("../tokens/neutral.json");

    #[test]
    fn the_neutral_file_is_the_default_set() {
        let read = Tokens::from_json(NEUTRAL_FILE).unwrap();
        assert_eq!(read, Tokens::default());
    }

    #[test]
    fn tokens_round_trip_through_json() {
        let t = Tokens::default();
        assert_eq!(Tokens::from_json(&t.to_json()).unwrap(), t);
    }

    #[test]
    fn palette_names_and_fallbacks() {
        let json = r##"{
            "name": "sample",
            "palette": { "ink": "#102030", "paper": "#FAFAF0" },
            "light": { "text": "ink", "surface": "paper", "accent": "#3050A0" },
            "dark": { "surface": "ink" },
            "extra_dark": { "canvas": "#000000" },
            "fonts": { "regular": "Sample Sans" },
            "type": { "body": 13 },
            "radius": { "window": 200 }
        }"##;
        let t = Tokens::from_json(json).unwrap();
        let n = Tokens::default();
        assert_eq!(t.name, "sample");
        assert_eq!(t.light.text, Color32::from_rgb(0x10, 0x20, 0x30));
        assert_eq!(t.light.surface, Color32::from_rgb(0xFA, 0xFA, 0xF0));
        assert_eq!(t.light.border, n.light.border, "left out: the neutral color");
        assert_eq!(t.dark.surface, Color32::from_rgb(0x10, 0x20, 0x30));
        assert_eq!(t.dark.text, n.dark.text);
        assert_eq!(t.extra_dark.canvas, Color32::BLACK);
        assert_eq!(t.extra_dark.surface, t.dark.surface, "extra dark falls back to dark");
        assert_eq!(t.fonts.regular.as_deref(), Some("Sample Sans"));
        assert_eq!(t.type_sizes.body, 13.0);
        assert_eq!(t.type_sizes.caption, n.type_sizes.caption);
        assert_eq!(t.radius.window, 32, "bounded");
    }

    #[test]
    fn bad_colors_say_where_they_are() {
        let err = Tokens::from_json(r#"{"dark": {"accent": "blue"}}"#).unwrap_err();
        assert!(err.to_string().starts_with("dark.accent:"), "{err}");
        assert!(Tokens::from_json("{").is_err());
        // An empty object is the neutral set.
        assert_eq!(Tokens::from_json("{}").unwrap().light, Tokens::default().light);
    }

    #[test]
    fn modes_pick_their_colors() {
        let t = Tokens::default();
        assert_eq!(Mode::of(true, true), Mode::ExtraDark);
        assert_eq!(Mode::of(false, true), Mode::Light);
        let th = t.theme(Mode::Dark);
        assert!(th.dark);
        assert_eq!(th.surface, t.dark.surface);
        let ctx = Context::default();
        assert_eq!(Theme::get(&ctx), Theme::default());
        th.set(&ctx);
        assert_eq!(Theme::get(&ctx), th);
    }
}
