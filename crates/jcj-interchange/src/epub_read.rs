// SPDX-License-Identifier: MIT OR Apache-2.0
//! Reading an EPUB (core issue #13): the package's chapters in spine order, each read with the
//! HTML reader, joined into one document with a page break before each chapter after the first.
//!
//! Bounded and careful with files that are not trusted: entries are read one by one into memory
//! with their real sizes capped, their number is capped, only the files the package names are
//! read, and damage is an error, never a panic.

use std::collections::HashMap;
use std::io::{Cursor, Read};

use jcj_doc::Document;

/// The most entries a package may have.
pub const MAX_ENTRIES: usize = 10_000;
/// The largest single entry read, bytes.
pub const MAX_ENTRY: u64 = 32 << 20;
/// The most chapters read.
pub const MAX_CHAPTERS: usize = 5_000;

fn entry(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Vec<u8>, String> {
    let f = zip.by_name(name).map_err(|_| format!("the package has no {name}"))?;
    let mut out = Vec::new();
    f.take(MAX_ENTRY + 1).read_to_end(&mut out).map_err(|e| e.to_string())?;
    if out.len() as u64 > MAX_ENTRY {
        return Err(format!("{name} is larger than the limit"));
    }
    Ok(out)
}

/// The value of `attr="..."` inside one tag's text.
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = tag;
    while let Some(at) = rest.find(name) {
        let before_ok = at == 0 || rest[..at].ends_with(|c: char| c.is_whitespace());
        let after = &rest[at + name.len()..];
        let after_trim = after.trim_start();
        if before_ok && let Some(v) = after_trim.strip_prefix('=') {
            let v = v.trim_start();
            let q = v.chars().next()?;
            if q == '"' || q == '\'' {
                let body = &v[1..];
                return body.find(q).map(|end| &body[..end]);
            }
        }
        rest = &rest[at + name.len()..];
    }
    None
}

/// Every tag named `name` (`<name ...>`), as the text between `<` and `>`.
fn tags<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut rest = xml;
    let open = format!("<{name}");
    while let Some(at) = rest.find(&open) {
        let after = &rest[at + open.len()..];
        if after.starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/') {
            let end = after.find('>').unwrap_or(after.len());
            out.push(&after[..end]);
            rest = &after[end..];
        } else {
            rest = after;
        }
    }
    out
}

/// A path relative to the package document's folder, resolved (`../` allowed inside the package).
fn resolve(base: &str, href: &str) -> String {
    let href = href.split('#').next().unwrap_or(href);
    let mut parts: Vec<&str> = base.rsplit_once('/').map_or_else(Vec::new, |(d, _)| d.split('/').collect());
    for p in href.split('/') {
        match p {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    parts.join("/").replace("%20", " ")
}

/// Read an EPUB into one document.
pub fn read(bytes: &[u8]) -> Result<Document, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| "this is not an EPUB package".to_owned())?;
    if zip.len() > MAX_ENTRIES {
        return Err("the package has too many entries".into());
    }
    let container = String::from_utf8(entry(&mut zip, "META-INF/container.xml")?).map_err(|_| "container.xml is not text".to_owned())?;
    let opf_path =
        tags(&container, "rootfile").into_iter().find_map(|t| attr(t, "full-path")).ok_or("container.xml names no package document")?.to_owned();
    let opf = String::from_utf8(entry(&mut zip, &opf_path)?).map_err(|_| "the package document is not text".to_owned())?;
    let mut manifest: HashMap<String, (String, String)> = HashMap::new();
    for t in tags(&opf, "item") {
        if let (Some(id), Some(href)) = (attr(t, "id"), attr(t, "href")) {
            manifest.insert(id.to_owned(), (resolve(&opf_path, href), attr(t, "media-type").unwrap_or("").to_owned()));
        }
    }
    let spine: Vec<String> =
        tags(&opf, "itemref").into_iter().filter(|t| attr(t, "linear") != Some("no")).filter_map(|t| attr(t, "idref").map(str::to_owned)).collect();
    if spine.is_empty() {
        return Err("the package lists no chapters".into());
    }
    let mut doc = jcj_doc::new_document();
    doc.body.clear();
    for (n, idref) in spine.iter().take(MAX_CHAPTERS).enumerate() {
        let Some((path, media)) = manifest.get(idref) else { continue };
        if !(media.contains("html") || path.ends_with(".xhtml") || path.ends_with(".html") || path.ends_with(".htm")) {
            continue;
        }
        let html = entry(&mut zip, path)?;
        let chapter = crate::import(crate::Format::Html, &html).map_err(|e| format!("{path}: {e}"))?;
        let mut blocks = chapter.body;
        if n > 0
            && let Some(first) = blocks.first_mut()
            && let jcj_doc::Block::Para(p) = std::sync::Arc::make_mut(first)
        {
            p.props.page_break_before = Some(true);
        }
        doc.body.extend(blocks);
    }
    if let Some(title) = tags(&opf, "dc:title")
        .first()
        .and_then(|_| opf.split("<dc:title").nth(1))
        .and_then(|s| s.split_once('>'))
        .and_then(|(_, r)| r.split_once("</dc:title>"))
        .map(|(t, _)| t.trim())
    {
        doc.core.title = title.to_owned();
    }
    if let Some(who) =
        opf.split("<dc:creator").nth(1).and_then(|s| s.split_once('>')).and_then(|(_, r)| r.split_once("</dc:creator>")).map(|(t, _)| t.trim())
    {
        doc.core.creator = who.to_owned();
    }
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_resolve_inside_the_package() {
        assert_eq!(resolve("OEBPS/content.opf", "text/ch1.xhtml"), "OEBPS/text/ch1.xhtml");
        assert_eq!(resolve("OEBPS/content.opf", "../ch.xhtml#a"), "ch.xhtml");
        assert_eq!(resolve("content.opf", "a%20b.xhtml"), "a b.xhtml");
    }

    #[test]
    fn attributes_and_tags() {
        let t = tags(r#"<item id="c1" href='ch1.xhtml' media-type="application/xhtml+xml"/><itemref idref="c1"/>"#, "item");
        assert_eq!(t.len(), 1);
        assert_eq!(attr(t[0], "href"), Some("ch1.xhtml"));
        assert_eq!(attr(t[0], "id"), Some("c1"));
    }

    #[test]
    fn what_we_write_we_read_back() {
        let mut d = jcj_doc::new_document();
        d.body.clear();
        for t in ["First chapter words.", "Second chapter words."] {
            d.body.push(std::sync::Arc::new(jcj_doc::Block::Para(jcj_doc::Paragraph::with_text(t, jcj_doc::CharProps::default()))));
        }
        d.core.title = "A Test".into();
        let bytes = crate::export(crate::Format::Epub, &d).unwrap();
        let back = read(&bytes).unwrap();
        let text: Vec<String> = back.body.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).filter(|t| !t.trim().is_empty()).collect();
        assert!(text.iter().any(|t| t.contains("First chapter words.")) && text.iter().any(|t| t.contains("Second chapter words.")), "{text:?}");
    }

    #[test]
    fn damaged_packages_are_errors() {
        assert!(read(b"not a zip").is_err());
        assert!(read(&[]).is_err());
    }
}
