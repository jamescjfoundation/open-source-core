// SPDX-License-Identifier: MIT OR Apache-2.0
//! Output: PDF, page images and printing.
//!
//! - [`pdf`]: write a layout ([`jcj_layout::DocLayout`]) as PDF. The same document, layout and
//!   settings give the same bytes every time when the settings fix the creation date
//!   ([`PdfSettings::created`]): golden files rely on it.
//! - [`image`]: draw a page, or part of one, into an RGBA image (previews, thumbnails, tests).
//! - [`read`]: read a PDF's text back, with where each glyph lands (importing a PDF, checking an
//!   exported one in tests).
//! - [`facts`]: a PDF's page boxes, annotations, images and their resolution, fonts and whether
//!   each is embedded, and bookmarks (preflight before sending a PDF to print).
//! - `print` (Cargo feature `print`): send a PDF to the system print spooler.
//!
//! Wraps WordCraft's `wordcraft-pdf` and `wordcraft-render`, and craft-libs' `craft-pdf` and
//! `craft-print`.
//!
//! **PDF/X is not offered yet.** No crate at our pins writes PDF/X: craft-libs' `craft-prepress`
//! (PDF/X-1a) is not among the pinned crates. It joins here, behind this interface, when an app
//! needs print-ready files (docs/UPSTREAM.md).

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod facts;
pub mod image;
pub mod pdf;
#[cfg(feature = "print")]
pub mod print;
pub mod read;

pub use facts::{PageBox, PageFacts, PdfFacts, PdfFont, PdfImage, read_facts};
pub use image::{PageImage, RenderSettings, render_area, render_page};
pub use pdf::{OutputError, PdfSettings, export_pdf, export_pdf_with_layout};
pub use read::{GlyphMode, PdfGlyph, PdfPage, ReadError, ReadLimits, read_pages};

#[cfg(test)]
mod tests;
