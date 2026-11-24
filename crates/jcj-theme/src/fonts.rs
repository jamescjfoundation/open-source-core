// SPDX-License-Identifier: MIT OR Apache-2.0
//! The interface's fonts. The app supplies each face's bytes under the name its tokens give
//! ([`crate::FontNames`]); the theme builds egui's font definitions from them: text in the
//! proportional family, a monospace family, and two heavier families (`medium` and `semibold`)
//! for tab names, button labels and titles. A weight the app does not supply falls back to the
//! regular face, and every family ends with egui's own fonts when they are built in.

use std::borrow::Cow;
use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily, FontId};

use crate::tokens::FontNames;

/// The family name of the medium weight.
pub const FAMILY_MEDIUM: &str = "medium";
/// The family name of the semibold weight.
pub const FAMILY_SEMIBOLD: &str = "semibold";

/// A font face the app supplies: its name (as the tokens refer to it) and its TrueType or
/// OpenType bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontFace {
    /// The name the tokens use for it.
    pub name: String,
    /// The font file's bytes.
    pub bytes: Cow<'static, [u8]>,
}

impl FontFace {
    /// A face from bytes built into the app.
    pub fn from_static(name: impl Into<String>, bytes: &'static [u8]) -> Self {
        FontFace { name: name.into(), bytes: Cow::Borrowed(bytes) }
    }
}

/// egui's font definitions for `names`, with the faces in `faces`. Faces named in `names` but
/// not supplied are skipped.
pub fn font_definitions(names: &FontNames, faces: &[FontFace]) -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    // egui's own fonts, if built in, go last in every family.
    let builtin_prop = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let builtin_mono = fonts.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();
    let mut have = |name: &Option<String>| -> Option<String> {
        let name = name.as_ref()?;
        let face = faces.iter().find(|f| &f.name == name)?;
        let data = match &face.bytes {
            Cow::Borrowed(b) => FontData::from_static(b),
            Cow::Owned(b) => FontData::from_owned(b.clone()),
        };
        fonts.font_data.insert(name.clone(), Arc::new(data));
        Some(name.clone())
    };
    let regular = have(&names.regular);
    let mono = have(&names.mono);
    let medium = have(&names.medium);
    let semibold = have(&names.semibold);
    let chain = |first: &[&Option<String>], rest: &[String]| -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for n in first.iter().filter_map(|n| n.as_ref()) {
            if !v.contains(n) {
                v.push(n.clone());
            }
        }
        for n in rest {
            if !v.contains(n) {
                v.push(n.clone());
            }
        }
        v
    };
    fonts.families.insert(FontFamily::Proportional, chain(&[&regular, &mono], &builtin_prop));
    fonts.families.insert(FontFamily::Monospace, chain(&[&mono, &regular], &builtin_mono));
    fonts.families.insert(FontFamily::Name(FAMILY_MEDIUM.into()), chain(&[&medium, &regular, &mono], &builtin_prop));
    fonts.families.insert(FontFamily::Name(FAMILY_SEMIBOLD.into()), chain(&[&semibold, &medium, &regular, &mono], &builtin_prop));
    fonts
}

/// Install the interface fonts in `ctx`.
pub fn install_fonts(ctx: &egui::Context, names: &FontNames, faces: &[FontFace]) {
    ctx.set_fonts(font_definitions(names, faces));
}

/// Text at `size` in the regular weight.
pub fn regular(size: f32) -> FontId {
    FontId::proportional(size)
}

/// Text at `size` in the medium weight (installed by [`install_fonts`]).
pub fn medium(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(FAMILY_MEDIUM.into()))
}

/// Text at `size` in the semibold weight (installed by [`install_fonts`]).
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(FAMILY_SEMIBOLD.into()))
}

/// Monospace text at `size`.
pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn families_fall_back_to_regular_and_skip_missing_faces() {
        let names = FontNames { regular: Some("Text".into()), medium: Some("Text Medium".into()), semibold: None, mono: Some("Absent".into()) };
        // Any bytes do for building the definitions; egui reads them when it lays out text.
        let faces = [FontFace::from_static("Text", b"not a real font"), FontFace::from_static("Text Medium", b"x")];
        let d = font_definitions(&names, &faces);
        assert!(d.font_data.contains_key("Text") && d.font_data.contains_key("Text Medium"));
        assert!(!d.font_data.contains_key("Absent"));
        let fam = |f: FontFamily| d.families.get(&f).cloned().unwrap_or_default();
        assert_eq!(fam(FontFamily::Proportional).first().map(String::as_str), Some("Text"));
        assert_eq!(fam(FontFamily::Name(FAMILY_MEDIUM.into()))[..2], ["Text Medium".to_string(), "Text".to_string()]);
        assert_eq!(fam(FontFamily::Name(FAMILY_SEMIBOLD.into())).first().map(String::as_str), Some("Text Medium"));
        assert_eq!(fam(FontFamily::Monospace).first().map(String::as_str), Some("Text"), "no mono face: the regular one");
    }

    #[test]
    fn no_faces_keeps_egui_s_own_fonts() {
        let d = font_definitions(&FontNames::default(), &[]);
        let builtin = FontDefinitions::default();
        assert_eq!(d.families.get(&FontFamily::Proportional), builtin.families.get(&FontFamily::Proportional));
        assert!(d.families.contains_key(&FontFamily::Name(FAMILY_SEMIBOLD.into())));
    }
}
