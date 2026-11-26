// SPDX-License-Identifier: MIT OR Apache-2.0
//! The document model interface for foundation apps.
//!
//! Apps name document types through this crate, never through WordCraft (decision 0005). Today
//! the model *is* WordCraft's `wordcraft-doc` at the pinned release, re-exported item by item:
//! a thin adapter on purpose. When upstream renames or moves something, this crate keeps the old
//! name working (a type alias, a wrapper function) and the apps do not change. If the model ever
//! moves to `craft-wordmodel`, this is the only crate that changes.
//!
//! What this crate adds of its own is [typed extension data](ExtKey): each app attaches its own
//! values to documents, paragraphs and runs (a screenplay's scene numbers, a manuscript's status
//! labels, a book's chapter roles) under its own namespace. The model carries, copies, compares,
//! undoes and saves them without understanding them. The carrier, [`Ext`], is the extension-data
//! patch in our WordCraft fork (docs/UPSTREAM.md).
//!
//! It also holds two things about a text's words that need the model and nothing else, so that a
//! crate below the engine and the layout can use them: the word list and the word-for-word check
//! ([`words`], [`verify`]) and the word-level comparison of two versions ([`wordiff`]).

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod ext;
pub mod verify;
pub mod walk;
pub mod wordiff;

pub use ext::{ExtError, ExtKey, HasExt};
pub use verify::words;

use jcj_upstream::doc as up;

// The model. Each name is part of the core's interface: removing or changing one is a breaking
// change (a minor release while at 0.x) and is listed in CHANGELOG.md under "What apps must do".
pub use up::{
    Align, Block, Blocks, BuildingBlock, CharProps, Comment, CoreProps, CustomProp, DocError, Document, Ext, ExtKeyError, InlineObject, ListKind,
    MAX_EXT_KEY_LEN, Numbering, ParaProps, Paragraph, Part, PartKind, Path, Pos, Result, Revision, RevisionKind, Rgb, Run, SectionProps, Settings,
    StoryRef, Style, StyleKind, StyleSheet, TextColor, Watermark, count_chars, count_words, para_block, valid_ext_key,
};

/// Paragraph and character properties, page and border types.
pub mod props {
    pub use jcj_upstream::doc::props::{
        Align, Border, BorderStyle, Borders, CharProps, Highlight, LineSpacing, ParaProps, Rgb, TabAlign, TabLeader, TabStop, TextColor, Underline,
        VertAlign,
    };
}

/// Section settings: how a section starts (next page, odd or even page), its page numbers, its
/// header and footer stories, columns and note settings. [`SectionProps`] holds them.
pub mod section {
    pub use jcj_upstream::doc::section::{
        ChapterSep, Columns, DocGrid, DocGridType, HeaderSet, LineNumberRestart, LineNumbering, NoteOptions, NotePos, NoteProps, NoteRestart,
        NumFormat, PageVAlign, SectionStart,
    };
}

/// Inline objects and floating shapes inside paragraphs.
pub mod para {
    pub use jcj_upstream::doc::para::{Anchor, Float, InlineObject, NoteKind, OBJ, Paragraph, Run, ShapeKind, Wrap};
}

/// Shape fills.
pub mod fill {
    pub use jcj_upstream::doc::fill::Fill;
}

/// Freeform (drawn) shapes.
pub mod freeform {
    pub use jcj_upstream::doc::freeform::{FreePath, Freeform, InkTool};
}

/// Text wrapping around shapes.
pub mod wrap {
    pub use jcj_upstream::doc::wrap::arrow_outline;
}

/// Tables.
pub mod table {
    pub use jcj_upstream::doc::table::{Cell, Row, Table};
}

/// Word and character classes for editing (word boundaries in any script).
pub mod text {
    pub use jcj_upstream::doc::bidi::is_word_char;
}

/// A new, empty document: one empty paragraph in the Normal style, Letter page, built-in styles.
pub fn new_document() -> Document {
    Document::new()
}

/// The plain text of a paragraph: what the user typed, without inline objects.
pub fn plain_text(p: &Paragraph) -> String {
    p.plain_text()
}
