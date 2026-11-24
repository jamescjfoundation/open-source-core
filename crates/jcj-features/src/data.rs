// SPDX-License-Identifier: MIT OR Apache-2.0
//! Settings groups kept in the document's extension data, and small helpers the features share.
//!
//! A settings group (the watermark, the header and footer, the revision sets) is one JSON value
//! under one `core.` key of the document's [`Ext`](jcj_doc::Ext). A value that does not read as
//! the group gives the group's defaults, so a hostile or damaged file never fails to open; each
//! group then bounds what it read (texts cut to their limits, numbers clamped).

use jcj_doc::props::Rgb;
use jcj_doc::{Document, ExtKey, HasExt, Path, Pos, StoryRef};
use jcj_engine::CmdError;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// The group stored under `key`, or its defaults when unset or unreadable.
pub fn load<T: Serialize + DeserializeOwned + Default>(doc: &Document, key: ExtKey<T>) -> T {
    key.get_or_none(doc).unwrap_or_default()
}

/// Store `value` under `key` (a value that serializes to `null` removes the key).
pub fn store<T: Serialize + DeserializeOwned>(doc: &mut Document, key: ExtKey<T>, value: &T) -> Result<(), CmdError> {
    key.set(doc, value).map_err(|e| CmdError::Failed(e.to_string()))
}

/// Set `key` on `on`, ignoring the error a valid `core.` key cannot give. For closures that
/// cannot return one (`Document::format_range`).
pub(crate) fn put<T: Serialize + DeserializeOwned>(key: ExtKey<T>, on: &mut impl HasExt, value: &T) {
    let _ = key.set(on, value);
}

/// A position in top-level body paragraph `block`.
pub fn body_pos(block: usize, off: usize) -> Pos {
    Pos { story: StoryRef::Body, path: Path::top(block), off }
}

/// A position in the body paragraph at `path`.
pub fn pos_at(path: &Path, off: usize) -> Pos {
    Pos { story: StoryRef::Body, path: path.clone(), off }
}

/// Text cut to `max` characters, with control characters dropped and the ends trimmed.
pub fn clean_line(text: &str, max: usize) -> String {
    text.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

/// `#RRGGBB` as a color, if it is one.
pub fn parse_color(s: &str) -> Option<Rgb> {
    let h = s.trim().strip_prefix('#')?;
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    let [_, r, g, b] = v.to_be_bytes();
    Some(Rgb(r, g, b))
}

/// A color as `#RRGGBB`.
pub fn color_hex(c: Rgb) -> String {
    format!("#{:02X}{:02X}{:02X}", c.0, c.1, c.2)
}

/// The id after the largest of `ids` (1 when there is none).
pub(crate) fn next_id(ids: impl Iterator<Item = u32>) -> u32 {
    ids.max().map_or(1, |k| k.saturating_add(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    struct Group {
        on: bool,
        size: u32,
    }

    const GROUP: ExtKey<Group> = ExtKey::new("test.group");

    #[test]
    fn bad_data_reads_as_the_defaults() {
        let mut doc = Document::new();
        assert_eq!(load(&doc, GROUP), Group::default());
        store(&mut doc, GROUP, &Group { on: true, size: 3 }).unwrap();
        assert_eq!(load(&doc, GROUP), Group { on: true, size: 3 });
        doc.ext.set("test.group", json!({"on": "yes"})).unwrap();
        assert_eq!(load(&doc, GROUP), Group::default());
        doc.ext.set("test.group", json!({"size": 4})).unwrap();
        assert_eq!(load(&doc, GROUP), Group { on: false, size: 4 }, "missing fields take their defaults");
    }

    #[test]
    fn colors_and_lines() {
        assert_eq!(parse_color("#ff8000"), Some(Rgb(255, 128, 0)));
        assert_eq!(parse_color(" #A9C8F2 "), Some(Rgb(0xA9, 0xC8, 0xF2)));
        assert_eq!(parse_color("#+12345"), None);
        assert_eq!(parse_color("blue"), None);
        assert_eq!(color_hex(Rgb(1, 2, 255)), "#0102FF");
        assert_eq!(clean_line(" a\u{0}b\tc ", 3), "ab");
        assert_eq!(next_id([3, 7, 1].into_iter()), 8);
        assert_eq!(next_id(std::iter::empty()), 1);
    }
}
