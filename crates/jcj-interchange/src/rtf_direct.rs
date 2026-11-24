// SPDX-License-Identifier: MIT OR Apache-2.0
//! RTF with every paragraph's and run's formatting written out directly (see
//! [`crate::export_rtf_direct`]).

use std::fmt::Write as _;

use jcj_doc::props::{Align, Underline};
use jcj_doc::{Document, walk};

/// Points to twips (1/20 point), rounded; values that are not finite become 0.
fn twips(points: f32) -> i64 {
    if points.is_finite() { (f64::from(points) * 20.0).round() as i64 } else { 0 }
}

/// RTF text for `s`: `\`, `{` and `}` escaped, other characters outside printable ASCII as
/// `\uN?`, a line break as `\line`, a tab as `\tab`. Objects and other control characters are
/// dropped.
fn push_text(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '\n' => out.push_str("\\line "),
            '\t' => out.push_str("\\tab "),
            ' '..='~' => out.push(c),
            c if c.is_control() || c == jcj_doc::para::OBJ => {}
            c => {
                let mut units = [0u16; 2];
                for u in c.encode_utf16(&mut units) {
                    // RTF writes 16-bit units as signed numbers.
                    let _ = write!(out, "\\u{}?", *u as i16);
                }
            }
        }
    }
}

/// The font table: each font once, in the order first used.
#[derive(Default)]
struct Fonts(Vec<String>);

impl Fonts {
    fn index(&mut self, name: &str) -> usize {
        if let Some(i) = self.0.iter().position(|f| f == name) {
            return i;
        }
        self.0.push(name.to_owned());
        self.0.len() - 1
    }
}

pub(crate) fn write(doc: &Document) -> String {
    let mut fonts = Fonts::default();
    let mut body = String::new();
    let paragraphs = walk::body_paragraphs(doc);
    for (n, (_, p)) in paragraphs.iter().enumerate() {
        if n > 0 {
            body.push_str("\\par\n");
        }
        let rp = doc.styles.resolve_para(&p.props);
        body.push_str("\\pard\\plain");
        body.push_str(match rp.align {
            Align::Center => "\\qc",
            Align::Right => "\\qr",
            Align::Justify | Align::Distribute => "\\qj",
            Align::Left => "\\ql",
        });
        let _ = write!(
            body,
            "\\li{}\\ri{}\\fi{}\\sb{}\\sa{}",
            twips(rp.indent_left),
            twips(rp.indent_right),
            twips(rp.indent_first),
            twips(rp.space_before),
            twips(rp.space_after)
        );
        if rp.page_break_before {
            body.push_str("\\pagebb");
        }
        let style = p.props.style.as_deref();
        let mut covered = 0;
        for (range, props) in p.run_ranges() {
            let Some(text) = p.text.get(range.clone()) else { continue };
            covered = range.end;
            if text.is_empty() {
                continue;
            }
            let rc = doc.styles.resolve_char(style, props);
            let _ = write!(body, "\\plain\\f{}\\fs{}", fonts.index(&rc.font), (f64::from(rc.size) * 2.0).round() as i64);
            if rc.bold {
                body.push_str("\\b");
            }
            if rc.italic {
                body.push_str("\\i");
            }
            if rc.underline != Underline::None {
                body.push_str("\\ul");
            }
            body.push(' ');
            push_text(&mut body, text);
        }
        // Text the runs do not cover takes the paragraph mark's formatting.
        if let Some(rest) = p.text.get(covered..).filter(|t| !t.is_empty()) {
            let rc = doc.styles.resolve_char(style, &p.mark);
            let _ = write!(body, "\\plain\\f{}\\fs{} ", fonts.index(&rc.font), (f64::from(rc.size) * 2.0).round() as i64);
            push_text(&mut body, rest);
        }
    }
    if fonts.0.is_empty() {
        fonts.index(&doc.styles.resolve_char(None, &jcj_doc::CharProps::default()).font);
    }

    let s = &doc.last_section;
    let mut out = String::from("{\\rtf1\\ansi\\ansicpg1252\\deff0\\uc1\n{\\fonttbl");
    for (i, name) in fonts.0.iter().enumerate() {
        let _ = write!(out, "{{\\f{i}\\fnil ");
        push_text(&mut out, name);
        out.push_str(";}");
    }
    out.push_str("}\n");
    let _ = writeln!(
        out,
        "\\paperw{}\\paperh{}\\margl{}\\margr{}\\margt{}\\margb{}",
        twips(s.page_w),
        twips(s.page_h),
        twips(s.margin_left),
        twips(s.margin_right),
        twips(s.margin_top),
        twips(s.margin_bottom)
    );
    out.push_str(&body);
    out.push_str("\\par\n}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::push_text;

    #[test]
    fn text_is_escaped() {
        let mut s = String::new();
        push_text(&mut s, "a\\{b}\né\u{1F600}\u{FFFC}\u{7}");
        assert_eq!(s, "a\\\\\\{b\\}\\line \\u233?\\u-10179?\\u-8704?");
    }
}
