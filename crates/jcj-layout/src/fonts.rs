// SPDX-License-Identifier: MIT OR Apache-2.0
//! Fonts: the bundled set every app can rely on, and adding fonts at run time.
//!
//! Layout finds fonts in one process-wide database: the bundled fonts first, then fonts added at
//! run time, then the fonts installed on the system (cataloged the first time a family that is
//! not loaded is asked for). Text set in a family that is nowhere to be found is drawn in
//! [`FALLBACK_FAMILY`].
//!
//! **Output that must be the same on every machine** (golden files, contract tests) uses only
//! the [`BUNDLED_FAMILIES`] and turns the system fallback off ([`set_system_fallback`]): a family
//! that is loaded is never looked up on the system, so installed fonts cannot change it.

use jcj_upstream::fonts::FontDb;

/// The font families built into every app (all under the SIL Open Font License). These are
/// always available, whatever is installed.
pub const BUNDLED_FAMILIES: &[&str] = &["Source Sans 3", "Source Serif 4", "Inter", "JetBrains Mono"];

/// The family text is drawn in when its own font is not available.
pub const FALLBACK_FAMILY: &str = jcj_upstream::fonts::FALLBACK_FAMILY;

/// Why a font file could not be added.
pub use jcj_upstream::fonts::UserFontError;

fn db() -> &'static FontDb {
    FontDb::global()
}

/// Is a face of `family` loaded (bundled or added at run time)? Never scans the system.
pub fn is_loaded(family: &str) -> bool {
    db().is_loaded(family)
}

/// Is `family` available: loaded, or installed on the system (scans the system fonts the first
/// time it is asked about a family that is not loaded).
pub fn has_family(family: &str) -> bool {
    db().has_family(family)
}

/// Every family name available (loaded and installed), sorted. Scans the system fonts.
pub fn families() -> Vec<String> {
    db().families()
}

/// Add a font file's faces (TTF, OTF or TTC bytes) for this run, in memory only. Returns how many
/// faces were added: 0 when the file could not be read or its faces were already there (a
/// bundled face is never replaced).
pub fn register_font(bytes: Vec<u8>) -> usize {
    db().add_font(bytes)
}

/// Add a font file (TTF, OTF, TTC, WOFF or WOFF2) the user chose, by its file name. Returns the
/// family names it added. Kept for this run only unless the host stores user fonts in a folder.
pub fn install_user_font(file_name: &str, bytes: &[u8]) -> Result<Vec<String>, UserFontError> {
    db().install_font(file_name, bytes).map(|i| i.font.families())
}

/// Draw characters the loaded fonts lack in an installed system font that has them (on by
/// default). Turn it off for output that must not depend on the machine.
pub fn set_system_fallback(on: bool) {
    db().set_system_fallback(on);
}

/// Start cataloging the installed system fonts on a background thread, so the first lookup of a
/// family that is not loaded does not wait for the scan. Call once at app start.
pub fn scan_system_fonts_in_background() {
    db().scan_in_background();
}

/// A number that changes whenever fonts are added or removed at run time; a layout cached
/// with an older number is redone.
pub fn generation() -> u64 {
    db().generation()
}

/// A font face built into the core: WordCraft's bundled set, every face under the SIL Open Font
/// License 1.1 (Source Sans 3, Source Serif 4, Inter, JetBrains Mono). Apps may use them for the
/// interface too (`jcj-theme` does by default).
#[derive(Clone, Copy, Debug)]
pub struct BundledFace {
    /// The family name ("Inter").
    pub family: &'static str,
    /// The style name ("Regular", "Medium").
    pub style: &'static str,
    /// The font file.
    pub bytes: &'static [u8],
}

/// The bundled faces' names, in the order WordCraft bundles them.
const BUNDLED_NAMES: [(&str, &str); 13] = [
    ("Source Sans 3", "Regular"),
    ("Source Sans 3", "Semibold"),
    ("Source Sans 3", "Bold"),
    ("Source Sans 3", "Italic"),
    ("Source Serif 4", "Regular"),
    ("Source Serif 4", "Italic"),
    ("Source Serif 4", "Semibold"),
    ("Source Serif 4", "Bold"),
    ("Source Serif 4", "Bold Italic"),
    ("Inter", "Regular"),
    ("Inter", "Medium"),
    ("Inter", "SemiBold"),
    ("JetBrains Mono", "Regular"),
];

/// Every bundled face, with its family and style.
pub fn bundled_faces() -> Vec<BundledFace> {
    jcj_upstream::fonts::bundled().iter().zip(BUNDLED_NAMES).map(|(bytes, (family, style))| BundledFace { family, style, bytes }).collect()
}

/// One bundled face by family and style.
pub fn bundled_face(family: &str, style: &str) -> Option<BundledFace> {
    bundled_faces().into_iter().find(|f| f.family == family && f.style == style)
}

#[cfg(test)]
mod tests {
    #[test]
    fn bundled_faces_are_named_in_order() {
        let faces = super::bundled_faces();
        assert_eq!(faces.len(), 13, "WordCraft changed its bundled set: update BUNDLED_NAMES");
        let inter = super::bundled_face("Inter", "Regular").unwrap();
        assert!(!inter.bytes.is_empty());
    }

    use super::*;

    #[test]
    fn the_bundled_families_are_always_loaded() {
        for f in BUNDLED_FAMILIES {
            assert!(is_loaded(f), "{f} is not loaded");
        }
        assert!(is_loaded(FALLBACK_FAMILY));
    }

    #[test]
    fn a_file_that_is_not_a_font_adds_nothing() {
        assert_eq!(register_font(b"not a font".to_vec()), 0);
        assert!(install_user_font("bad.ttf", b"not a font").is_err());
    }
}
