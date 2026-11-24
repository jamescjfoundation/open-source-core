// SPDX-License-Identifier: MIT OR Apache-2.0
//! Spelling and hyphenation.
//!
//! Wraps craft-libs' `craft-proof`. A language is a BCP 47 tag (`en-US`, `pl`, `pt-BR`); `None`
//! means the document default, US English.
//!
//! - **Spelling**: [`is_correct`], [`suggestions`], [`check`] (spelling and grammar marks in a
//!   paragraph). English always has a word list: the SCOWL dictionaries when the language data
//!   was built in (`CRAFT_I18N_DIR` at build time), else an English Hunspell dictionary installed
//!   on the system, else a public-domain word list built into the crate. Other languages use the
//!   Hunspell dictionary installed on the system; without one their text is not marked.
//! - **The user's words**: [`add_word`] (Add to Dictionary, kept in the user dictionary file),
//!   [`ignore_word`] (Ignore All, for this run only), [`load_user_dictionary`],
//!   [`save_user_dictionary`]. They apply to every language and every document in the process.
//! - **Hyphenation**: [`hyphen_points`] and [`hyphenate`]. Pattern-based hyphenation needs the
//!   language data built in; without it English words still hyphenate from the dictionary
//!   where they can, and rule-based languages (Bulgarian, Russian, Turkish) always do.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::ops::Range;
use std::path::Path;

use jcj_upstream::proof as up;
use up::ProofLang;

/// What the spelling check skips: words in capitals, words with digits, web and email addresses
/// and file paths. All on by default.
pub use up::Options as SpellingOptions;

/// Hyphenation limits: the shortest word hyphenated, the fewest letters before and after a
/// hyphen, whether capitalized words are hyphenated.
pub use up::hyphen::Limits as HyphenLimits;

fn lang(tag: Option<&str>) -> ProofLang {
    ProofLang::from_tag(tag)
}

/// Is there a spelling dictionary for language `tag` (English always has one)?
pub fn has_dictionary(tag: Option<&str>) -> bool {
    up::has_dictionary(tag)
}

/// Is `word` spelled correctly in language `tag`? Words in the user dictionary or ignored are.
/// A language without a dictionary marks nothing, so every word is correct in it.
pub fn is_correct(word: &str, tag: Option<&str>) -> bool {
    up::is_correct_for(word, tag)
}

/// [`is_correct`] with the spelling options.
pub fn is_correct_with(word: &str, tag: Option<&str>, opts: &SpellingOptions) -> bool {
    up::is_correct_in_with(word, lang(tag), opts)
}

/// Spelling suggestions for `word` in language `tag`, best first, at most `max`.
pub fn suggestions(word: &str, max: usize, tag: Option<&str>) -> Vec<String> {
    up::suggest_in(word, max, lang(tag))
}

/// What a mark is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssueKind {
    Spelling,
    Grammar,
}

/// A spelling or grammar mark in a paragraph.
#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    /// The marked bytes of the text.
    pub range: Range<usize>,
    pub kind: IssueKind,
    /// What is wrong, in the language of the text.
    pub message: String,
    /// Replacements, best first. Spelling marks carry none: ask [`suggestions`] when the user
    /// opens the mark.
    pub suggestions: Vec<String>,
}

impl From<up::Issue> for Issue {
    fn from(i: up::Issue) -> Self {
        Issue {
            range: i.start..i.end,
            kind: match i.kind {
                up::IssueKind::Spelling => IssueKind::Spelling,
                up::IssueKind::Grammar => IssueKind::Grammar,
            },
            message: i.message,
            suggestions: i.suggestions,
        }
    }
}

/// The spelling marks, and the grammar marks when `grammar` is on, in a paragraph of text in
/// language `tag`, in order.
pub fn check(text: &str, tag: Option<&str>, opts: &SpellingOptions, grammar: bool) -> Vec<Issue> {
    let spans = [(0..text.len(), lang(tag))];
    up::check_text_with(text, &spans, opts, grammar, false).into_iter().map(Issue::from).collect()
}

/// The words of `text` as byte ranges, as the spelling check sees them.
pub fn words(text: &str) -> Vec<Range<usize>> {
    up::words(text).into_iter().map(|(a, b)| a..b).collect()
}

/// Add `word` to the user dictionary ("Add to Dictionary"), for every language.
pub fn add_word(word: &str) {
    up::add_word(word);
}

/// A number that changes whenever the user dictionary or the dictionaries in use change (a word
/// added or removed). Keep it with cached spelling results and check again when it moves.
pub fn dictionary_generation() -> u64 {
    up::dictionary_generation()
}

/// Take `word` out of the user dictionary. Returns whether it was there.
pub fn remove_word(word: &str) -> bool {
    up::remove_word(word)
}

/// Accept `word` everywhere until the app closes ("Ignore All"). Not saved with the user
/// dictionary.
pub fn ignore_word(word: &str) {
    up::ignore_word(word);
}

/// The words in the user dictionary, sorted (lower case).
pub fn user_words() -> Vec<String> {
    up::user_dictionary()
}

/// Read a user dictionary file (one word per line) into the user dictionary. A missing or
/// unreadable file reads as empty; an oversized one is capped. Returns the words added.
pub fn load_user_dictionary(path: &Path) -> Vec<String> {
    up::load_user_dictionary(path)
}

/// Write the user dictionary to `path`, merging words another process saved there since, and
/// leaving out `removed` (words just taken out). Replaces the file atomically.
pub fn save_user_dictionary(path: &Path, removed: &[String]) -> std::io::Result<()> {
    up::save_user_dictionary(path, removed)
}

/// A number that changes whenever the user dictionary or the ignored words change: cached marks
/// older than it are checked again.
pub fn generation() -> u64 {
    up::dictionary_generation()
}

/// Where `word` (in language `tag`) may be hyphenated, as character indices: a hyphen goes
/// before the character at each index. Limits apply to each run of letters.
pub fn hyphen_points(word: &str, tag: Option<&str>, limits: &HyphenLimits) -> Vec<usize> {
    up::hyphen::hyphen_points_in(word, limits, lang(tag))
}

/// `word` with `-` at each hyphenation point (default limits): for display and diagnostics.
pub fn hyphenate(word: &str, tag: Option<&str>) -> String {
    up::hyphen::hyphenate_word_in(word, &HyphenLimits::default(), lang(tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dictionary_generation_moves_when_a_word_is_added() {
        let before = dictionary_generation();
        add_word("zzqxgenerationword");
        assert_ne!(dictionary_generation(), before);
        remove_word("zzqxgenerationword");
    }

    #[test]
    fn english_is_always_checked() {
        assert!(has_dictionary(None));
        assert!(has_dictionary(Some("en-GB")));
        assert!(is_correct("house", None));
        assert!(!is_correct("hosue", None));
        // Capitals and digits are skipped by default.
        assert!(is_correct("QWXZT", None));
        assert!(!is_correct_with("QWXZT", None, &SpellingOptions { ignore_uppercase: false, ..Default::default() }));
    }

    #[test]
    fn a_misspelling_gets_suggestions() {
        let s = suggestions("hosue", 5, None);
        assert!(!s.is_empty() && s.len() <= 5);
        assert!(s.iter().any(|w| w == "house"), "{s:?}");
    }

    #[test]
    fn a_paragraph_check_marks_spelling_and_grammar() {
        let text = "This is is a sentense.";
        let issues = check(text, None, &SpellingOptions::default(), true);
        let spelling: Vec<&Issue> = issues.iter().filter(|i| i.kind == IssueKind::Spelling).collect();
        assert_eq!(spelling.len(), 1);
        assert_eq!(&text[spelling[0].range.clone()], "sentense");
        assert!(issues.iter().any(|i| i.kind == IssueKind::Grammar));
        assert!(check(text, None, &SpellingOptions::default(), false).iter().all(|i| i.kind == IssueKind::Spelling));
        assert_eq!(words("Two words."), vec![0..3, 4..9]);
    }

    #[test]
    fn the_users_words_are_accepted() {
        // Made-up words, so other tests in the process are not affected.
        let added = "zorblequin";
        let ignored = "flimbertax";
        assert!(!is_correct(added, None) && !is_correct(ignored, None));
        let before = generation();
        add_word(added);
        ignore_word(ignored);
        assert!(is_correct(added, None) && is_correct(ignored, None));
        assert!(user_words().contains(&added.to_string()));
        assert!(!user_words().contains(&ignored.to_string()));
        assert!(generation() > before);
        assert!(remove_word(added));
        assert!(!is_correct(added, None));
        assert!(!remove_word(added));
    }

    #[test]
    fn a_user_dictionary_file_round_trips() {
        let dir = std::env::temp_dir().join(format!("jcj-proof-{}", std::process::id()));
        let file = dir.join("words.txt");
        add_word("quorbantic");
        save_user_dictionary(&file, &[]).unwrap();
        assert!(std::fs::read_to_string(&file).unwrap().lines().any(|l| l == "quorbantic"));
        remove_word("quorbantic");
        assert!(load_user_dictionary(&file).contains(&"quorbantic".to_string()));
        assert!(is_correct("quorbantic", None));
        remove_word("quorbantic");
        assert!(load_user_dictionary(&dir.join("missing.txt")).is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn hyphen_points_respect_the_limits_with_or_without_language_data() {
        let word = "hyphenation";
        let lim = HyphenLimits::default();
        let points = hyphen_points(word, None, &lim);
        let n = word.chars().count();
        assert!(points.iter().all(|&p| p >= lim.after_first && p <= n - lim.before_last), "{points:?}");
        assert!(points.windows(2).all(|w| w[0] < w[1]));
        let shown = hyphenate(word, None);
        assert_eq!(shown.replace('-', ""), word);
        assert_eq!(shown.matches('-').count(), points.len());
        // Too short to hyphenate.
        assert!(hyphen_points("cat", None, &lim).is_empty());
    }
}
