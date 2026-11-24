// SPDX-License-Identifier: MIT OR Apache-2.0
//! Marks on text: run extension values (a note's id, a revision set, a page lock) that move with
//! the characters they are on.

use jcj_doc::walk::body_paragraphs;
use jcj_doc::{CharProps, Document, ExtKey, HasExt, Path, Pos, StoryRef};
use jcj_engine::CmdError;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::data::put;

/// Put mark `key` = `value` on the text `a..b`. Unlike [`Document::format_range`] alone, the
/// paragraph marks stay unmarked, so paragraphs made later by Enter do not carry the mark into
/// text typed in them.
pub fn mark_range<T: Serialize + DeserializeOwned + Clone>(doc: &mut Document, a: &Pos, b: &Pos, key: ExtKey<T>, value: &T) -> Result<(), CmdError> {
    let v = value.clone();
    doc.format_range(a, b, &move |c| put(key, c, &v))?;
    unmark_paragraph_marks(doc, a, b, key)
}

/// Remove mark `key` from the text `a..b` where `drop` says so, paragraph marks included.
pub fn unmark_range<T: Serialize + DeserializeOwned>(
    doc: &mut Document,
    a: &Pos,
    b: &Pos,
    key: ExtKey<T>,
    drop: &dyn Fn(&T) -> bool,
) -> Result<(), CmdError> {
    doc.format_range(a, b, &|c| {
        if key.get_or_none(c).is_some_and(|v| drop(&v)) {
            key.remove(c);
        }
    })?;
    for path in doc.para_paths_in(a, b) {
        let Ok(p) = doc.para_mut(a.story, &path) else { continue };
        if key.get_or_none(&p.mark).is_some_and(|v| drop(&v)) {
            key.remove(&mut p.mark);
            p.touch();
        }
    }
    Ok(())
}

fn unmark_paragraph_marks<T: Serialize + DeserializeOwned>(doc: &mut Document, a: &Pos, b: &Pos, key: ExtKey<T>) -> Result<(), CmdError> {
    for path in doc.para_paths_in(a, b) {
        let p = doc.para_mut(a.story, &path)?;
        if key.is_set(&p.mark) {
            key.remove(&mut p.mark);
            p.touch();
        }
    }
    Ok(())
}

/// Remove mark `key` wherever `drop` says so, in every body paragraph (tables included) and its
/// paragraph mark.
pub fn unmark_everywhere<T: Serialize + DeserializeOwned>(doc: &mut Document, key: ExtKey<T>, drop: &dyn Fn(&T) -> bool) -> Result<(), CmdError> {
    let paths: Vec<Path> = body_paragraphs(doc)
        .into_iter()
        .filter(|(_, p)| {
            key.get_or_none(&p.mark).is_some_and(|v| drop(&v)) || p.run_ranges().any(|(_, c)| key.get_or_none(c).is_some_and(|v| drop(&v)))
        })
        .map(|(path, _)| path)
        .collect();
    for path in paths {
        let p = doc.para_mut(StoryRef::Body, &path)?;
        let len = p.len();
        p.format(0, len, &|c| {
            if key.get_or_none(c).is_some_and(|v| drop(&v)) {
                key.remove(c);
            }
        })?;
        if key.get_or_none(&p.mark).is_some_and(|v| drop(&v)) {
            key.remove(&mut p.mark);
        }
        p.touch();
    }
    Ok(())
}

/// The value of mark `key` on `c`.
pub fn mark<T: Serialize + DeserializeOwned>(key: ExtKey<T>, c: &CharProps) -> Option<T> {
    key.get_or_none(c)
}

/// Whether `c` carries mark `key`.
pub fn has_mark<T: Serialize + DeserializeOwned>(key: ExtKey<T>, c: &impl HasExt) -> bool {
    key.is_set(c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::body_pos;
    use jcj_doc::{Paragraph, para_block};

    const M: ExtKey<u32> = ExtKey::new("test.mark");

    fn doc() -> Document {
        let mut d = Document::new();
        d.body = ["One two.", "Three."].iter().map(|t| para_block(Paragraph::with_text(t, CharProps::default()))).collect();
        d
    }

    #[test]
    fn marks_leave_paragraph_marks_alone() {
        let mut d = doc();
        mark_range(&mut d, &body_pos(0, 0), &body_pos(1, 6), M, &4).unwrap();
        let p = d.body[0].as_para().unwrap();
        assert!(p.run_ranges().all(|(_, c)| mark(M, c) == Some(4)));
        assert!(!has_mark(M, &p.mark));
        unmark_range(&mut d, &body_pos(0, 0), &body_pos(0, 3), M, &|v| *v == 4).unwrap();
        let p = d.body[0].as_para().unwrap();
        assert_eq!(p.run_ranges().map(|(_, c)| mark(M, c)).collect::<Vec<_>>(), [None, Some(4)]);
        unmark_everywhere(&mut d, M, &|_| true).unwrap();
        assert!(d.body.iter().filter_map(|b| b.as_para()).all(|p| p.run_ranges().all(|(_, c)| !has_mark(M, c))));
    }
}
