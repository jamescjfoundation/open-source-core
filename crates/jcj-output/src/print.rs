// SPDX-License-Identifier: MIT OR Apache-2.0
//! Printing through the system print spooler: CUPS on macOS and Linux, the Windows spooler on
//! Windows. A job is a print-ready PDF and the job's options. Other platforms report that
//! printing is not available; the PDF can still be saved.

use jcj_doc::Document;
use jcj_layout::DocLayout;

use crate::pdf::{OutputError, PdfSettings, export_pdf};

pub use jcj_upstream::print::{Duplex, Error as PrintError, Job, Printer, Scaling};

/// Whether this platform has a print spooler this crate can use.
pub fn available() -> bool {
    jcj_upstream::print::backend().is_some()
}

/// The printers the system knows, the default one marked.
pub fn printers() -> Vec<Printer> {
    jcj_upstream::print::printers()
}

/// Send `pdf` to the spooler as `job`. Returns the spooler's job id.
pub fn submit(pdf: &[u8], job: &Job) -> Result<String, PrintError> {
    jcj_upstream::print::submit(pdf, job)
}

/// What [`submit`] would run, for diagnostics (no job is made).
pub fn preview(job: &Job) -> Result<Vec<String>, PrintError> {
    jcj_upstream::print::command_preview(job)
}

/// Why printing failed.
#[derive(Debug)]
pub enum PrintFailure {
    /// The PDF could not be made.
    Pdf(OutputError),
    /// The spooler refused the job.
    Spool(PrintError),
}

impl std::fmt::Display for PrintFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrintFailure::Pdf(e) => e.fmt(f),
            PrintFailure::Spool(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for PrintFailure {}

/// Print `layout` of `doc`: write it as a print PDF (no link annotations, which printers cannot
/// use) and send it to the spooler.
pub fn print_layout(doc: &Document, layout: &DocLayout, pdf: &PdfSettings, job: &Job) -> Result<String, PrintFailure> {
    let settings = PdfSettings { links: false, tagged: false, ..pdf.clone() };
    let bytes = export_pdf(doc, layout, &settings).map_err(PrintFailure::Pdf)?;
    submit(&bytes, job).map_err(PrintFailure::Spool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_preview_names_the_options_or_says_printing_is_unavailable() {
        let job = Job { copies: 2, title: "Test".into(), ..Default::default() };
        match preview(&job) {
            Ok(args) => assert!(!args.is_empty()),
            Err(e) => assert!(!available() && !e.to_string().is_empty()),
        }
    }
}
