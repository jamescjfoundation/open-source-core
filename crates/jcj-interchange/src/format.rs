// SPDX-License-Identifier: MIT OR Apache-2.0
//! The formats, and finding a file's format from its name or its content.

use std::io::{Cursor, Read};

/// A format documents are imported from and exported to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    /// Word documents (Office Open XML).
    Docx,
    /// OpenDocument Text.
    Odt,
    /// Rich Text Format.
    Rtf,
    /// Markdown (CommonMark with GFM tables and strikethrough).
    Markdown,
    /// Web pages.
    Html,
    /// Plain text.
    Text,
    /// EPUB 3 e-books (export only for now).
    Epub,
}

/// The most entries a zip may declare before sniffing looks inside it.
const MAX_SNIFF_ENTRIES: u64 = 20_000;
/// How much of a text file sniffing looks at.
const SNIFF_TEXT: usize = 64 << 10;

impl Format {
    /// Every format.
    pub const ALL: [Format; 7] = [Format::Docx, Format::Odt, Format::Rtf, Format::Markdown, Format::Html, Format::Text, Format::Epub];

    /// The extension written for this format (no dot).
    pub fn extension(self) -> &'static str {
        self.extensions()[0]
    }

    /// Every extension read as this format (no dot, lower case), the usual one first.
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Format::Docx => &["docx", "docm", "dotx", "dotm"],
            Format::Odt => &["odt", "ott"],
            Format::Rtf => &["rtf"],
            Format::Markdown => &["md", "markdown"],
            Format::Html => &["html", "htm", "xhtml"],
            Format::Text => &["txt", "text"],
            Format::Epub => &["epub"],
        }
    }

    /// A name for menus and file dialogs.
    pub fn label(self) -> &'static str {
        match self {
            Format::Docx => "Word Document (DOCX)",
            Format::Odt => "OpenDocument Text (ODT)",
            Format::Rtf => "Rich Text (RTF)",
            Format::Markdown => "Markdown",
            Format::Html => "Web Page (HTML)",
            Format::Text => "Plain Text",
            Format::Epub => "EPUB",
        }
    }

    /// The format's media type.
    pub fn mime(self) -> &'static str {
        match self {
            Format::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Format::Odt => "application/vnd.oasis.opendocument.text",
            Format::Rtf => "application/rtf",
            Format::Markdown => "text/markdown",
            Format::Html => "text/html",
            Format::Text => "text/plain",
            Format::Epub => "application/epub+zip",
        }
    }

    /// Whether apps should offer this format for import. EPUB says no so that apps' lists stay
    /// as they were: [`crate::import`] reads EPUB too ([`crate::epub_read`]), for apps that ask
    /// for it by name.
    pub fn can_import(self) -> bool {
        self != Format::Epub
    }

    /// Whether [`crate::export`] writes this format.
    pub fn can_export(self) -> bool {
        true
    }

    /// The format an extension names (with or without the dot, any case).
    pub fn from_extension(ext: &str) -> Option<Format> {
        let ext = ext.trim_start_matches('.').to_ascii_lowercase();
        Format::ALL.into_iter().find(|f| f.extensions().contains(&ext.as_str()))
    }

    /// The format a file name or path's extension names.
    pub fn from_name(name: &str) -> Option<Format> {
        let file = name.rsplit(['/', '\\']).next().unwrap_or(name);
        let (_, ext) = file.rsplit_once('.')?;
        Format::from_extension(ext)
    }

    /// The format the content shows: a zip package's own markers (EPUB and ODT `mimetype`,
    /// DOCX's `word/document.xml`), RTF's `{\rtf`, an HTML document's start, then Markdown or
    /// plain text by their look. `None` for content that is none of these (binary data).
    pub fn sniff(bytes: &[u8]) -> Option<Format> {
        if bytes.starts_with(b"PK\x03\x04") {
            return sniff_zip(bytes);
        }
        let head = text_head(bytes)?;
        let start = head.trim_start();
        if start.starts_with("{\\rtf") {
            return Some(Format::Rtf);
        }
        let lower: String = start.chars().take(1024).collect::<String>().to_ascii_lowercase();
        if lower.starts_with("<!doctype html") || lower.starts_with("<html") || (lower.starts_with("<?xml") && lower.contains("<html")) {
            return Some(Format::Html);
        }
        Some(if looks_like_markdown(&head) { Format::Markdown } else { Format::Text })
    }

    /// The format of a file named `name` (if known) with content `bytes`: the content when it is
    /// decisive (a package, RTF), else the name's extension, else the content's look.
    pub fn detect(name: Option<&str>, bytes: &[u8]) -> Option<Format> {
        let sniffed = Format::sniff(bytes);
        match sniffed {
            Some(Format::Docx | Format::Odt | Format::Epub | Format::Rtf) => sniffed,
            _ => name.and_then(Format::from_name).or(sniffed),
        }
    }
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// The start of `bytes` as text, `None` when it looks binary. UTF-16 with a byte order mark is
/// text (plain text import reads it).
fn text_head(bytes: &[u8]) -> Option<String> {
    let head = &bytes[..bytes.len().min(SNIFF_TEXT)];
    if head.starts_with(&[0xFF, 0xFE]) || head.starts_with(&[0xFE, 0xFF]) {
        return Some(String::new());
    }
    if head.contains(&0) {
        return None;
    }
    let head = head.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(head);
    Some(String::from_utf8_lossy(head).into_owned())
}

/// Markdown by its marks: headings, list items, fences, tables, links, strong emphasis.
fn looks_like_markdown(text: &str) -> bool {
    let mut marks = 0;
    for line in text.lines().take(2000) {
        let t = line.trim_start();
        let heading = t.len() > 1 && t.starts_with('#') && t.trim_start_matches('#').starts_with(' ') && t.find(' ').is_some_and(|n| n <= 6);
        let item = ["- ", "* ", "+ ", "> "].iter().any(|m| t.starts_with(m))
            || t.split_once(". ").is_some_and(|(n, _)| !n.is_empty() && n.len() <= 3 && n.bytes().all(|b| b.is_ascii_digit()));
        let table_row = t.starts_with('|') && t.trim_end().ends_with('|') && t.len() > 2;
        if heading || t.starts_with("```") || t.starts_with("~~~") {
            marks += 2;
        } else if item || table_row {
            marks += 1;
        }
        if t.contains("](") || t.contains("**") {
            marks += 1;
        }
        if marks >= 2 {
            return true;
        }
    }
    false
}

/// What a zip package is, by its `mimetype` entry or its parts.
fn sniff_zip(bytes: &[u8]) -> Option<Format> {
    jcj_upstream::doc::zip_guard::check(bytes, MAX_SNIFF_ENTRIES).ok()?;
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).ok()?;
    if let Ok(entry) = zip.by_name("mimetype") {
        let mut m = Vec::new();
        entry.take(256).read_to_end(&mut m).ok()?;
        let m = String::from_utf8_lossy(&m);
        match m.trim() {
            "application/epub+zip" => return Some(Format::Epub),
            "application/vnd.oasis.opendocument.text" | "application/vnd.oasis.opendocument.text-template" => return Some(Format::Odt),
            _ => {}
        }
    }
    if zip.index_for_name("word/document.xml").is_some() || zip.index_for_name("[Content_Types].xml").is_some() {
        return Some(Format::Docx);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_name_formats() {
        assert_eq!(Format::from_extension(".DOCX"), Some(Format::Docx));
        assert_eq!(Format::from_extension("markdown"), Some(Format::Markdown));
        assert_eq!(Format::from_extension("pdf"), None);
        assert_eq!(Format::from_name("dir.v2/Book.Final.epub"), Some(Format::Epub));
        assert_eq!(Format::from_name("README"), None);
        for f in Format::ALL {
            assert_eq!(Format::from_extension(f.extension()), Some(f));
        }
    }

    #[test]
    fn content_is_sniffed() {
        assert_eq!(Format::sniff(b"  {\\rtf1\\ansi hello}"), Some(Format::Rtf));
        assert_eq!(Format::sniff(b"\xEF\xBB\xBF<!DOCTYPE HTML><p>x"), Some(Format::Html));
        assert_eq!(Format::sniff(b"<html><body>x"), Some(Format::Html));
        assert_eq!(Format::sniff(b"# Heading\n\nText"), Some(Format::Markdown));
        assert_eq!(Format::sniff(b"- one\n- two\n"), Some(Format::Markdown));
        assert_eq!(Format::sniff(b"Dear reader,\n\nHello. 5 * 3 = 15.\n"), Some(Format::Text));
        assert_eq!(Format::sniff(b"\x00\x01\x02binary"), None);
        assert_eq!(Format::sniff(b"PK\x03\x04broken"), None);
        assert_eq!(Format::sniff(b""), Some(Format::Text));
    }

    #[test]
    fn detection_prefers_decisive_content_then_the_name() {
        assert_eq!(Format::detect(Some("letter.docx"), b"{\\rtf1 x}"), Some(Format::Rtf));
        assert_eq!(Format::detect(Some("notes.txt"), b"# Not a heading here"), Some(Format::Text));
        assert_eq!(Format::detect(None, b"# Heading"), Some(Format::Markdown));
        assert_eq!(Format::detect(Some("page.htm"), b"plain words"), Some(Format::Html));
    }
}
