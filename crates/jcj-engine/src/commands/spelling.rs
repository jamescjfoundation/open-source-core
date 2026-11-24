// SPDX-License-Identifier: MIT OR Apache-2.0
//! Spelling commands over the whole document, with words the document keeps as correct.
//!
//! The dictionaries, the user dictionary and the session's ignored words are `jcj-proof`'s. On
//! top of them a document keeps its own correct words (`spelling.ignore`): they are saved with the
//! document under the extension key [`DOCUMENT_WORDS`] (`core.spelling`), and adding one is an
//! undo step.
//!
//! - `spelling.check`: every misspelled word in the body, as `{start, end, word}` with positions.
//!   Text marked "do not check spelling" and tracked deletions are skipped.
//! - `spelling.suggest {word, max?}`: suggestions for a word, best first.
//! - `spelling.ignore {word}` / `spelling.unignore {word}`: add a word to, or take it out of,
//!   the document's own words. `spelling.words` lists them.
//! - `spelling.replace {start, end, word, with}`: replace a misspelling, after checking that the
//!   text there is still `word` (an agent's list may be out of date).
//!
//! The engine's `review.*` proofing commands (`review.issues`, `review.suggestions`,
//! `review.addToDictionary`, `review.ignoreAll`) stay available; they do not know the document's
//! own words.
//!
//! **An app's own rules** (paragraphs whose text is never checked, words it knows are right,
//! such as names it keeps elsewhere) go in its own `spelling.check`: register a command with that
//! id that calls [`misspellings_with`]. It replaces this one.

use std::collections::HashSet;

use jcj_doc::{Document, ExtKey, Paragraph, Pos, StoryRef};
use jcj_proof::SpellingOptions;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::cmd::{parse_pos, pos_json, sel_result};
use crate::{CmdError, CmdResult, CommandSpec, Session, params as p};

/// The document's own correct words.
pub const DOCUMENT_WORDS: ExtKey<DocumentWords> = ExtKey::new("core.spelling");
/// Most words a document keeps.
pub const MAX_DOCUMENT_WORDS: usize = 20_000;
/// Longest word, in bytes.
pub const MAX_WORD: usize = 100;
/// Most misspellings one check reports.
pub const MAX_REPORTED: usize = 5_000;
/// Most suggestions one `spelling.suggest` returns.
pub const MAX_SUGGESTIONS: usize = 20;

/// Words that are correct in one document, whatever the dictionaries say.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DocumentWords {
    /// The words, as added (compared without regard to case).
    pub words: Vec<String>,
}

impl DocumentWords {
    /// The words `doc` keeps, bounded and cleaned (a damaged value reads as none).
    pub fn of(doc: &Document) -> Self {
        let mut w = DOCUMENT_WORDS.get_or_none(doc).unwrap_or_default();
        w.words.retain(|x| !x.trim().is_empty() && x.len() <= MAX_WORD);
        w.words.truncate(MAX_DOCUMENT_WORDS);
        w
    }

    /// Whether `word` is one of them.
    pub fn contains(&self, word: &str) -> bool {
        self.words.iter().any(|x| x.eq_ignore_ascii_case(word) || x.to_lowercase() == word.to_lowercase())
    }

    /// Add `word`. Returns false when it is already there, empty, too long, or the list is full.
    pub fn add(&mut self, word: &str) -> bool {
        let w = word.trim();
        if w.is_empty() || w.len() > MAX_WORD || self.words.len() >= MAX_DOCUMENT_WORDS || self.contains(w) {
            return false;
        }
        self.words.push(w.to_string());
        true
    }

    /// Take `word` out. Returns whether it was there.
    pub fn remove(&mut self, word: &str) -> bool {
        let w = word.trim().to_lowercase();
        let before = self.words.len();
        self.words.retain(|x| x.to_lowercase() != w);
        self.words.len() != before
    }

    /// Store the words in `doc` (an empty list removes the key).
    pub fn store(&self, doc: &mut Document) -> Result<(), CmdError> {
        if self.words.is_empty() {
            DOCUMENT_WORDS.remove(doc);
            return Ok(());
        }
        DOCUMENT_WORDS.set(doc, self).map_err(|e| CmdError::Failed(e.to_string()))
    }
}

/// A misspelled word.
#[derive(Clone, Debug, PartialEq)]
pub struct Misspelling {
    /// Where it starts.
    pub start: Pos,
    /// Where it ends.
    pub end: Pos,
    /// The word as written.
    pub word: String,
}

/// The misspellings in `doc`'s body with the default options and no rules of the app's own: only
/// the dictionaries and the document's own words.
pub fn misspellings(doc: &Document) -> Vec<Misspelling> {
    misspellings_with(doc, &SpellingOptions::default(), &|_| false, &|_| false)
}

/// The misspellings in `doc`'s body, in order, at most [`MAX_REPORTED`]. Paragraphs for which
/// `skip` answers true are not checked, and words for which `correct` answers true are right.
/// Each word is checked in its text's language; the document's own words are always right.
pub fn misspellings_with(
    doc: &Document,
    opts: &SpellingOptions,
    skip: &dyn Fn(&Paragraph) -> bool,
    correct: &dyn Fn(&str) -> bool,
) -> Vec<Misspelling> {
    let own: HashSet<String> = DocumentWords::of(doc).words.iter().map(|w| w.to_lowercase()).collect();
    let mut out = Vec::new();
    for path in doc.para_paths(StoryRef::Body) {
        let Some(para) = doc.para(StoryRef::Body, &path) else { continue };
        if skip(para) {
            continue;
        }
        for r in jcj_proof::words(&para.text) {
            let Some(word) = para.text.get(r.clone()) else { continue };
            let props = para.props_of_char(r.start);
            if props.del.is_some() {
                continue;
            }
            let resolved = doc.styles.resolve_char(para.props.style.as_deref(), props);
            if resolved.no_proof || own.contains(&word.to_lowercase()) || correct(word) {
                continue;
            }
            if !jcj_proof::is_correct_with(word, resolved.lang.as_deref(), opts) {
                let at = |off| Pos { story: StoryRef::Body, path: path.clone(), off };
                out.push(Misspelling { start: at(r.start), end: at(r.end), word: word.to_string() });
                if out.len() >= MAX_REPORTED {
                    return out;
                }
            }
        }
    }
    out
}

/// The JSON `spelling.check` answers with for `found`.
pub fn misspellings_json(found: &[Misspelling]) -> Value {
    let list: Vec<Value> = found.iter().map(|m| json!({"start": pos_json(&m.start), "end": pos_json(&m.end), "word": m.word})).collect();
    json!({"misspellings": list, "truncated": found.len() >= MAX_REPORTED})
}

fn check(s: &mut Session, _: &Value) -> CmdResult {
    Ok(misspellings_json(&misspellings(&s.doc)))
}

fn word_param(v: &Value) -> Result<String, CmdError> {
    let w = p::req_str(v, "word")?.trim();
    if w.is_empty() || w.len() > MAX_WORD {
        return Err(CmdError::Params(format!("`word` is 1 to {MAX_WORD} bytes")));
    }
    Ok(w.to_string())
}

fn suggest(_: &mut Session, v: &Value) -> CmdResult {
    let word = word_param(v)?;
    let max = p::u64(v, "max").map_or(8, |m| usize::try_from(m).unwrap_or(MAX_SUGGESTIONS).clamp(1, MAX_SUGGESTIONS));
    Ok(json!({"suggestions": jcj_proof::suggestions(&word, max, None)}))
}

fn ignore(s: &mut Session, v: &Value) -> CmdResult {
    let word = word_param(v)?;
    let mut w = DocumentWords::of(&s.doc);
    let added = w.add(&word);
    if added {
        w.store(&mut s.doc)?;
    }
    Ok(json!({"added": added}))
}

fn unignore(s: &mut Session, v: &Value) -> CmdResult {
    let word = word_param(v)?;
    let mut w = DocumentWords::of(&s.doc);
    let removed = w.remove(&word);
    if removed {
        w.store(&mut s.doc)?;
    }
    Ok(json!({"removed": removed}))
}

fn pos_param(v: &Value, key: &str) -> Result<Pos, CmdError> {
    v.get(key).and_then(parse_pos).ok_or_else(|| CmdError::Params(format!("`{key}` must be a position")))
}

fn replace(s: &mut Session, v: &Value) -> CmdResult {
    let a = pos_param(v, "start")?;
    let b = pos_param(v, "end")?;
    let word = p::req_str(v, "word")?;
    let with = p::req_str(v, "with")?;
    if with.contains(['\n', '\r']) || with.len() > 2 * MAX_WORD {
        return Err(CmdError::Params(format!("`with` is one line of at most {} bytes", 2 * MAX_WORD)));
    }
    let here = (a.story == b.story && a.path == b.path && a.off <= b.off).then(|| s.doc.para_at(&a).and_then(|p| p.text.get(a.off..b.off))).flatten();
    if here != Some(word) {
        return Err(CmdError::Failed(format!("the text there is no longer `{word}`")));
    }
    super::find::replace_range(s, &a, &b, with)?;
    sel_result(s)
}

/// The spelling commands.
pub fn specs() -> Vec<CommandSpec> {
    const LOC: &str = "Review › Spelling";
    const WORD: &str = r#"{"word": string}"#;
    vec![
        CommandSpec::new("spelling.check", "Check Spelling", LOC, check).pure(),
        CommandSpec::new("spelling.suggest", "Spelling Suggestions", LOC, suggest).params(r#"{"word": string, "max"?: n}"#).pure(),
        CommandSpec::new("spelling.ignore", "Ignore in This Document", LOC, ignore).params(WORD),
        CommandSpec::new("spelling.unignore", "Check Again in This Document", LOC, unignore).params(WORD),
        CommandSpec::new("spelling.words", "Words of This Document", LOC, |s, _| Ok(json!({"words": DocumentWords::of(&s.doc).words}))).pure(),
        CommandSpec::new("spelling.replace", "Replace Misspelling", LOC, replace)
            .params(r#"{"start": position, "end": position, "word": string, "with": string}"#),
    ]
}
