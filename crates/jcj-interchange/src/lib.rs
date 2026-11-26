// SPDX-License-Identifier: MIT OR Apache-2.0
//! Format import and export shared by every foundation app: DOCX, ODT, RTF, Markdown, HTML,
//! plain text and EPUB.
//!
//! One small interface: a [`Format`], found from a file name ([`Format::from_extension`]) or its
//! content ([`Format::sniff`], [`Format::detect`]); [`import`] bytes into a document and
//! [`export`] a document to bytes. Inputs are bounded ([`MAX_IMPORT_BYTES`]) and every failure is
//! an [`Error`] value.
//!
//! ```
//! use jcj_interchange::{Format, export, import};
//!
//! let doc = import(Format::Markdown, b"# Title\n\nSome *words*.").unwrap();
//! let html = export(Format::Html, &doc).unwrap();
//! assert!(String::from_utf8(html).unwrap().contains("<em>words</em>"));
//! assert_eq!(Format::detect(Some("notes.md"), b"# Title"), Some(Format::Markdown));
//! ```
//!
//! **Where the work is done.** The plan names craft-libs' format crates (`craft-docx`,
//! `craft-odf`, `craft-rtf`, `craft-markdown`, `craft-html`, `craft-epub`). At the craft-libs
//! commits our WordCraft release pins, those crates do not exist; WordCraft does the same work
//! in its own crates, which this crate wraps through `jcj-upstream`:
//!
//! | Format | Import | Export |
//! | --- | --- | --- |
//! | DOCX | `wordcraft-docx` | `wordcraft-docx` |
//! | ODT, RTF, Markdown, HTML, text | `wordcraft-formats` | `wordcraft-formats` |
//! | EPUB 3 | this crate's own reader ([`epub_read`]) | this crate's own writer |
//!
//! When craft-libs' crates reach our pins, they replace these behind the same interface.
//!
//! What each format keeps is what the upstream crate keeps; extension data
//! ([`jcj_doc::ExtKey`]) survives only in the native JSON a project stores, never in these
//! formats. [`emphasis`] adds a small inline markup (`**bold**`, `*italic*`, `_underline_`) for
//! places that hold one line of formatted text as plain text.
//!
//! [`export_epub`] is the EPUB writer with choices ([`EpubOptions`]): how notes are numbered,
//! what the link back from a note says, a heading above them; it also tells the words it added.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod emphasis;
mod epub;
pub mod epub_read;
mod format;
mod rtf_direct;

pub use epub::{EpubExport, EpubOptions, NoteBacklink, NoteNumbering};
pub use format::Format;

use std::panic::{AssertUnwindSafe, catch_unwind};

use jcj_doc::Document;
use jcj_upstream::{docx, formats};

/// The largest input [`import`] reads, bytes.
pub const MAX_IMPORT_BYTES: usize = 256 << 20;

/// Which way a conversion goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Import,
    Export,
}

/// Why a conversion failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The input is larger than [`MAX_IMPORT_BYTES`].
    #[error("the file is larger than {} MB", limit >> 20)]
    TooLarge { size: usize, limit: usize },
    /// This format cannot be converted this way (EPUB import).
    #[error("{} files cannot be {} yet", format.label(), match direction { Direction::Import => "imported", Direction::Export => "exported" })]
    Unsupported { format: Format, direction: Direction },
    /// The input is not a readable file of the format.
    #[error("the {} file could not be read: {reason}", format.label())]
    Malformed { format: Format, reason: String },
    /// The document could not be written in the format.
    #[error("the document could not be saved as {}: {reason}", format.label())]
    Failed { format: Format, reason: String },
}

/// Run an upstream conversion; a panic inside it (never expected) becomes `on_panic`.
fn guarded<T>(f: impl FnOnce() -> Result<T, Error>, on_panic: Error) -> Result<T, Error> {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(Err(on_panic))
}

/// Read `bytes` as `format` into a new document.
pub fn import(format: Format, bytes: &[u8]) -> Result<Document, Error> {
    if bytes.len() > MAX_IMPORT_BYTES {
        return Err(Error::TooLarge { size: bytes.len(), limit: MAX_IMPORT_BYTES });
    }
    let malformed = |reason: String| Error::Malformed { format, reason };
    let mut doc = guarded(
        || match format {
            Format::Epub => epub_read::read(bytes).map_err(malformed),
            Format::Docx => docx::read(bytes).map_err(|e| malformed(e.to_string())),
            other => match formats::import(other.extension(), bytes) {
                Some(r) => r.map_err(malformed),
                None => Err(Error::Unsupported { format, direction: Direction::Import }),
            },
        },
        malformed("the reader stopped unexpectedly".into()),
    )?;
    doc.ensure_nonempty();
    Ok(doc)
}

/// Write `doc` as `format`.
pub fn export(format: Format, doc: &Document) -> Result<Vec<u8>, Error> {
    let failed = |reason: String| Error::Failed { format, reason };
    guarded(
        || match format {
            Format::Epub => epub::write(doc, &EpubOptions::default()).map(|e| e.bytes).map_err(failed),
            Format::Docx => docx::write(doc).map_err(|e| failed(e.to_string())),
            other => match formats::export(other.extension(), doc) {
                Some(r) => r.map_err(failed),
                None => Err(Error::Unsupported { format, direction: Direction::Export }),
            },
        },
        failed("the writer stopped unexpectedly".into()),
    )
}

/// Write `doc` as an EPUB with choices about its notes, and learn the words the writer added.
///
/// [`export`] with [`Format::Epub`] numbers notes from 1 in each chapter file and puts the
/// English word "Back" under each as the link to its reference. Here the caller chooses the
/// numbering ([`NoteNumbering`]), the link ([`NoteBacklink`]: its text in the book's language, a
/// symbol with a name, or none) and a heading above each chapter's notes, and gets back
/// [`EpubExport::added_words`], so a word-for-word check of the file can leave out exactly the
/// words that are the writer's. With `EpubOptions::default()` the bytes are [`export`]'s.
///
/// ```
/// use jcj_interchange::{EpubOptions, Format, NoteBacklink, export, export_epub};
///
/// let doc = jcj_interchange::import(Format::Markdown, b"# One\n\nWords.").unwrap();
/// let same = export_epub(&doc, &EpubOptions::default()).unwrap();
/// assert_eq!(same.bytes, export(Format::Epub, &doc).unwrap());
/// let quiet = export_epub(&doc, &EpubOptions::default().note_backlink(NoteBacklink::None)).unwrap();
/// assert!(quiet.added_words.is_empty(), "a book with no notes gets no added words");
/// ```
pub fn export_epub(doc: &Document, options: &EpubOptions) -> Result<EpubExport, Error> {
    let failed = |reason: String| Error::Failed { format: Format::Epub, reason };
    guarded(|| epub::write(doc, options).map_err(failed), failed("the writer stopped unexpectedly".into()))
}

/// Write `doc` as RTF with each paragraph's formatting written out in full, for apps that lay out
/// their own paragraphs (each kind of paragraph at its own indent).
///
/// [`export`] with [`Format::Rtf`] writes paragraph kinds (headings, quotes, lists) and leaves
/// indents and fonts to the reader's defaults. This writes, for every body paragraph, its
/// resolved alignment, left, right and first-line indents, space before and after, and page
/// break before; and for every run its resolved font, size, bold, italic and underline (any
/// underline is written as a single one). The page size and margins are the last section's.
/// Tables are written as their cells' paragraphs, one after another. No styles, lists, fields,
/// pictures or other objects are written.
pub fn export_rtf_direct(doc: &Document) -> Result<Vec<u8>, Error> {
    let failed = |reason: String| Error::Failed { format: Format::Rtf, reason };
    guarded(|| Ok(rtf_direct::write(doc).into_bytes()), failed("the writer stopped unexpectedly".into()))
}
