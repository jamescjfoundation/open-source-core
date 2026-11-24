// SPDX-License-Identifier: MIT OR Apache-2.0
//! The interface's fonts. The app supplies each face's bytes under the name its tokens give
//! ([`crate::FontNames`]); the theme builds egui's font definitions from them: text in the
//! proportional family, a monospace family, and two heavier families (`medium` and `semibold`)
//! for tab names, button labels and titles. A weight the app does not supply falls back to the
//! regular face; a role the product leaves open takes the core's default face (`default_faces`).

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
    let (names, faces) = with_defaults(names, faces);
    let (names, faces) = (&names, faces.as_slice());
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

/// The core's default interface faces: Inter (regular, medium, semibold) and JetBrains Mono, from
/// the fonts bundled with WordCraft (SIL Open Font License 1.1; `jcj_layout::fonts`). egui's own
/// fonts are not built in (their Ubuntu Font Licence is outside the allowlist, decision 0001), so
/// these are what a product gets when its tokens name no fonts.
pub fn default_faces() -> Vec<FontFace> {
    DEFAULT_FACES
        .iter()
        .filter_map(|(name, family, style)| jcj_layout::fonts::bundled_face(family, style).map(|f| FontFace::from_static(*name, f.bytes)))
        .collect()
}

/// (name in the tokens, bundled family, style) of [`default_faces`].
const DEFAULT_FACES: [(&str, &str, &str); 4] = [
    ("Inter", "Inter", "Regular"),
    ("Inter Medium", "Inter", "Medium"),
    ("Inter SemiBold", "Inter", "SemiBold"),
    ("JetBrains Mono", "JetBrains Mono", "Regular"),
];

/// `names` and `faces` with the default faces filling every role the product leaves open (a role
/// not named, or named but not supplied).
pub fn with_defaults(names: &FontNames, faces: &[FontFace]) -> (FontNames, Vec<FontFace>) {
    let mut faces = faces.to_vec();
    let mut names = names.clone();
    let defaults = default_faces();
    let mut fill = |role: &mut Option<String>, default: &str| {
        let supplied = role.as_ref().is_some_and(|n| faces.iter().any(|f| &f.name == n));
        if !supplied {
            *role = Some(default.to_owned());
            if !faces.iter().any(|f| f.name == default)
                && let Some(face) = defaults.iter().find(|f| f.name == default)
            {
                faces.push(face.clone());
            }
        }
    };
    fill(&mut names.regular, "Inter");
    fill(&mut names.medium, "Inter Medium");
    fill(&mut names.semibold, "Inter SemiBold");
    fill(&mut names.mono, "JetBrains Mono");
    (names, faces)
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
        // Roles left open, or named without a face, take the core's defaults.
        assert_eq!(fam(FontFamily::Name(FAMILY_SEMIBOLD.into())).first().map(String::as_str), Some("Inter SemiBold"));
        assert_eq!(fam(FontFamily::Monospace).first().map(String::as_str), Some("JetBrains Mono"), "a missing mono face: the default");
    }

    #[test]
    fn no_faces_gives_the_bundled_defaults() {
        let d = font_definitions(&FontNames::default(), &[]);
        let fam = |f: FontFamily| d.families.get(&f).cloned().unwrap_or_default();
        assert_eq!(fam(FontFamily::Proportional).first().map(String::as_str), Some("Inter"));
        assert_eq!(fam(FontFamily::Monospace).first().map(String::as_str), Some("JetBrains Mono"));
        assert!(d.font_data.contains_key("Inter Medium") && d.font_data.contains_key("Inter SemiBold"));
    }
}
