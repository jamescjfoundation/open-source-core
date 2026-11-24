// SPDX-License-Identifier: MIT OR Apache-2.0
//! Comparing two versions of a document, paragraph by paragraph.
//!
//! The body is first cut into groups by a [`Grouping`] the app supplies: the whole body as one
//! group ([`WholeDocument`]), the stretches that start at each heading of a style ([`ByStyle`]),
//! or the app's own units. Groups are matched first (the same key, else the most similar words,
//! at least half shared), then paragraphs within matched groups (the longest common run of
//! paragraphs with the same style and words); matched groups whose order changed are found as
//! moved. The result is the newer document marked up with tracked changes: added paragraphs as
//! insertions and removed paragraphs put back as deletions, by "Comparison", and a note
//! ([`crate::notes`]) on the first paragraph of each moved group saying where it was.
//!
//! Ported from the first foundation app's comparison, with its own grouping rule made the
//! [`Grouping`] hook.

use std::collections::HashSet;
use std::ops::Range;

use jcj_doc::{Block, Document, Paragraph, Revision, RevisionKind, para_block};
use serde_json::{Value, json};

use crate::marks::mark_range;
use crate::notes::{NOTE, Note, Notes};

/// The author of the tracked changes and notes a comparison makes.
pub const AUTHOR: &str = "Comparison";

/// What changed between two versions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub groups_added: usize,
    pub groups_removed: usize,
    pub groups_moved: usize,
    pub paragraphs_added: usize,
    pub paragraphs_removed: usize,
}

impl Summary {
    pub fn to_json(&self) -> Value {
        json!({
            "groupsAdded": self.groups_added,
            "groupsRemoved": self.groups_removed,
            "groupsMoved": self.groups_moved,
            "paragraphsAdded": self.paragraphs_added,
            "paragraphsRemoved": self.paragraphs_removed,
        })
    }
}

/// How a comparison cuts a document into groups and matches them. Only top-level body blocks
/// are grouped; a table is compared as a whole block.
pub trait Grouping {
    /// The groups of `doc`'s body as block ranges, in order. They should cover the body; blocks
    /// left out are not compared.
    fn groups(&self, doc: &Document) -> Vec<Range<usize>>;

    /// A group's key: groups with the same key are matched first. Default: its first
    /// paragraph's text, trimmed, upper case.
    fn key(&self, doc: &Document, group: &Range<usize>) -> String {
        doc.body.get(group.start).and_then(|b| b.as_para()).map(|p| p.text.trim().to_uppercase()).unwrap_or_default()
    }

    /// The words a group is matched on when no key matches. Default: the words of every
    /// paragraph but the first (headings alone share too many words), lower case.
    fn words(&self, doc: &Document, group: &Range<usize>) -> HashSet<String> {
        (group.start + 1..group.end)
            .filter_map(|i| doc.body.get(i).and_then(|b| b.as_para()))
            .flat_map(|p| p.text.split_whitespace().map(str::to_lowercase).collect::<Vec<_>>())
            .collect()
    }

    /// The note put on a group that moved; `old` is its index among the old version's groups.
    /// `None` puts no note.
    fn moved_note(&self, old: usize) -> Option<String> {
        Some(format!("Moved: this was part {} in the other version.", old + 1))
    }
}

/// The whole body as one group: a plain paragraph-by-paragraph comparison.
#[derive(Clone, Copy, Debug, Default)]
pub struct WholeDocument;

impl Grouping for WholeDocument {
    fn groups(&self, doc: &Document) -> Vec<Range<usize>> {
        std::iter::once(0..doc.body.len()).collect()
    }
    fn key(&self, _: &Document, _: &Range<usize>) -> String {
        String::new()
    }
}

/// Groups that start at each paragraph in style `style` (a heading); the text before the first
/// is a group of its own.
#[derive(Clone, Debug)]
pub struct ByStyle(pub String);

impl Grouping for ByStyle {
    fn groups(&self, doc: &Document) -> Vec<Range<usize>> {
        let starts: Vec<usize> = doc
            .body
            .iter()
            .enumerate()
            .filter(|(_, b)| b.as_para().is_some_and(|p| p.props.style.as_deref() == Some(self.0.as_str())))
            .map(|(i, _)| i)
            .collect();
        let mut out = Vec::new();
        let first = starts.first().copied().unwrap_or(doc.body.len());
        if first > 0 {
            out.push(0..first);
        }
        for (k, s) in starts.iter().enumerate() {
            out.push(*s..starts.get(k + 1).copied().unwrap_or(doc.body.len()));
        }
        out
    }
}

/// A paragraph as compared: its style and its words.
fn para_key(b: &Block) -> String {
    match b.as_para() {
        Some(p) => format!("{}|{}", p.props.style.as_deref().unwrap_or(""), p.text.split_whitespace().collect::<Vec<_>>().join(" ")),
        None => "\u{1}table".into(),
    }
}

/// The longest common subsequence of `a` and `b`, as index pairs.
#[allow(clippy::indexing_slicing, reason = "i < n and j < m by the loops, and the table is (n + 1) by (m + 1)")]
fn lcs<T: PartialEq>(a: &[T], b: &[T]) -> Vec<(usize, usize)> {
    let (n, m) = (a.len(), b.len());
    // Long documents: a quadratic table over every paragraph would be too big; fall back to a
    // greedy forward match beyond this.
    if n.saturating_mul(m) > 4_000_000 {
        let mut out = Vec::new();
        let mut j0 = 0;
        for (i, x) in a.iter().enumerate() {
            if let Some(j) = (j0..m).find(|&j| b.get(j) == Some(x)) {
                out.push((i, j));
                j0 = j + 1;
            }
        }
        return out;
    }
    let mut t = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            t[i][j] = if a[i] == b[j] { t[i + 1][j + 1] + 1 } else { t[i + 1][j].max(t[i][j + 1]) };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < n && j < m {
        if a[i] == b[j] {
            out.push((i, j));
            i += 1;
            j += 1;
        } else if t[i + 1][j] >= t[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

/// Match each new group with an old one: the same key first, then the most similar words.
fn match_groups(old: &Document, new: &Document, so: &[Range<usize>], sn: &[Range<usize>], g: &dyn Grouping) -> Vec<Option<usize>> {
    let mut matched: Vec<Option<usize>> = vec![None; sn.len()];
    let mut taken = vec![false; so.len()];
    let old_keys: Vec<String> = so.iter().map(|o| g.key(old, o)).collect();
    for (j, s) in sn.iter().enumerate() {
        let k = g.key(new, s);
        if let Some(i) = old_keys.iter().enumerate().position(|(i, ok)| taken.get(i) == Some(&false) && *ok == k) {
            matched[j] = Some(i);
            taken[i] = true;
        }
    }
    for (j, s) in sn.iter().enumerate() {
        if matched.get(j).is_some_and(Option::is_some) {
            continue;
        }
        let wn = g.words(new, s);
        if wn.is_empty() {
            continue;
        }
        let best = so
            .iter()
            .enumerate()
            .filter(|(i, _)| taken.get(*i) == Some(&false))
            .map(|(i, o)| {
                let wo = g.words(old, o);
                let shared = wn.intersection(&wo).count() as f32;
                (i, shared / (wn.len().max(wo.len()).max(1) as f32))
            })
            .filter(|(_, sim)| *sim >= 0.5)
            .max_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((i, _)) = best {
            matched[j] = Some(i);
            taken[i] = true;
        }
    }
    matched
}

/// Mark the whole of `b` as inserted or deleted with record `which`.
fn mark(b: &Block, which: u32, inserted: bool) -> std::sync::Arc<Block> {
    let set = |c: &mut jcj_doc::CharProps| {
        if inserted {
            c.ins = Some(which);
        } else {
            c.del = Some(which);
        }
    };
    match b {
        Block::Para(p) => {
            let mut p: Paragraph = p.clone();
            let len = p.len();
            let _ = p.format(0, len, &set);
            p.touch();
            para_block(p)
        }
        // A table is put in or back as it is.
        Block::Table(_) => std::sync::Arc::new(b.clone()),
    }
}

/// Compare `old` with `new`, grouped by `grouping`: the marked-up copy of `new` and what changed.
pub fn compare(old: &Document, new: &Document, grouping: &dyn Grouping) -> (Document, Summary) {
    let (so, sn) = (grouping.groups(old), grouping.groups(new));
    let matched = match_groups(old, new, &so, &sn, grouping);
    // Moved: matched groups outside the longest run kept in order.
    let order: Vec<usize> = matched.iter().flatten().copied().collect();
    let mut sorted = order.clone();
    sorted.sort_unstable();
    let kept: HashSet<usize> = lcs(&order, &sorted).into_iter().filter_map(|(k, _)| order.get(k).copied()).collect();
    let mut out = new.clone();
    let mut summary = Summary::default();
    let ins = u32::try_from(out.revisions.len()).unwrap_or(u32::MAX);
    out.revisions.push(Revision { kind: RevisionKind::Insert, author: AUTHOR.into(), date: String::new(), move_name: None });
    let del = ins.saturating_add(1);
    out.revisions.push(Revision { kind: RevisionKind::Delete, author: AUTHOR.into(), date: String::new(), move_name: None });
    let old_blocks = |r: &Range<usize>| old.body.get(r.clone()).unwrap_or_default();
    let mut body = Vec::new();
    let mut moved_notes: Vec<(usize, String)> = Vec::new();
    let taken: HashSet<usize> = order.iter().copied().collect();
    let mut removed: Vec<usize> = (0..so.len()).filter(|i| !taken.contains(i)).collect();
    let put_back = |r: usize, body: &mut Vec<std::sync::Arc<Block>>, summary: &mut Summary| {
        summary.groups_removed += 1;
        for b in so.get(r).map(&old_blocks).unwrap_or_default() {
            body.push(mark(b, del, false));
            summary.paragraphs_removed += 1;
        }
    };
    for (j, s) in sn.iter().enumerate() {
        let new_blocks = new.body.get(s.clone()).unwrap_or_default();
        let matched_j = matched.get(j).copied().flatten();
        // Removed groups that came before this one's match go in first.
        if let Some(i) = matched_j {
            for r in removed.iter().copied().filter(|r| *r < i).collect::<Vec<_>>() {
                removed.retain(|x| *x != r);
                put_back(r, &mut body, &mut summary);
            }
        }
        let Some(i) = matched_j else {
            summary.groups_added += 1;
            for b in new_blocks {
                body.push(mark(b, ins, true));
                summary.paragraphs_added += 1;
            }
            continue;
        };
        if !kept.contains(&i) {
            summary.groups_moved += 1;
            if let Some(text) = grouping.moved_note(i) {
                moved_notes.push((body.len(), text));
            }
        }
        let olds = so.get(i).map(&old_blocks).unwrap_or_default();
        let ko: Vec<String> = olds.iter().map(|b| para_key(b)).collect();
        let kn: Vec<String> = new_blocks.iter().map(|b| para_key(b)).collect();
        let (mut oi, mut ni) = (0, 0);
        for (a, b) in lcs(&ko, &kn).into_iter().chain(std::iter::once((ko.len(), kn.len()))) {
            for b in olds.get(oi..a).unwrap_or_default() {
                body.push(mark(b, del, false));
                summary.paragraphs_removed += 1;
            }
            for b in new_blocks.get(ni..b).unwrap_or_default() {
                body.push(mark(b, ins, true));
                summary.paragraphs_added += 1;
            }
            if let Some(b) = new_blocks.get(b) {
                body.push(b.clone());
            }
            oi = a + 1;
            ni = b + 1;
        }
    }
    for r in removed {
        put_back(r, &mut body, &mut summary);
    }
    out.body = body;
    out.ensure_nonempty();
    // A note on each moved group's first paragraph.
    let mut notes = Notes::of_doc(&out);
    for (block, text) in moved_notes {
        let len = out.body.get(block).and_then(|b| b.as_para()).map_or(0, Paragraph::len);
        if len == 0 {
            continue;
        }
        let Ok(id) = notes.add(Note { text, author: AUTHOR.into(), ..Note::default() }) else { break };
        let _ = mark_range(&mut out, &crate::data::body_pos(block, 0), &crate::data::body_pos(block, len), NOTE, &id);
    }
    let _ = notes.store(&mut out);
    (out, summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracking::changes;
    use jcj_doc::CharProps;

    fn doc(lines: &[(&str, &str)]) -> Document {
        let mut d = Document::new();
        d.body = lines
            .iter()
            .map(|(style, t)| {
                let mut p = Paragraph::with_text(t, CharProps::default());
                p.props.style = Some((*style).into());
                para_block(p)
            })
            .collect();
        d
    }

    fn text(d: &Document, c: &crate::tracking::Change) -> String {
        d.para(jcj_doc::StoryRef::Body, &c.path).map(|p| p.text[c.start..c.end].to_string()).unwrap_or_default()
    }

    #[test]
    fn added_removed_changed_and_moved_groups() {
        let old = doc(&[
            ("Heading1", "Harbour"),
            ("Normal", "Rain falls."),
            ("Normal", "Boats wait."),
            ("Heading1", "Market"),
            ("Normal", "Wind."),
            ("Heading1", "Attic"),
            ("Normal", "Dust."),
        ]);
        let new = doc(&[
            ("Heading1", "Market"),
            ("Normal", "Wind."),
            ("Heading1", "Harbour"),
            ("Normal", "Rain falls."),
            ("Normal", "Boats leave."),
            ("Heading1", "Cellar"),
            ("Normal", "Damp."),
        ]);
        let (marked, sum) = compare(&old, &new, &ByStyle("Heading1".into()));
        assert_eq!((sum.groups_added, sum.groups_removed, sum.groups_moved), (1, 1, 1));
        assert_eq!((sum.paragraphs_added, sum.paragraphs_removed), (3, 3));
        let ch = changes(&marked);
        assert!(ch.iter().any(|c| !c.inserted && text(&marked, c) == "Boats wait."));
        assert!(ch.iter().any(|c| c.inserted && text(&marked, c) == "Boats leave."));
        assert!(ch.iter().any(|c| !c.inserted && text(&marked, c) == "Attic"));
        assert_eq!(Notes::of_doc(&marked).notes.len(), 1, "one moved group noted");
        assert_eq!(sum.to_json()["groupsMoved"], 1);
        let (same, s2) = compare(&new, &new, &ByStyle("Heading1".into()));
        assert_eq!(s2, Summary::default());
        assert!(changes(&same).is_empty());
    }

    #[test]
    fn whole_document_compares_paragraphs() {
        let old = doc(&[("Normal", "One."), ("Normal", "Two."), ("Normal", "Three.")]);
        let new = doc(&[("Normal", "One."), ("Normal", "Three."), ("Normal", "Four.")]);
        let (marked, sum) = compare(&old, &new, &WholeDocument);
        assert_eq!((sum.paragraphs_added, sum.paragraphs_removed, sum.groups_moved), (1, 1, 0));
        let texts: Vec<String> = marked.body.iter().filter_map(|b| b.as_para()).map(|p| p.text.clone()).collect();
        assert_eq!(texts, ["One.", "Two.", "Three.", "Four."]);
        assert!(Notes::of_doc(&marked).notes.is_empty());
    }
}
