// SPDX-License-Identifier: MIT OR Apache-2.0
//! Trim and bleed boxes on an exported PDF: where the paper is cut, and how far printing runs
//! past the cut on a page that bleeds (print services read both).
//!
//! [`set_page_boxes`] adds them as an incremental update: the original bytes stay as they are,
//! and each page's dictionary is written again after them with `/TrimBox` and `/BleedBox`, with a
//! cross-reference section and trailer pointing back to the original's. The same input gives the
//! same output. Files are not trusted: the input is bounded ([`ReadLimits`]), and an encrypted
//! file, a cross-reference stream, or a page that already has the boxes is refused, never
//! damaged.

use std::fmt::Write as _;

use jcj_upstream::pdf_read::{OpenOptions, open_slice_with_options};

use crate::facts::PageBox;
use crate::read::{ReadError, ReadLimits};

/// The boxes one page gets, in PDF units from the page's bottom left (within its media box).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Boxes {
    /// The trim box: the finished page after cutting.
    pub trim: PageBox,
    /// The bleed box, when the page bleeds (usually the media box).
    pub bleed: Option<PageBox>,
}

/// Why the boxes could not be set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoxError {
    /// The PDF could not be read.
    Read(ReadError),
    /// The number of boxes given is not the number of pages (pages, boxes).
    Count(usize, usize),
    /// The PDF is encrypted, or keeps its cross-references in a stream; the boxes are not set.
    Unsupported(&'static str),
    /// A page (from 1) already has a trim or bleed box.
    HasBoxes(usize),
}

impl std::fmt::Display for BoxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BoxError::Read(e) => e.fmt(f),
            BoxError::Count(p, b) => write!(f, "the PDF has {p} pages but {b} sets of boxes were given"),
            BoxError::Unsupported(why) => write!(f, "the page boxes cannot be set: {why}"),
            BoxError::HasBoxes(n) => write!(f, "page {n} already has a trim or bleed box"),
        }
    }
}

impl std::error::Error for BoxError {}

fn rect(b: &PageBox) -> String {
    let n = |v: f64| {
        let s = format!("{v:.4}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        if s == "-0" { "0".to_owned() } else { s.to_owned() }
    };
    format!("[{} {} {} {}]", n(b.x0), n(b.y0), n(b.x1), n(b.y1))
}

/// The last `startxref` offset and the trailer dictionary's text before it.
fn trailer(bytes: &[u8]) -> Option<(usize, String)> {
    let tail_from = bytes.len().saturating_sub(4096);
    let tail = &bytes[tail_from..];
    let sx = tail.windows(9).rposition(|w| w == b"startxref")?;
    let after = std::str::from_utf8(&tail[sx + 9..]).ok()?;
    let offset: usize = after.split_whitespace().next()?.parse().ok()?;
    let before = &bytes[..tail_from + sx];
    let t = before.windows(7).rposition(|w| w == b"trailer")?;
    let text = std::str::from_utf8(&before[t + 7..]).ok()?.trim().to_owned();
    Some((offset, text))
}

/// The trailer's entries without `/Prev` (its number dropped too).
fn without_prev(dict: &str) -> Option<String> {
    let inner = dict.trim().strip_prefix("<<")?.strip_suffix(">>")?;
    match inner.find("/Prev") {
        None => Some(inner.trim().to_owned()),
        Some(i) => {
            let rest = inner[i + 5..].trim_start();
            let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
            Some(format!("{} {}", inner[..i].trim(), rest[end..].trim()).trim().to_owned())
        }
    }
}

/// Set each page's trim box and, when given, bleed box. `boxes` has one entry per page, in order.
pub fn set_page_boxes(bytes: &[u8], boxes: &[Boxes], limits: &ReadLimits) -> Result<Vec<u8>, BoxError> {
    if bytes.len() > limits.max_input_bytes {
        return Err(BoxError::Read(ReadError::TooLarge(limits.max_input_bytes)));
    }
    let options = OpenOptions { max_input_bytes: limits.max_input_bytes, ..OpenOptions::default() };
    let file = open_slice_with_options(bytes, None, &options).map_err(|e| BoxError::Read(ReadError::NotPdf(e.to_string())))?;
    let pages = file.pages();
    if pages.len() > limits.max_pages {
        return Err(BoxError::Read(ReadError::TooManyPages(limits.max_pages)));
    }
    if pages.len() != boxes.len() {
        return Err(BoxError::Count(pages.len(), boxes.len()));
    }
    let (prev, trailer_text) = trailer(bytes).ok_or(BoxError::Unsupported("the PDF's trailer could not be found"))?;
    if trailer_text.contains("/Encrypt") {
        return Err(BoxError::Unsupported("the PDF is encrypted"));
    }
    if !bytes.get(prev..).is_some_and(|b| b.starts_with(b"xref")) {
        return Err(BoxError::Unsupported("the PDF keeps its cross-references in a stream"));
    }
    let entries = without_prev(&trailer_text).ok_or(BoxError::Unsupported("the PDF's trailer could not be read"))?;
    let mut out = bytes.to_vec();
    if !out.ends_with(b"\n") {
        out.push(b'\n');
    }
    let mut written: Vec<(i32, i32, usize)> = Vec::with_capacity(pages.len());
    for (i, (page, b)) in pages.iter().zip(boxes).enumerate() {
        let dict = page.raw();
        if dict.contains_key(b"TrimBox") || dict.contains_key(b"BleedBox") {
            return Err(BoxError::HasBoxes(i + 1));
        }
        let id = dict.obj_id().ok_or(BoxError::Unsupported("a page is not an object of its own"))?;
        let data = dict.data();
        let close = data.windows(2).rposition(|w| w == b">>").ok_or(BoxError::Unsupported("a page's dictionary could not be read"))?;
        let mut added = format!(" /TrimBox {}", rect(&b.trim));
        if let Some(bleed) = &b.bleed {
            let _ = write!(added, " /BleedBox {}", rect(bleed));
        }
        written.push((id.obj_number, id.gen_number, out.len()));
        out.extend_from_slice(format!("{} {} obj\n", id.obj_number, id.gen_number).as_bytes());
        out.extend_from_slice(&data[..close]);
        out.extend_from_slice(added.as_bytes());
        out.extend_from_slice(b" >>\nendobj\n");
    }
    // The cross-reference section: one subsection per object, in object order.
    written.sort_by_key(|w| w.0);
    let xref = out.len();
    let mut x = String::from("xref\n");
    for (num, generation, off) in &written {
        let _ = write!(x, "{num} 1\n{off:010} {generation:05} n \n");
    }
    let _ = write!(x, "trailer\n<< {entries} /Prev {prev} >>\nstartxref\n{xref}\n%%EOF\n");
    out.extend_from_slice(x.as_bytes());
    Ok(out)
}
