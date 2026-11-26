// SPDX-License-Identifier: MIT OR Apache-2.0
//! Comparing two versions of a text word by word: paragraphs are matched first, then the ones
//! left over are compared word by word.
//!
//! The code is `jcj_doc::wordiff`, where a crate with no engine or layout can use it too (core
//! issue #31). These are the same items under the names they have had here since core issue
//! #17. [`crate::compare`] marks whole paragraphs as tracked changes; this is the word-level view
//! beside it.

pub use jcj_doc::wordiff::{Change, Diff, Segment, documents, paragraph, paragraphs, tokens};

#[cfg(test)]
mod tests {
    #[test]
    fn the_comparison_here_is_the_model_crates() {
        let old = ["One.".to_owned(), "Two words here.".to_owned()];
        let new = ["One.".to_owned(), "Two new words here.".to_owned()];
        let d: jcj_doc::wordiff::Diff = super::paragraphs(&old, &new);
        assert_eq!((d.added, d.removed), (1, 0));
        assert!(!d.is_same());
        let segs: Vec<jcj_doc::wordiff::Segment> = super::paragraph("a b", "a c");
        assert!(segs.contains(&super::Segment { change: super::Change::Added, text: "c".into() }));
        assert_eq!(super::tokens("a, b"), jcj_doc::wordiff::tokens("a, b"));
        assert!(super::documents(&jcj_doc::Document::new(), &jcj_doc::Document::new()).is_same());
    }
}
