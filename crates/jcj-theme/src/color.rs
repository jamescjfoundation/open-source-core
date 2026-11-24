// SPDX-License-Identifier: MIT OR Apache-2.0
//! Colors written as `#RRGGBB` or `#RRGGBBAA`.

use egui::Color32;

/// Why a color could not be read.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ColorError {
    /// Not `#RRGGBB` or `#RRGGBBAA`.
    #[error("`{0}` is not a color: write #RRGGBB or #RRGGBBAA")]
    Format(String),
}

/// `#RRGGBB` or `#RRGGBBAA` (the `#` may be left out; case does not matter) as a color. The
/// alpha is straight, not premultiplied, as designers write it.
pub fn parse_hex(s: &str) -> Result<Color32, ColorError> {
    let bad = || ColorError::Format(s.to_string());
    let h = s.trim();
    let h = h.strip_prefix('#').unwrap_or(h);
    if !(h.len() == 6 || h.len() == 8) || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(bad());
    }
    let byte = |i: usize| h.get(i..i + 2).and_then(|x| u8::from_str_radix(x, 16).ok()).ok_or_else(bad);
    let (r, g, b) = (byte(0)?, byte(2)?, byte(4)?);
    let a = if h.len() == 8 { byte(6)? } else { 255 };
    Ok(Color32::from_rgba_unmultiplied(r, g, b, a))
}

/// A color as `#RRGGBB`, or `#RRGGBBAA` when it is not opaque.
pub fn hex(c: Color32) -> String {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    if a == 255 { format!("#{r:02X}{g:02X}{b:02X}") } else { format!("#{r:02X}{g:02X}{b:02X}{a:02X}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_writes_hex_colors() {
        assert_eq!(parse_hex("#1C2340"), Ok(Color32::from_rgb(0x1C, 0x23, 0x40)));
        assert_eq!(parse_hex("fafaf7"), Ok(Color32::from_rgb(0xFA, 0xFA, 0xF7)));
        assert_eq!(parse_hex(" #ffffff80 ").map(|c| c.a()), Ok(128));
        for bad in ["", "#", "#12345", "#1234567", "#GGGGGG", "red", "#12345678ab"] {
            assert!(parse_hex(bad).is_err(), "{bad}");
        }
        assert_eq!(hex(Color32::from_rgb(0x1C, 0x23, 0x40)), "#1C2340");
        let half = Color32::from_rgba_unmultiplied(255, 255, 255, 128);
        assert_eq!(parse_hex(&hex(half)), Ok(half));
    }
}
