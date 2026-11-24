// SPDX-License-Identifier: MIT OR Apache-2.0
//! A minimal EPUB 3 writer.
//!
//! No EPUB crate exists at our upstream pins, so the core writes its own: the document is turned
//! into WordCraft's flow model (paragraph kinds, lists, formatted spans, tables, pictures) and
//! written as XHTML, one file per chapter. A chapter starts at each level 1 heading (Heading 1,
//! or outline level 1); anything before the first one is a chapter of its own.
//!
//! The package (EPUB 3.3, OCF):
//!
//! ```text
//! mimetype                     first, Stored, "application/epub+zip"
//! META-INF/container.xml       points to OEBPS/content.opf
//! OEBPS/content.opf            package: metadata from the document's properties, manifest, spine
//! OEBPS/nav.xhtml              the table of contents (one entry per chapter)
//! OEBPS/style.css
//! OEBPS/chapter-001.xhtml ...
//! OEBPS/images/image-1.png ... pictures (PNG, JPEG, GIF, WebP, SVG; others are left out)
//! ```
//!
//! The output is deterministic: the same document gives the same bytes (the identifier is made
//! from the content, and the modification date from the document's properties).

use std::io::{Cursor, Write};
use std::sync::Arc;

use jcj_doc::{Align, Document};
use jcj_upstream::formats::model::{self, Cell, FBlock, FTable, Fmt, Inline, Kind, Para};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// Deeper table nesting than this is written as text only.
const MAX_DEPTH: usize = 16;

const XHTML_NS: &str = "http://www.w3.org/1999/xhtml";
const OPS_NS: &str = "http://www.idpf.org/2007/ops";

const STYLE: &str = "body { margin: 0 5%; line-height: 1.4; }\n\
h1, h2, h3, h4, h5, h6 { line-height: 1.2; page-break-after: avoid; }\n\
p { margin: 0 0 0.6em 0; }\n\
.title { font-size: 2em; text-align: center; }\n\
.code { font-family: monospace; white-space: pre-wrap; }\n\
img { max-width: 100%; }\n\
table { border-collapse: collapse; margin: 0.6em 0; }\n\
td, th { border: 1px solid #888; padding: 0.2em 0.4em; vertical-align: top; }\n";

/// Text escaped for XML content and attributes; characters XML 1.0 forbids are left out.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if (c as u32) < 0x20 || c == '\u{FFFE}' || c == '\u{FFFF}' => {}
            c => out.push(c),
        }
    }
    out
}

/// Text of a run for XHTML: escaped, line breaks as `<br/>`, objects and page breaks left out.
fn text_xhtml(s: &str) -> String {
    let cleaned: String = s.chars().filter(|c| *c != jcj_doc::para::OBJ).collect();
    let mut out = String::new();
    for (n, line) in cleaned.split(['\n', '\u{b}']).enumerate() {
        if n > 0 {
            out.push_str("<br/>");
        }
        out.push_str(&esc(line));
    }
    out
}

/// The pictures written into the package, in order of first use.
#[derive(Default)]
struct Images {
    files: Vec<(String, &'static str, Arc<Vec<u8>>)>,
}

impl Images {
    /// The package path (from OEBPS) of a picture, `None` for a type EPUB readers need not show.
    fn add(&mut self, data: &Arc<Vec<u8>>, ext: &str) -> Option<String> {
        let (ext, media) = match ext.to_ascii_lowercase().as_str() {
            "png" => ("png", "image/png"),
            "jpg" | "jpeg" => ("jpeg", "image/jpeg"),
            "gif" => ("gif", "image/gif"),
            "webp" => ("webp", "image/webp"),
            "svg" => ("svg", "image/svg+xml"),
            _ => return None,
        };
        if let Some((name, ..)) = self.files.iter().find(|(_, _, d)| Arc::ptr_eq(d, data) || d == data) {
            return Some(name.clone());
        }
        let name = format!("images/image-{}.{ext}", self.files.len() + 1);
        self.files.push((name.clone(), media, data.clone()));
        Some(name)
    }
}

/// A link target safe to write: web and mail links only.
fn safe_link(link: &str) -> Option<&str> {
    let l = link.trim();
    let lower = l.to_ascii_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")).then_some(l)
}

/// The tags `f` opens, outermost first, with their closing tags.
fn fmt_tags(f: &Fmt) -> Vec<(String, &'static str)> {
    let mut t = Vec::new();
    if let Some(link) = f.link.as_deref().and_then(safe_link) {
        t.push((format!("<a href=\"{}\">", esc(link)), "</a>"));
    }
    for (on, open, close) in [
        (f.bold, "<strong>", "</strong>"),
        (f.italic, "<em>", "</em>"),
        (f.underline, "<u>", "</u>"),
        (f.strike, "<s>", "</s>"),
        (f.code, "<code>", "</code>"),
        (f.sup, "<sup>", "</sup>"),
        (f.sub && !f.sup, "<sub>", "</sub>"),
    ] {
        if on {
            t.push((open.to_string(), close));
        }
    }
    t
}

fn styled(out: &mut String, inner: &str, f: &Fmt) {
    if inner.is_empty() {
        return;
    }
    let tags = fmt_tags(f);
    for (open, _) in &tags {
        out.push_str(open);
    }
    out.push_str(inner);
    for (_, close) in tags.iter().rev() {
        out.push_str(close);
    }
}

fn inlines(out: &mut String, items: &[Inline], images: &mut Images) {
    for i in items {
        match i {
            Inline::Text(t, f) => styled(out, &text_xhtml(t), f),
            Inline::Image(img) => match images.add(&img.data, &img.ext) {
                Some(src) => out.push_str(&format!("<img src=\"{}\" alt=\"{}\"/>", esc(&src), esc(&img.alt))),
                None => styled(out, &esc(&img.alt), &Fmt::default()),
            },
            Inline::Equation { linear, .. } => styled(out, &text_xhtml(linear), &Fmt::default()),
            Inline::Ruby { base, ruby, fmt } => {
                styled(out, &format!("<ruby>{}<rt>{}</rt></ruby>", text_xhtml(base), text_xhtml(ruby)), fmt);
            }
            Inline::Field { result, fmt, .. } => styled(out, &text_xhtml(result), fmt),
            Inline::Control { text, fmt, .. } => styled(out, &text_xhtml(text), fmt),
            _ => {}
        }
    }
}

/// A paragraph's plain text (for chapter titles).
fn plain(p: &Para) -> String {
    let mut s = String::new();
    for i in &p.inlines {
        if let Inline::Text(t, _) = i {
            s.push_str(t);
        }
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn align_attr(a: Option<Align>) -> &'static str {
    match a {
        Some(Align::Center) => " style=\"text-align: center\"",
        Some(Align::Right) => " style=\"text-align: right\"",
        Some(Align::Justify | Align::Distribute) => " style=\"text-align: justify\"",
        _ => "",
    }
}

/// The body of one paragraph element (without list markup).
fn para_xhtml(out: &mut String, p: &Para, images: &mut Images) {
    let mut body = String::new();
    inlines(&mut body, &p.inlines, images);
    let align = align_attr(p.align);
    match p.kind {
        Kind::Heading(n) => {
            let n = n.clamp(1, 6);
            out.push_str(&format!("<h{n}{align}>{body}</h{n}>"));
        }
        Kind::Title => out.push_str(&format!("<p class=\"title\"{align}>{body}</p>")),
        Kind::Quote => out.push_str(&format!("<blockquote><p{align}>{body}</p></blockquote>")),
        Kind::Code => out.push_str(&format!("<p class=\"code\"><code>{body}</code></p>")),
        Kind::Rule => out.push_str("<hr/>"),
        Kind::Normal if body.is_empty() => out.push_str("<p>&#160;</p>"),
        Kind::Normal => out.push_str(&format!("<p{align}>{body}</p>")),
    }
    out.push('\n');
}

/// Open lists while writing blocks: (ordered, an item is open).
#[derive(Default)]
struct Lists(Vec<(bool, bool)>);

impl Lists {
    fn close_to(&mut self, out: &mut String, depth: usize) {
        while self.0.len() > depth {
            if let Some((ordered, item)) = self.0.pop() {
                if item {
                    out.push_str("</li>");
                }
                out.push_str(if ordered { "</ol>\n" } else { "</ul>\n" });
            }
        }
    }

    /// Start a list item at `level` (0-based).
    fn item(&mut self, out: &mut String, ordered: bool, level: u8, start: u32) {
        let depth = usize::from(level.min(8)) + 1;
        self.close_to(out, depth);
        if self.0.len() == depth && self.0.last().is_some_and(|(o, _)| *o != ordered) {
            self.close_to(out, depth - 1);
        }
        while self.0.len() < depth {
            if let Some(top) = self.0.last_mut()
                && !top.1
            {
                out.push_str("<li>");
                top.1 = true;
            }
            let nested_ordered = if self.0.len() + 1 == depth { ordered } else { false };
            if nested_ordered && start != 1 {
                out.push_str(&format!("<ol start=\"{start}\">"));
            } else {
                out.push_str(if nested_ordered { "<ol>" } else { "<ul>" });
            }
            self.0.push((nested_ordered, false));
        }
        if let Some(top) = self.0.last_mut() {
            if top.1 {
                out.push_str("</li>\n");
            }
            out.push_str("<li>");
            top.1 = true;
        }
    }
}

fn blocks_xhtml(out: &mut String, blocks: &[&FBlock], images: &mut Images, depth: usize) {
    let mut lists = Lists::default();
    for b in blocks {
        match b {
            FBlock::Para(p) => match p.list {
                Some(l) => {
                    lists.item(out, l.ordered, l.level, l.start);
                    let mut item = String::new();
                    inlines(&mut item, &p.inlines, images);
                    out.push_str(&item);
                }
                None => {
                    lists.close_to(out, 0);
                    para_xhtml(out, p, images);
                }
            },
            FBlock::Table(t) => {
                lists.close_to(out, 0);
                table_xhtml(out, t, images, depth);
            }
        }
    }
    lists.close_to(out, 0);
}

fn table_xhtml(out: &mut String, t: &FTable, images: &mut Images, depth: usize) {
    if depth >= MAX_DEPTH {
        return;
    }
    out.push_str("<table><tbody>\n");
    for row in &t.rows {
        out.push_str("<tr>");
        for cell in row.iter().filter(|c| !c.covered) {
            cell_xhtml(out, cell, images, depth);
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</tbody></table>\n");
}

fn cell_xhtml(out: &mut String, cell: &Cell, images: &mut Images, depth: usize) {
    let tag = if cell.header { "th" } else { "td" };
    let mut attrs = String::new();
    if cell.colspan > 1 {
        attrs.push_str(&format!(" colspan=\"{}\"", cell.colspan.min(1000)));
    }
    if cell.rowspan > 1 {
        attrs.push_str(&format!(" rowspan=\"{}\"", cell.rowspan.min(65_534)));
    }
    out.push_str(&format!("<{tag}{attrs}>"));
    let blocks: Vec<&FBlock> = cell.blocks.iter().collect();
    blocks_xhtml(out, &blocks, images, depth + 1);
    out.push_str(&format!("</{tag}>"));
}

/// A chapter: its title and its blocks.
struct Chapter<'a> {
    title: String,
    blocks: Vec<&'a FBlock>,
}

/// Split at level 1 headings.
fn chapters(flow: &model::Flow) -> Vec<Chapter<'_>> {
    let mut out: Vec<Chapter> = Vec::new();
    for b in &flow.blocks {
        let heading = match b {
            FBlock::Para(p) if p.kind == Kind::Heading(1) && p.list.is_none() => Some(plain(p)),
            _ => None,
        };
        match (heading, out.last_mut()) {
            (Some(title), Some(last)) if !last.blocks.is_empty() => out.push(Chapter { title, blocks: vec![b] }),
            (Some(title), Some(last)) => {
                last.title = title;
                last.blocks.push(b);
            }
            (Some(title), None) => out.push(Chapter { title, blocks: vec![b] }),
            (None, Some(last)) => last.blocks.push(b),
            (None, None) => out.push(Chapter { title: String::new(), blocks: vec![b] }),
        }
    }
    if out.is_empty() {
        out.push(Chapter { title: String::new(), blocks: Vec::new() });
    }
    out
}

/// The document's language, from its default text properties (`und`, undetermined, if none).
fn language(doc: &Document) -> String {
    let from = |l: &Option<String>| l.as_deref().map(str::trim).filter(|l| !l.is_empty() && l.len() <= 35).map(str::to_string);
    from(&doc.styles.default_chr.lang)
        .or_else(|| doc.styles.get("Normal").and_then(|s| from(&s.chr.lang)))
        .filter(|l| l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
        .unwrap_or_else(|| "und".into())
}

/// The modification date EPUB requires (`CCYY-MM-DDThh:mm:ssZ`), from the document's
/// properties; the Unix epoch when it has none.
fn modified(doc: &Document) -> String {
    [&doc.core.modified, &doc.core.created]
        .into_iter()
        .find_map(|s| chrono::DateTime::parse_from_rfc3339(s.trim()).ok())
        .map(|t| t.with_timezone(&chrono::Utc))
        .unwrap_or_default()
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string()
}

fn xhtml_page(lang: &str, title: &str, body: &str) -> String {
    let lang = esc(lang);
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"{XHTML_NS}\" xmlns:epub=\"{OPS_NS}\" lang=\"{lang}\" xml:lang=\"{lang}\">\n<head>\n<meta charset=\"utf-8\"/>\n<title>{}</title>\n<link rel=\"stylesheet\" type=\"text/css\" href=\"style.css\"/>\n</head>\n<body>\n{body}</body>\n</html>\n",
        esc(title)
    )
}

/// The EPUB file for `doc`.
pub(crate) fn write(doc: &Document) -> Result<Vec<u8>, String> {
    let flow = model::from_doc(doc);
    let lang = language(doc);
    let book_title = Some(doc.core.title.trim()).filter(|t| !t.is_empty()).map(str::to_string);
    let mut images = Images::default();
    let mut pages: Vec<(String, String, String)> = Vec::new();
    for (n, ch) in chapters(&flow).into_iter().enumerate() {
        let title = Some(ch.title).filter(|t| !t.is_empty()).or_else(|| book_title.clone()).unwrap_or_else(|| format!("Chapter {}", n + 1));
        let mut body = String::from("<section epub:type=\"chapter\">\n");
        blocks_xhtml(&mut body, &ch.blocks, &mut images, 0);
        body.push_str("</section>\n");
        pages.push((format!("chapter-{:03}.xhtml", n + 1), title.clone(), xhtml_page(&lang, &title, &body)));
    }
    let title = book_title.or_else(|| pages.first().map(|p| p.1.clone())).unwrap_or_else(|| "Untitled".into());

    let mut toc = String::from("<nav epub:type=\"toc\" id=\"toc\">\n<h1>Contents</h1>\n<ol>\n");
    for (file, t, _) in &pages {
        toc.push_str(&format!("<li><a href=\"{file}\">{}</a></li>\n", esc(t)));
    }
    toc.push_str("</ol>\n</nav>\n");
    let nav = xhtml_page(&lang, &title, &toc);

    // The identifier is made from the content, so the same document always gives the same book.
    let mut h = Sha256::new();
    h.update(title.as_bytes());
    for (_, _, page) in &pages {
        h.update(page.as_bytes());
    }
    let d: Vec<u8> = h.finalize().iter().take(16).copied().collect();
    let hex: String = d.iter().map(|b| format!("{b:02x}")).collect();
    let uuid = format!("{}-{}-4{}-a{}-{}", &hex[0..8], &hex[8..12], &hex[13..16], &hex[17..20], &hex[20..32]);

    let mut meta = format!(
        "<dc:identifier id=\"uid\">urn:uuid:{uuid}</dc:identifier>\n<dc:title>{}</dc:title>\n<dc:language>{}</dc:language>\n",
        esc(&title),
        esc(&lang)
    );
    for (tag, value) in [("creator", &doc.core.creator), ("subject", &doc.core.subject), ("description", &doc.core.description)] {
        if !value.trim().is_empty() {
            meta.push_str(&format!("<dc:{tag}>{}</dc:{tag}>\n", esc(value.trim())));
        }
    }
    meta.push_str(&format!("<meta property=\"dcterms:modified\">{}</meta>\n", modified(doc)));
    let mut manifest = String::from(
        "<item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n<item id=\"css\" href=\"style.css\" media-type=\"text/css\"/>\n",
    );
    let mut spine = String::new();
    for (n, (file, ..)) in pages.iter().enumerate() {
        manifest.push_str(&format!("<item id=\"ch{}\" href=\"{file}\" media-type=\"application/xhtml+xml\"/>\n", n + 1));
        spine.push_str(&format!("<itemref idref=\"ch{}\"/>\n", n + 1));
    }
    for (n, (file, media, _)) in images.files.iter().enumerate() {
        manifest.push_str(&format!("<item id=\"img{}\" href=\"{file}\" media-type=\"{media}\"/>\n", n + 1));
    }
    let opf = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"uid\" xml:lang=\"{}\">\n<metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n{meta}</metadata>\n<manifest>\n{manifest}</manifest>\n<spine>\n{spine}</spine>\n</package>\n",
        esc(&lang)
    );
    let container = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\">\n<rootfiles>\n<rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/>\n</rootfiles>\n</container>\n";

    let mut parts: Vec<(String, Vec<u8>, CompressionMethod)> = vec![
        ("META-INF/container.xml".into(), container.as_bytes().to_vec(), CompressionMethod::Deflated),
        ("OEBPS/content.opf".into(), opf.into_bytes(), CompressionMethod::Deflated),
        ("OEBPS/nav.xhtml".into(), nav.into_bytes(), CompressionMethod::Deflated),
        ("OEBPS/style.css".into(), STYLE.as_bytes().to_vec(), CompressionMethod::Deflated),
    ];
    for (file, _, page) in pages {
        parts.push((format!("OEBPS/{file}"), page.into_bytes(), CompressionMethod::Deflated));
    }
    for (file, _, data) in &images.files {
        parts.push((format!("OEBPS/{file}"), data.to_vec(), CompressionMethod::Stored));
    }
    package(&parts).map_err(|e| e.to_string())
}

fn package(parts: &[(String, Vec<u8>, CompressionMethod)]) -> zip::result::ZipResult<Vec<u8>> {
    let base = SimpleFileOptions::default().last_modified_time(zip::DateTime::default());
    let mut w = ZipWriter::new(Cursor::new(Vec::new()));
    // OCF: `mimetype` first, uncompressed, no extra field.
    w.start_file("mimetype", base.compression_method(CompressionMethod::Stored))?;
    w.write_all(b"application/epub+zip")?;
    for (name, bytes, method) in parts {
        w.start_file(name.as_str(), base.compression_method(*method))?;
        w.write_all(bytes)?;
    }
    Ok(w.finish()?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_escaped_for_xml() {
        assert_eq!(esc("a<b>&\"c'\u{1}\u{FFFE}"), "a&lt;b&gt;&amp;&quot;c&#39;");
        assert_eq!(text_xhtml("one\ntwo\u{FFFC}"), "one<br/>two");
        assert_eq!(safe_link("javascript:alert(1)"), None);
        assert_eq!(safe_link(" https://example.org "), Some("https://example.org"));
    }

    #[test]
    fn nested_lists_are_balanced() {
        let mut out = String::new();
        let mut l = Lists::default();
        l.item(&mut out, false, 0, 1);
        out.push('a');
        l.item(&mut out, true, 2, 3);
        out.push('b');
        l.item(&mut out, false, 0, 1);
        out.push('c');
        l.item(&mut out, true, 0, 1);
        out.push('d');
        l.close_to(&mut out, 0);
        assert_eq!(out.matches("<li>").count(), out.matches("</li>").count(), "{out}");
        assert_eq!(out.matches("<ul>").count(), out.matches("</ul>").count(), "{out}");
        assert_eq!(out.matches("<ol").count(), out.matches("</ol>").count(), "{out}");
        assert!(out.contains("<ol start=\"3\">"), "{out}");
    }
}
