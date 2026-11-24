// SPDX-License-Identifier: MIT OR Apache-2.0
//! Output: PDF, page images and printing.
//!
//! - [`pdf`]: write a layout ([`jcj_layout::DocLayout`]) as PDF. The same document, layout and
//!   settings give the same bytes every time when the settings fix the creation date
//!   ([`PdfSettings::created`]): golden files rely on it.
//! - [`image`]: draw a page, or part of one, into an RGBA image (previews, thumbnails, tests).
//! - `print` (Cargo feature `print`): send a PDF to the system print spooler.
//!
//! Wraps WordCraft's `wordcraft-pdf` and `wordcraft-render`, and craft-libs' `craft-print`.
//!
//! **PDF/X is not offered yet.** No crate at our pins writes PDF/X: craft-libs' `craft-prepress`
//! (PDF/X-1a) is not among the pinned crates. It joins here, behind this interface, when an app
//! needs print-ready files (docs/UPSTREAM.md).

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod image;
pub mod pdf;
#[cfg(feature = "print")]
pub mod print;

pub use image::{PageImage, RenderSettings, render_area, render_page};
pub use pdf::{OutputError, PdfSettings, export_pdf, export_pdf_with_layout};

#[cfg(test)]
mod tests;
