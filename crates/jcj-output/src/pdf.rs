// SPDX-License-Identifier: MIT OR Apache-2.0
//! PDF export.

use std::borrow::Cow;

use jcj_doc::Document;
use jcj_layout::DocLayout;
use jcj_upstream::pdf::{PdfError, PdfOptions};

/// Why a PDF could not be written.
#[derive(Clone, Debug, PartialEq)]
pub enum OutputError {
    /// The layout has no pages (or the page list chose none).
    NoPages,
    /// A page asked for does not exist (1-based, as people count pages).
    BadPage(usize),
    /// Writing failed.
    Write(String),
}

impl std::fmt::Display for OutputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputError::NoPages => f.write_str("there are no pages to export"),
            OutputError::BadPage(n) => write!(f, "page {n} does not exist"),
            OutputError::Write(e) => write!(f, "writing the PDF failed: {e}"),
        }
    }
}

impl std::error::Error for OutputError {}

impl From<PdfError> for OutputError {
    fn from(e: PdfError) -> Self {
        match e {
            PdfError::NoPages => OutputError::NoPages,
            PdfError::BadPage(n) => OutputError::BadPage(n),
            PdfError::Write(s) => OutputError::Write(s),
        }
    }
}

/// How a PDF is written.
#[derive(Clone, Debug, PartialEq)]
pub struct PdfSettings {
    /// The pages to write (0-based layout page indexes, in output order); `None` writes all.
    pub pages: Option<Vec<usize>>,
    /// Title and author written into the file; `None` takes the document's properties.
    pub title: Option<String>,
    pub author: Option<String>,
    /// The creation date written into the file, ISO 8601 (`2026-01-31T09:00:00`). `None` takes
    /// the document's own (`core.created`), or the current time when it has none. Fix it for
    /// output that must be the same every time.
    pub created: Option<String>,
    /// Write a structure tree for assistive technology (tagged PDF).
    pub tagged: bool,
    /// Compress the page contents.
    pub compress: bool,
    /// Make links clickable. Off for printing: printers ignore them.
    pub links: bool,
    /// Show tracked changes as markup instead of the final text.
    pub markup: bool,
}

impl Default for PdfSettings {
    fn default() -> Self {
        PdfSettings { pages: None, title: None, author: None, created: None, tagged: true, compress: true, links: true, markup: false }
    }
}

impl PdfSettings {
    /// The settings for golden files and tests: every value fixed, the creation date included.
    pub fn deterministic() -> Self {
        PdfSettings { created: Some("2000-01-01T00:00:00".into()), ..Default::default() }
    }

    fn options(&self) -> PdfOptions {
        PdfOptions {
            pages: self.pages.clone(),
            title: self.title.clone(),
            author: self.author.clone(),
            tagged: self.tagged,
            include_markup: self.markup,
            compress: self.compress,
            links: self.links,
            ..Default::default()
        }
    }

    /// `doc` with the creation date these settings fix.
    fn dated<'a>(&self, doc: &'a Document) -> Cow<'a, Document> {
        match &self.created {
            Some(c) if *c != doc.core.created => {
                let mut d = doc.clone();
                d.core.created.clone_from(c);
                Cow::Owned(d)
            }
            _ => Cow::Borrowed(doc),
        }
    }

    /// The layout options PDF export lays a document out with for these settings.
    pub fn layout_options(&self) -> jcj_layout::LayoutOptions {
        self.options().layout_options()
    }
}

/// Write `layout` (a print layout of `doc`) as PDF.
pub fn export_pdf(doc: &Document, layout: &DocLayout, settings: &PdfSettings) -> Result<Vec<u8>, OutputError> {
    Ok(jcj_upstream::pdf::export_layout(&settings.dated(doc), layout, &settings.options())?)
}

/// Lay `doc` out for PDF ([`PdfSettings::layout_options`]) and write it.
pub fn export_pdf_with_layout(doc: &Document, settings: &PdfSettings) -> Result<Vec<u8>, OutputError> {
    Ok(jcj_upstream::pdf::export(&settings.dated(doc), &settings.options())?)
}
