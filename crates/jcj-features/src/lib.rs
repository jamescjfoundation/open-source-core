// SPDX-License-Identifier: MIT OR Apache-2.0
//! Document features every foundation app shares.
//!
//! Each module is one feature, with its data, its commands and its tests:
//!
//! | Module | What | Where its data lives |
//! | --- | --- | --- |
//! | [`bookmarks`] | Named places to go to; labels | Paragraph `core.bookmark`, document `core.bookmarks` |
//! | [`notes`] | Notes attached to text; note types; detached notes | Run mark `core.note`, document `core.notes`, `core.note_types` |
//! | [`revisions`] | Revision mode, revision sets (name, color, text styles), the color cycle | Run and paragraph mark `core.revised`, document `core.revisions` |
//! | [`tracking`] | Tracked changes: list, step, accept, reject; writers' colors | The model's own tracked changes |
//! | [`locks`] | Page locking and page labels (12, 12A); a pagination pass | Run mark `core.page_lock`, document `core.page_locks` |
//! | [`watermark`] | Diagonal text behind every page; a name on every page | Document `core.watermark` |
//! | [`header_footer`] | Header and footer bands with fields; app fields | Document `core.header_footer` |
//! | [`compare`] | Comparing two versions into tracked changes | — |
//!
//! and the pieces they share: [`data`] (settings groups in extension data), [`dates`],
//! [`marks`] (run marks), [`pages`] (the pages commands see) and [`editing`] (typing that keeps
//! the marks right).
//!
//! **Data.** Everything is kept in the model's extension data (decision 0006) under `core.`
//! keys, typed with [`jcj_doc::ExtKey`]: it saves, copies and undoes with the document. A value
//! that does not read gives the feature's defaults, so a damaged file still opens. Run data is a
//! mark that moves with its text; paragraph data stays on its paragraph.
//!
//! **Commands.** [`commands`] is every feature's commands; an app passes them, with its own, to
//! [`jcj_engine::registry_with`]. Each module's `commands()` gives one feature's. The list
//! includes [`editing::commands`], which take the engine's `text.insert`, `text.newParagraph`,
//! `text.backspace` and `text.delete` over (same labels and keys) to keep notes, revision marks,
//! page locks and bookmarks right while typing.
//!
//! **Pages.** Page locks need pagination: an app composes [`locks::LockPass`] with its own
//! [`jcj_layout::paginate::PaginationPass`], and tells the commands how it plans pages with
//! [`pages::set_pages`]. [`watermark::apply`], [`header_footer::apply`] and
//! [`revisions::decorate_output`] work on the paginated output (a pass's `finish`).
//!
//! The features came from the first foundation app; decision 0010 records what came from where.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod bookmarks;
pub mod compare;
pub mod data;
pub mod dates;
pub mod editing;
pub mod header_footer;
pub mod locks;
pub mod marks;
pub mod notes;
pub mod pages;
pub mod revisions;
pub mod tracking;
pub mod watermark;

use jcj_engine::CommandSpec;

/// Every extension key the features use, in one place.
pub mod keys {
    pub use crate::bookmarks::{BOOKMARK, BOOKMARKS};
    pub use crate::header_footer::HEADER_FOOTER;
    pub use crate::locks::{LOCKS, PAGE_LOCK};
    pub use crate::notes::{NOTE, NOTE_TYPES, NOTES};
    pub use crate::revisions::{REVISED, REVISIONS};
    pub use crate::watermark::WATERMARK;
}

/// Every feature's commands, the typing commands of [`editing`] included. Pass them to
/// [`jcj_engine::registry_with`] with the app's own.
pub fn commands() -> Vec<CommandSpec> {
    let mut all = Vec::new();
    all.extend(editing::commands());
    all.extend(bookmarks::commands());
    all.extend(notes::commands());
    all.extend(revisions::commands());
    all.extend(tracking::commands());
    all.extend(locks::commands());
    all.extend(watermark::commands());
    all.extend(header_footer::commands());
    all
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Arc;

    use jcj_doc::props::CharProps;
    use jcj_doc::{Document, Paragraph, Path, para_block};
    use jcj_engine::{Selection, Session, registry_with, session_with as engine_session};

    use crate::data::pos_at;

    /// A session on a document of one paragraph per line, with the features' commands.
    pub fn session_with(lines: &[&str]) -> Session {
        let mut doc = Document::new();
        doc.body = lines.iter().map(|t| para_block(Paragraph::with_text(t, CharProps::default()))).collect();
        engine_session(doc, Arc::new(registry_with(super::commands())))
    }

    /// Each top-level paragraph's text.
    pub fn lines(s: &Session) -> Vec<String> {
        s.doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.text.clone()).collect()
    }

    /// Select bytes `a..b` of top-level paragraph `block`.
    pub fn select(s: &mut Session, block: usize, a: usize, b: usize) {
        s.sel = Selection { anchor: pos_at(&Path::top(block), a), focus: pos_at(&Path::top(block), b) };
    }

    #[test]
    fn command_ids_are_unique_and_namespaced() {
        let all = super::commands();
        assert!(jcj_engine::duplicate_ids(&all).is_empty(), "{:?}", jcj_engine::duplicate_ids(&all));
        assert!(all.iter().all(|c| c.id.contains('.')));
        assert!(jcj_engine::try_registry_with(all).is_ok());
    }

    #[test]
    fn every_key_is_in_the_core_namespace() {
        use super::keys::*;
        let ks = [
            BOOKMARK.key(),
            BOOKMARKS.key(),
            HEADER_FOOTER.key(),
            LOCKS.key(),
            PAGE_LOCK.key(),
            NOTE.key(),
            NOTE_TYPES.key(),
            NOTES.key(),
            REVISED.key(),
            REVISIONS.key(),
            WATERMARK.key(),
        ];
        assert!(ks.iter().all(|k| k.starts_with("core.") && jcj_doc::valid_ext_key(k).is_ok()));
    }

    #[test]
    fn feature_data_saves_and_reopens() {
        let mut s = session_with(&["One two.", "Three."]);
        select(&mut s, 0, 0, 3);
        s.run("note.add", &serde_json::json!({"text": "A note."})).unwrap();
        s.run("bookmark.add", &serde_json::json!({"name": "start"})).unwrap();
        s.run("revision.markRevised", &serde_json::json!({})).unwrap();
        let bytes = jcj_engine::native::save(&s.doc).unwrap();
        let back = jcj_engine::native::open(&bytes).unwrap();
        assert_eq!(crate::notes::Notes::of_doc(&back).notes.len(), 1);
        assert_eq!(crate::bookmarks::placed(&back).len(), 1);
        assert_eq!(crate::notes::anchors(&back).values().next().map(|a| a.quote.clone()).as_deref(), Some("One"));
        assert_eq!(crate::revisions::sets_in_use(&back).len(), 1);
    }
}
