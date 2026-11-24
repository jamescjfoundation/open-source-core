// SPDX-License-Identifier: MIT OR Apache-2.0
//! Page locking: keeping page numbers put while a document is revised.
//!
//! Locking marks the first character of every page with a lock id (run mark [`PAGE_LOCK`],
//! `core.page_lock`) and records each lock's page label and lock level in the document
//! ([`LOCKS`], `core.page_locks`). Pagination then starts a page at every mark and never pulls
//! text back across one ([`LockPass`]). Text that no longer fits flows onto added pages labelled
//! with letters after the locked page before them; locked pages whose text was all deleted are
//! listed as placeholders labelled with their combined labels ([`label_pages`]).
//!
//! The page-label scheme: a locked page keeps its label ("12"). Pages added after it take its
//! label and letters in order: 12A, 12B … 12Z, 12ZA. When a lettered page is itself locked (a
//! second lock level), pages added after it take another letter: after 12A, 12AA, 12AB. A run of
//! deleted locked pages leaves one placeholder labelled first-last ("10-11", "7B-9"), or the one
//! label when a single page went. Pages before the first locked page are numbered 1, 2, 3.
//!
//! **With pagination.** [`LockPass`] wraps an app's own pagination pass. A lock on the first
//! character of a paragraph starts a page there. A lock inside a paragraph (a page that began in
//! the middle of one) lets the paragraph split only there; when the lines before it do not fit,
//! the whole paragraph goes to the next page and the locked page starts with it. Placeholders are
//! data ([`PageLabels::placeholders`]): the pass cannot add empty pages, so an app that prints
//! them adds them itself.
//!
//! Ported from the first foundation app's page locking (its decision 0026), with the marks moved into
//! extension data.

use std::collections::{BTreeMap, HashMap};

use jcj_doc::{Document, ExtKey, Paragraph, Pos, StoryRef};
use jcj_engine::{CmdError, CmdResult, CommandSpec, Session};
use jcj_layout::measure::MeasuredBlock;
use jcj_layout::paginate::{Continuation, PageBreak, PaginationPass, Piece, Source};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::data::{body_pos, load, next_id, store};
use crate::marks::{mark_range, unmark_everywhere};
use crate::pages::{page_start, planned_pages};

/// Run mark: the lock id of the locked page this character starts.
pub const PAGE_LOCK: ExtKey<u32> = ExtKey::new("core.page_lock");
/// Document data: the locked pages ([`Locks`]).
pub const LOCKS: ExtKey<Locks> = ExtKey::new("core.page_locks");
/// Most locked pages a document keeps.
pub const MAX_LOCKS: usize = 100_000;
/// Longest page label, characters.
pub const MAX_LABEL_CHARS: usize = 24;

/// One locked page.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lock {
    pub id: u32,
    /// The page's label: "12", "12A".
    pub label: String,
    /// The lock level that made it (1 for the first lock).
    pub level: u32,
}

/// The locked pages, in page order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Locks {
    pub pages: Vec<Lock>,
}

impl Locks {
    /// The locks `doc` holds; bad data reads as none, labels are cut to letters and digits.
    pub fn of_doc(doc: &Document) -> Self {
        let mut l = load(doc, LOCKS);
        l.pages.truncate(MAX_LOCKS);
        for p in &mut l.pages {
            p.label = p.label.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(MAX_LABEL_CHARS).collect();
        }
        l
    }

    /// Store in `doc` (no locks: the key is removed).
    pub fn store(&self, doc: &mut Document) -> Result<(), CmdError> {
        if self.pages.is_empty() {
            LOCKS.remove(doc);
            return Ok(());
        }
        store(doc, LOCKS, self)
    }

    pub fn is_locked(&self) -> bool {
        !self.pages.is_empty()
    }

    /// The newest lock level (0: not locked).
    pub fn level(&self) -> u32 {
        self.pages.iter().map(|p| p.level).max().unwrap_or(0)
    }
}

/// Letters for the `n`th page added after a locked one (1 → "A", 26 → "Z", 27 → "ZA").
pub fn letters(n: usize) -> String {
    let n = n.max(1);
    let mut out = "Z".repeat((n - 1) / 26);
    out.push(char::from(b'A' + ((n - 1) % 26) as u8));
    out
}

/// Where each lock mark is in the top-level body: lock id → (block, byte offset) of its first
/// character.
pub fn lock_marks(doc: &Document) -> HashMap<u32, (usize, usize)> {
    let mut out = HashMap::new();
    for (block, b) in doc.body.iter().enumerate() {
        let Some(p) = b.as_para() else { continue };
        for (range, c) in p.run_ranges() {
            if let Some(id) = PAGE_LOCK.get_or_none(c) {
                out.entry(id).or_insert((block, range.start));
            }
        }
    }
    out
}

/// The marks of the locks `doc` records, by position.
fn known_marks(doc: &Document) -> HashMap<(usize, usize), u32> {
    let locks = Locks::of_doc(doc);
    if !locks.is_locked() {
        return HashMap::new();
    }
    let known: std::collections::HashSet<u32> = locks.pages.iter().map(|l| l.id).collect();
    lock_marks(doc).into_iter().filter(|(id, _)| known.contains(id)).map(|(id, at)| (at, id)).collect()
}

/// A pagination pass that keeps locked pages where they are, around an app's own pass
/// (`LockPass::new(NoContinuations)` when it has none). See the module documentation.
#[derive(Clone, Debug, Default)]
pub struct LockPass<P> {
    inner: P,
    /// Byte offsets of locks inside paragraphs, by block (filled by `prepare`).
    inside: HashMap<usize, Vec<usize>>,
}

impl<P> LockPass<P> {
    pub fn new(inner: P) -> Self {
        LockPass { inner, inside: HashMap::new() }
    }
    pub fn inner(&self) -> &P {
        &self.inner
    }
    pub fn inner_mut(&mut self) -> &mut P {
        &mut self.inner
    }
}

/// A revision for a paragraph the pass changed, the same each time for the same source.
fn derived_rev(rev: u64) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (rev, "core.page_lock").hash(&mut h);
    h.finish() | (1 << 63)
}

impl<P: PaginationPass> PaginationPass for LockPass<P> {
    fn prepare(&mut self, doc: &mut Document) {
        self.inner.prepare(doc);
        self.inside.clear();
        for ((block, off), _) in known_marks(doc) {
            if off > 0 {
                self.inside.entry(block).or_default().push(off);
                continue;
            }
            if let Some(jcj_doc::Block::Para(p)) = doc.body.get_mut(block).map(std::sync::Arc::make_mut)
                && p.props.page_break_before != Some(true)
            {
                p.props.page_break_before = Some(true);
                p.rev = derived_rev(p.rev);
            }
        }
        for v in self.inside.values_mut() {
            v.sort_unstable();
        }
    }

    fn may_split(&self, doc: &Document, block: &MeasuredBlock) -> bool {
        self.inside.contains_key(&block.block) || self.inner.may_split(doc, block)
    }

    fn split_points(&self, doc: &Document, block: &MeasuredBlock) -> Option<Vec<usize>> {
        match self.inside.get(&block.block) {
            Some(v) => Some(v.clone()),
            None => self.inner.split_points(doc, block),
        }
    }

    fn continuation(&self, doc: &Document, at: &PageBreak<'_>) -> Continuation {
        self.inner.continuation(doc, at)
    }

    fn finish(&mut self, out: &mut Document, map: &[Option<Source>]) {
        self.inner.finish(out, map);
    }
}

/// One entry of the page sequence a locked document prints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LabeledPage {
    /// Planned page `index` (0-based) and its label.
    Page { index: usize, label: String },
    /// A placeholder for locked pages whose text was all deleted.
    Placeholder { label: String },
}

/// The labels of a locked document's pages.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PageLabels {
    /// The pages and placeholders in print order.
    pub sequence: Vec<LabeledPage>,
}

impl PageLabels {
    /// One label per planned page, in order (placeholders left out).
    pub fn page_labels(&self) -> Vec<String> {
        self.sequence
            .iter()
            .filter_map(|e| match e {
                LabeledPage::Page { label, .. } => Some(label.clone()),
                LabeledPage::Placeholder { .. } => None,
            })
            .collect()
    }

    /// Each placeholder: the planned page it goes before (the page count: after the last) and
    /// its label.
    pub fn placeholders(&self) -> Vec<(usize, String)> {
        let mut next = 0;
        let mut out = Vec::new();
        for e in &self.sequence {
            match e {
                LabeledPage::Page { index, .. } => next = index + 1,
                LabeledPage::Placeholder { label } => out.push((next, label.clone())),
            }
        }
        out
    }

    /// Every label in print order, placeholders included.
    pub fn all(&self) -> Vec<String> {
        self.sequence
            .iter()
            .map(|e| match e {
                LabeledPage::Page { label, .. } | LabeledPage::Placeholder { label } => label.clone(),
            })
            .collect()
    }
}

fn combined(run: &[&str]) -> String {
    match run {
        [one] => (*one).to_string(),
        [first, .., last] => format!("{first}-{last}"),
        [] => String::new(),
    }
}

/// The labels of planned `pages` of `doc`; `None` when the document is not locked.
pub fn label_pages(doc: &Document, pages: &[Vec<Piece>]) -> Option<PageLabels> {
    let locks = Locks::of_doc(doc);
    if !locks.is_locked() {
        return None;
    }
    let at = known_marks(doc);
    let page_of_lock: HashMap<u32, usize> =
        pages.iter().enumerate().filter_map(|(i, pg)| page_start(pg).and_then(|s| at.get(&s)).map(|id| (*id, i))).collect();
    // Placeholders: runs of locked pages missing from the document, before the next one present.
    let mut placeholders: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    let mut run: Vec<&str> = Vec::new();
    for lock in &locks.pages {
        match page_of_lock.get(&lock.id) {
            Some(&page) => {
                if !run.is_empty() {
                    placeholders.entry(page).or_default().push(combined(&run));
                    run.clear();
                }
            }
            None => run.push(&lock.label),
        }
    }
    if !run.is_empty() {
        placeholders.entry(pages.len()).or_default().push(combined(&run));
    }
    let label_of: HashMap<u32, &str> = locks.pages.iter().map(|l| (l.id, l.label.as_str())).collect();
    let mut sequence = Vec::with_capacity(pages.len() + placeholders.len());
    let mut base: Option<String> = None;
    let mut added = 0;
    for (i, page) in pages.iter().enumerate() {
        for label in placeholders.remove(&i).unwrap_or_default() {
            sequence.push(LabeledPage::Placeholder { label });
        }
        let locked = page_start(page).and_then(|s| at.get(&s)).and_then(|id| label_of.get(id));
        let label = match (locked, &base) {
            (Some(l), _) => {
                base = Some((*l).to_string());
                added = 0;
                (*l).to_string()
            }
            (None, Some(b)) => {
                added += 1;
                format!("{b}{}", letters(added))
            }
            (None, None) => (i + 1).to_string(),
        };
        sequence.push(LabeledPage::Page { index: i, label });
    }
    for label in placeholders.remove(&pages.len()).unwrap_or_default() {
        sequence.push(LabeledPage::Placeholder { label });
    }
    Some(PageLabels { sequence })
}

/// One label per planned page: the locked labels, or 1, 2, 3 when the document is not locked.
pub fn labels_or_numbers(doc: &Document, pages: &[Vec<Piece>]) -> Vec<String> {
    match label_pages(doc, pages) {
        Some(l) => l.page_labels(),
        None => (1..=pages.len()).map(|n| n.to_string()).collect(),
    }
}

/// The length of the character at `off` in top-level body block `block`, if there is one.
fn char_len_at(doc: &Document, block: usize, off: usize) -> Option<usize> {
    doc.body.get(block).and_then(|b| b.as_para()).and_then(|p| p.text.get(off..)).and_then(|t| t.chars().next()).map(char::len_utf8)
}

/// Put lock `id`'s mark on the character at (`block`, `off`) alone.
fn put_mark(doc: &mut Document, id: u32, block: usize, off: usize) -> Result<(), CmdError> {
    let len = char_len_at(doc, block, off).ok_or_else(|| CmdError::Failed("a locked page must start with some text".into()))?;
    mark_range(doc, &body_pos(block, off), &body_pos(block, off + len), PAGE_LOCK, &id)
}

/// Remove the marks of the locks `drop` picks.
fn remove_marks(doc: &mut Document, drop: &dyn Fn(u32) -> bool) -> Result<(), CmdError> {
    unmark_everywhere(doc, PAGE_LOCK, &|id| drop(*id))
}

/// Lock the pages of `doc` as `pages` plans them (a further lock adds a level: pages added since
/// get locked with their lettered labels). Returns the number of locked pages and the level.
pub fn lock_pages(doc: &mut Document, pages: &[Vec<Piece>]) -> Result<(usize, u32), CmdError> {
    let mut locks = Locks::of_doc(doc);
    let level = locks.level() + 1;
    let marks = lock_marks(doc);
    let at: HashMap<(usize, usize), u32> = known_marks(doc);
    let mut fresh = next_id(locks.pages.iter().map(|l| l.id).chain(marks.keys().copied()));
    let labels = labels_or_numbers(doc, pages);
    let old: HashMap<u32, Lock> = locks.pages.iter().map(|l| (l.id, l.clone())).collect();
    let mut ordered: Vec<Lock> = Vec::new();
    for (page, label) in pages.iter().zip(&labels) {
        let Some((block, off)) = page_start(page) else { continue };
        if let Some(id) = at.get(&(block, off)) {
            if let Some(l) = old.get(id) {
                ordered.push(l.clone());
            }
            continue;
        }
        // Mark the page's first character; a page starting with an empty paragraph has none.
        if char_len_at(doc, block, off).is_none() {
            continue;
        }
        let id = fresh;
        fresh = fresh.saturating_add(1);
        put_mark(doc, id, block, off)?;
        ordered.push(Lock { id, label: label.clone(), level });
    }
    // Locks of deleted pages stay, so their placeholders keep showing.
    let kept: Vec<u32> = ordered.iter().map(|l| l.id).collect();
    for (i, l) in locks.pages.iter().enumerate() {
        if !kept.contains(&l.id) && !marks.contains_key(&l.id) {
            ordered.insert(i.min(ordered.len()), l.clone());
        }
    }
    locks.pages = ordered;
    locks.pages.truncate(MAX_LOCKS);
    locks.store(doc)?;
    Ok((locks.pages.len(), level))
}

/// `lock.pages {revisionMode?}`: lock the pages as they are now. With `revisionMode`, revision
/// mode is turned on too.
fn lock(s: &mut Session, v: &Value) -> CmdResult {
    let pages = planned_pages(&s.doc);
    let (n, level) = lock_pages(&mut s.doc, &pages)?;
    if v.get("revisionMode").and_then(Value::as_bool) == Some(true) {
        let mut r = crate::revisions::Revisions::of_doc(&s.doc);
        r.mode = true;
        if r.current_index().is_none() {
            r.start_next();
        }
        r.store(&mut s.doc)?;
    }
    Ok(json!({"pages": n, "level": level}))
}

/// `lock.unlockLevel`: remove the newest lock level (its added pages flow freely again).
fn unlock_level(s: &mut Session, _: &Value) -> CmdResult {
    let mut locks = Locks::of_doc(&s.doc);
    let level = locks.level();
    if level == 0 {
        return Err(CmdError::Failed("the pages are not locked".into()));
    }
    let gone: Vec<u32> = locks.pages.iter().filter(|l| l.level == level).map(|l| l.id).collect();
    remove_marks(&mut s.doc, &|id| gone.contains(&id))?;
    locks.pages.retain(|l| l.level != level);
    locks.store(&mut s.doc)?;
    Ok(json!({"level": level - 1}))
}

/// `lock.unlockAll`: unlock every page; pages are numbered 1, 2, 3 again.
fn unlock_all(s: &mut Session, _: &Value) -> CmdResult {
    remove_marks(&mut s.doc, &|_| true)?;
    Locks::default().store(&mut s.doc)?;
    Ok(json!({"level": 0}))
}

/// `lock.status`: whether the pages are locked, the level, each page's label and the
/// placeholders.
fn status(s: &mut Session, _: &Value) -> CmdResult {
    let locks = Locks::of_doc(&s.doc);
    let labels = if locks.is_locked() { label_pages(&s.doc, &planned_pages(&s.doc)) } else { None };
    Ok(json!({
        "locked": locks.is_locked(),
        "level": locks.level(),
        "labels": labels.as_ref().map(PageLabels::all),
        "placeholders": labels.as_ref().map(|l| l.placeholders().into_iter().map(|(before, label)| json!({"before": before, "label": label})).collect::<Vec<_>>()),
    }))
}

/// The lock ids marked in the top-level body, for keeping them when text is deleted.
pub fn mark_ids(doc: &Document) -> Vec<u32> {
    let mut ids: Vec<u32> = lock_marks(doc).into_keys().collect();
    ids.sort_unstable();
    ids
}

/// After deleting text that held lock marks, the first of them (`ids`) goes on the character
/// now at `at` (or the next paragraph's first), so the locked page still starts where its text
/// goes on.
pub fn keep_marks(doc: &mut Document, ids: &[u32], at: &Pos) {
    let Some(&id) = ids.first() else { return };
    if at.story != StoryRef::Body {
        return;
    }
    let mut block = at.path.0.first().map_or(0, |b| *b as usize);
    let mut off = if at.path.0.len() == 1 { at.off } else { 0 };
    while let Some(p) = doc.body.get(block).and_then(|b| b.as_para()) {
        if let Some(len) = p.text.get(off..).and_then(|t| t.chars().next()).map(char::len_utf8) {
            if !PAGE_LOCK.is_set(p.props_of_char(off)) {
                let _ = mark_range(doc, &body_pos(block, off), &body_pos(block, off + len), PAGE_LOCK, &id);
            }
            return;
        }
        block += 1;
        off = 0;
    }
}

/// The caret's top-level block and offset.
fn caret_at(s: &Session) -> (usize, usize) {
    let f = &s.sel.focus;
    let block = f.path.0.first().map_or(0, |b| *b as usize);
    (block, if f.path.0.len() == 1 { f.off } else { 0 })
}

/// The page the caret is on, the start of that page, and every page's label.
fn caret_page(s: &Session) -> (usize, Option<(usize, usize)>, Vec<String>) {
    let pages = planned_pages(&s.doc);
    let labels = labels_or_numbers(&s.doc, &pages);
    let here = caret_at(s);
    let index = pages.iter().enumerate().filter(|(_, pg)| page_start(pg).is_some_and(|st| st <= here)).map(|(i, _)| i).next_back().unwrap_or(0);
    let start = pages.get(index).and_then(|p| page_start(p));
    (index, start, labels)
}

/// The lock that starts the caret's page, if that page is locked.
fn caret_lock(s: &Session) -> Option<u32> {
    let (_, start, _) = caret_page(s);
    start.and_then(|st| known_marks(&s.doc).get(&st).copied())
}

/// `lock.relock`: unlock every page and lock them again as they are now (numbered 1, 2, 3).
fn relock(s: &mut Session, v: &Value) -> CmdResult {
    unlock_all(s, v)?;
    lock(s, v)
}

/// `lock.adjustBreak {by}`: move the caret page's locked start down (`by` 1) or up (-1) one
/// paragraph.
fn adjust_break(s: &mut Session, v: &Value) -> CmdResult {
    let by = match v.get("by") {
        None | Some(Value::Null) => 1,
        Some(b) => b.as_i64().filter(|b| *b == 1 || *b == -1).ok_or_else(|| CmdError::Params("`by` is 1 (down) or -1 (up)".into()))?,
    };
    let id = caret_lock(s).ok_or_else(|| CmdError::Failed("the caret's page is not a locked page".into()))?;
    let (block, _) = lock_marks(&s.doc).get(&id).copied().ok_or_else(|| CmdError::Failed("the locked page's start is gone".into()))?;
    // The next (or previous) paragraph with text.
    let mut to = block;
    loop {
        to = if by > 0 {
            to + 1
        } else {
            to.checked_sub(1).ok_or_else(|| CmdError::Failed("there is no paragraph to move the page's start to".into()))?
        };
        if to >= s.doc.body.len() {
            return Err(CmdError::Failed("there is no paragraph to move the page's start to".into()));
        }
        if s.doc.body.get(to).and_then(|b| b.as_para()).is_some_and(|p| !p.text.is_empty()) {
            break;
        }
    }
    if lock_marks(&s.doc).values().any(|(b, _)| *b == to) {
        return Err(CmdError::Failed("that paragraph already starts a locked page".into()));
    }
    remove_marks(&mut s.doc, &|x| x == id)?;
    put_mark(&mut s.doc, id, to, 0)?;
    Ok(json!({"block": to}))
}

/// `lock.setLabel {label}`: change the caret's locked page's label.
fn set_label(s: &mut Session, v: &Value) -> CmdResult {
    let label: String =
        v.get("label").and_then(Value::as_str).unwrap_or("").chars().filter(|c| c.is_ascii_alphanumeric()).take(MAX_LABEL_CHARS).collect();
    if label.is_empty() {
        return Err(CmdError::Params("`label` is letters and digits, such as 12 or 12A".into()));
    }
    let id = caret_lock(s).ok_or_else(|| CmdError::Failed("the caret's page is not a locked page".into()))?;
    let mut locks = Locks::of_doc(&s.doc);
    if locks.pages.iter().any(|l| l.id != id && l.label.eq_ignore_ascii_case(&label)) {
        return Err(CmdError::Params(format!("another locked page is already {label}")));
    }
    if let Some(l) = locks.pages.iter_mut().find(|l| l.id == id) {
        l.label.clone_from(&label);
    }
    locks.store(&mut s.doc)?;
    Ok(json!({"label": label}))
}

/// `lock.lockPage`: start a new locked page at the caret's paragraph, lettered after the page
/// it is on (12A, or the next letters free).
fn lock_page(s: &mut Session, _: &Value) -> CmdResult {
    let mut locks = Locks::of_doc(&s.doc);
    if !locks.is_locked() {
        return Err(CmdError::Failed("lock the pages first".into()));
    }
    let (index, _, labels) = caret_page(s);
    let (block, _) = caret_at(s);
    if lock_marks(&s.doc).values().any(|(b, _)| *b == block) {
        return Err(CmdError::Failed("this paragraph already starts a locked page".into()));
    }
    let base = labels.get(index).cloned().unwrap_or_default();
    let used: Vec<&String> = labels.iter().chain(locks.pages.iter().map(|l| &l.label)).collect();
    let label = (1..10_000).map(|n| format!("{base}{}", letters(n))).find(|l| !used.contains(&l)).unwrap_or_default();
    let id = next_id(locks.pages.iter().map(|l| l.id).chain(lock_marks(&s.doc).keys().copied()));
    put_mark(&mut s.doc, id, block, 0)?;
    let at = locks.pages.iter().position(|l| l.label == base).map_or(locks.pages.len(), |i| i + 1);
    locks.pages.insert(at, Lock { id, label: label.clone(), level: locks.level().max(1) });
    locks.store(&mut s.doc)?;
    Ok(json!({"label": label}))
}

/// `lock.unlockPage`: unlock the caret's page, so its text flows back onto the page before.
fn unlock_page(s: &mut Session, _: &Value) -> CmdResult {
    let id = caret_lock(s).ok_or_else(|| CmdError::Failed("the caret's page is not a locked page".into()))?;
    let mut locks = Locks::of_doc(&s.doc);
    if locks.pages.first().is_some_and(|l| l.id == id) {
        return Err(CmdError::Failed("the first locked page stays locked; use Unlock All Pages".into()));
    }
    remove_marks(&mut s.doc, &|x| x == id)?;
    locks.pages.retain(|l| l.id != id);
    locks.store(&mut s.doc)?;
    Ok(Value::Null)
}

fn is_locked(s: &Session) -> Option<&'static str> {
    if Locks::of_doc(&s.doc).is_locked() { None } else { Some("the pages are not locked") }
}

/// Whether `p` starts with a lock mark.
pub fn starts_locked_page(p: &Paragraph) -> bool {
    p.runs.first().is_some_and(|r| PAGE_LOCK.is_set(&r.props))
}

/// The page-lock commands.
pub fn commands() -> Vec<CommandSpec> {
    const LOC: &str = "Layout › Page Locks";
    vec![
        CommandSpec::new("lock.pages", "Lock Pages", LOC, lock).params(r#"{"revisionMode"?: bool}"#),
        CommandSpec::new("lock.unlockLevel", "Unlock One Level", LOC, unlock_level).when(is_locked),
        CommandSpec::new("lock.unlockAll", "Unlock All Pages", LOC, unlock_all).when(is_locked),
        CommandSpec::new("lock.status", "Page Lock Status", LOC, status).pure(),
        CommandSpec::new("lock.relock", "Relock Pages", LOC, relock).when(is_locked),
        CommandSpec::new("lock.adjustBreak", "Move Page Start", LOC, adjust_break).params(r#"{"by": 1 | -1}"#).when(is_locked),
        CommandSpec::new("lock.setLabel", "Edit Page Label", LOC, set_label).params(r#"{"label": string}"#).when(is_locked),
        CommandSpec::new("lock.lockPage", "Lock New Page Here", LOC, lock_page).when(is_locked),
        CommandSpec::new("lock.unlockPage", "Unlock This Page", LOC, unlock_page).when(is_locked),
    ]
}

#[cfg(test)]
mod tests;
