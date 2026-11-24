// SPDX-License-Identifier: MIT OR Apache-2.0
//! Emphasis as text markers: `**` bold, `*` italic, `_` underline, and `\` escapes for the marker
//! characters themselves. For places that keep one paragraph of formatted text as plain text
//! (a card's title, a list in app data, a line an agent writes).
//!
//! Each marker toggles its own style, which keeps encoding and decoding simple and exact:
//! `decode(&encode(p), false)` gives `p`'s text with its bold, italic and underline back. Other
//! formatting is not kept, and inline objects (pictures, fields) are left out.
//!
//! ```
//! use jcj_interchange::emphasis::{decode, encode};
//!
//! let p = decode("plain **bold** *it* _un_ \\*star", false);
//! assert_eq!(p.text, "plain bold it un *star");
//! assert_eq!(encode(&p), "plain **bold** *it* _un_ \\*star");
//! ```

use jcj_doc::Paragraph;
use jcj_doc::para::OBJ;
use jcj_doc::props::{CharProps, Underline};

/// Emphasis markers that take the styles from `from` to `to` (each marker toggles its own).
fn toggles(out: &mut String, from: (bool, bool, bool), to: (bool, bool, bool)) {
    if from.0 != to.0 {
        out.push_str("**");
    }
    if from.1 != to.1 {
        out.push('*');
    }
    if from.2 != to.2 {
        out.push('_');
    }
}

/// A paragraph's text with emphasis markers and escapes (runs → `**`, `*`, `_`).
pub fn encode(p: &Paragraph) -> String {
    const PLAIN: (bool, bool, bool) = (false, false, false);
    let mut out = String::with_capacity(p.text.len() + 8);
    let mut style = PLAIN;
    for (range, props) in p.run_ranges() {
        let next = (props.bold == Some(true), props.italic == Some(true), props.underline.is_some_and(|x| x != Underline::None));
        toggles(&mut out, style, next);
        style = next;
        for c in p.text.get(range).unwrap_or("").chars() {
            match c {
                '\\' | '*' | '_' => {
                    out.push('\\');
                    out.push(c);
                }
                OBJ => {}
                _ => out.push(c),
            }
        }
    }
    toggles(&mut out, style, PLAIN);
    out
}

/// Which markers are read as plain characters (bold, italic, underline).
type Literal = [bool; 3];

/// The emphasis being read.
#[derive(Clone, Copy, Default)]
struct State {
    b: bool,
    i: bool,
    u: bool,
}

impl State {
    fn props(self) -> CharProps {
        CharProps {
            bold: self.b.then_some(true),
            italic: self.i.then_some(true),
            underline: self.u.then_some(Underline::Single),
            ..CharProps::default()
        }
    }
}

fn flush(p: &mut Paragraph, piece: &mut String, st: State) {
    if piece.is_empty() {
        return;
    }
    let at = p.len();
    let _ = p.insert_text(at, piece, &st.props());
    piece.clear();
}

/// One decoding pass; also returns which styles were still open at the end.
fn decode_pass(text: &str, literal: Literal) -> (Paragraph, Literal) {
    let mut p = Paragraph::new();
    let mut st = State::default();
    let mut piece = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(n) = chars.next() {
                    piece.push(n);
                }
            }
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                if literal[0] {
                    piece.push_str("**");
                } else {
                    flush(&mut p, &mut piece, st);
                    st.b = !st.b;
                }
            }
            '*' if literal[1] => piece.push('*'),
            '*' => {
                flush(&mut p, &mut piece, st);
                st.i = !st.i;
            }
            '_' if literal[2] => piece.push('_'),
            '_' => {
                flush(&mut p, &mut piece, st);
                st.u = !st.u;
            }
            _ => piece.push(c),
        }
    }
    flush(&mut p, &mut piece, st);
    (p, [st.b, st.i, st.u])
}

/// A paragraph from marked-up text: markers toggle bold, italic and underline. When `lenient`,
/// a marker kind left unclosed at the end is read as plain characters instead ("5 * 3" is not
/// emphasis), as text typed by people needs.
pub fn decode(text: &str, lenient: bool) -> Paragraph {
    let mut literal = [false; 3];
    loop {
        let (p, open) = decode_pass(text, literal);
        if !lenient || !open.iter().any(|o| *o) {
            return p;
        }
        for (l, o) in literal.iter_mut().zip(open) {
            *l |= o;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn styled(p: &Paragraph) -> Vec<(String, bool, bool, bool)> {
        p.run_ranges()
            .map(|(r, c)| {
                (
                    p.text.get(r).unwrap_or("").to_string(),
                    c.bold == Some(true),
                    c.italic == Some(true),
                    c.underline.is_some_and(|x| x != Underline::None),
                )
            })
            .collect()
    }

    #[test]
    fn markers_round_trip() {
        let p = decode(r"plain **bold** *it* _un_ ***both*** \*star\_ \\", false);
        assert_eq!(p.text, r"plain bold it un both *star_ \");
        let again = decode(&encode(&p), false);
        assert_eq!(styled(&again), styled(&p));
        assert_eq!(again.text, p.text);
    }

    #[test]
    fn lenient_reads_unclosed_markers_as_text() {
        assert_eq!(decode("5 * 3 is *fifteen*", true).text, "5 * 3 is *fifteen*", "an odd count: all literal");
        assert_eq!(decode("5 * 3", true).text, "5 * 3");
        let p = decode("snake_case and **bold**", true);
        assert_eq!(p.text, "snake_case and bold");
        assert!(styled(&p).iter().any(|s| s.0 == "bold" && s.1));
        assert_eq!(decode("a ** b", true).text, "a ** b");
        assert_eq!(decode("a * b", false).text, "a  b", "strict mode toggles");
    }

    #[test]
    fn objects_are_left_out_and_hostile_text_is_fine() {
        let p = Paragraph::with_text(&format!("a{OBJ}b"), CharProps::default());
        assert_eq!(encode(&p), "ab");
        let _ = decode(&"*_".repeat(10_000), true);
        assert_eq!(decode("trailing \\", false).text, "trailing ");
    }
}
