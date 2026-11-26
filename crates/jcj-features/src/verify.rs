// SPDX-License-Identifier: MIT OR Apache-2.0
//! The word-for-word check: the words of a source and of what was made from it (an import, an
//! export, a conversion), in reading order, compared exactly.
//!
//! The code is `jcj_doc::verify`, where a crate with no engine or layout can use it too (core
//! issue #31). These are the same items under the names they have had here since core issue #10.

pub use jcj_doc::verify::{CONTEXT, Check, Difference, compare, document_words, paragraph_words, words};

#[cfg(test)]
mod tests {
    #[test]
    fn the_check_here_is_the_model_crates() {
        let words = super::words("“Hello,” she said — again.");
        assert_eq!(words, jcj_doc::words("“Hello,” she said — again."));
        let check: jcj_doc::verify::Check = super::Check::of(&words, &words);
        assert_eq!(check, super::Check::Same(4));
        let difference: Result<usize, jcj_doc::verify::Difference> = super::compare(&words, &words[..1]);
        assert_eq!(difference.unwrap_err().at, 1);
        assert_eq!(super::CONTEXT, jcj_doc::verify::CONTEXT);
        let p = jcj_doc::Paragraph::with_text("Two words", jcj_doc::CharProps::default());
        assert_eq!(super::paragraph_words(&p).len(), 2);
        assert_eq!(super::document_words(&jcj_doc::Document::new()), Vec::<String>::new());
    }
}
