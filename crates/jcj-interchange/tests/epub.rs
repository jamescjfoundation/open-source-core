// SPDX-License-Identifier: MIT OR Apache-2.0
//! The EPUB writer's package structure: OCF container, package document, navigation and
//! well-formed XHTML chapters split at level 1 headings.

use std::collections::BTreeMap;
use std::io::{Cursor, Read};

use jcj_doc::props::CharProps;
use jcj_doc::{Document, Paragraph, para_block};
use jcj_interchange::{Format, export};
use quick_xml::Reader;
use quick_xml::events::Event;

mod common;

/// Every entry: name → (stored uncompressed, bytes), and the names in order.
fn entries(bytes: &[u8]) -> (Vec<String>, BTreeMap<String, (bool, Vec<u8>)>) {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut order = Vec::new();
    let mut map = BTreeMap::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).unwrap();
        let mut b = Vec::new();
        f.read_to_end(&mut b).unwrap();
        order.push(f.name().to_string());
        map.insert(f.name().to_string(), (f.compression() == zip::CompressionMethod::Stored, b));
    }
    (order, map)
}

/// Parse fully and check that every start tag closes (quick-xml checks end names).
fn parse(name: &str, xml: &[u8]) -> (Vec<String>, String) {
    let mut r = Reader::from_reader(xml);
    r.config_mut().check_end_names = true;
    let mut open = 0i64;
    let mut items = Vec::new();
    let mut text = String::new();
    let mut buf = Vec::new();
    loop {
        let ev = r.read_event_into(&mut buf).unwrap_or_else(|e| panic!("{name} is not well-formed: {e}"));
        match ev {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                for a in e.attributes() {
                    let a = a.unwrap_or_else(|err| panic!("{name}: bad attribute: {err}"));
                    items.push(format!(
                        "{tag}@{}={}",
                        String::from_utf8_lossy(a.key.as_ref()),
                        a.normalized_value(quick_xml::XmlVersion::Explicit1_0).unwrap()
                    ));
                }
                items.push(tag);
                if matches!(ev, Event::Start(_)) {
                    open += 1;
                }
            }
            Event::End(_) => open -= 1,
            Event::Text(t) => text.push_str(&t.decode().unwrap()),
            // Only XML's own entities and character references: XHTML in EPUB has no DTD.
            Event::GeneralRef(r) => match r.resolve_char_ref().unwrap_or_else(|e| panic!("{name}: {e}")) {
                Some(c) => text.push(c),
                None => text.push(match &*r.decode().unwrap() {
                    "amp" => '&',
                    "lt" => '<',
                    "gt" => '>',
                    "quot" => '"',
                    "apos" => '\'',
                    other => panic!("{name}: undefined entity &{other};"),
                }),
            },
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    assert_eq!(open, 0, "{name}: every element closes");
    (items, text)
}

type Files = BTreeMap<String, (bool, Vec<u8>)>;

fn sample_book() -> (Vec<u8>, Files) {
    let bytes = export(Format::Epub, &common::sample()).unwrap();
    let files = entries(&bytes).1;
    (bytes, files)
}

#[test]
fn the_mimetype_comes_first_and_the_container_points_to_the_package() {
    let (bytes, _) = sample_book();
    // mimetype: first, stored, no extra field, exact content (OCF 3.3 §4.2.6).
    assert_eq!(&bytes[..4], b"PK\x03\x04");
    assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), 0, "stored");
    assert_eq!(u16::from_le_bytes([bytes[26], bytes[27]]), 8);
    assert_eq!(u16::from_le_bytes([bytes[28], bytes[29]]), 0, "no extra field");
    assert_eq!(&bytes[30..38], b"mimetype");
    assert_eq!(&bytes[38..58], b"application/epub+zip");

    let (order, files) = entries(&bytes);
    assert_eq!(order[0], "mimetype");
    assert!(files["mimetype"].0);

    // container.xml points to the package document.
    let (items, _) = parse("container.xml", &files["META-INF/container.xml"].1);
    assert!(items.contains(&"rootfile@full-path=OEBPS/content.opf".to_string()), "{items:?}");
    assert!(items.contains(&"rootfile@media-type=application/oebps-package+xml".to_string()));
}

#[test]
fn the_package_lists_metadata_every_file_and_the_spine() {
    let (_, files) = sample_book();
    let (opf, opf_text) = parse("content.opf", &files["OEBPS/content.opf"].1);
    assert!(opf.contains(&"package@version=3.0".to_string()));
    assert!(opf.contains(&"package@unique-identifier=uid".to_string()));
    assert!(opf.contains(&"dc:identifier@id=uid".to_string()));
    for tag in ["dc:title", "dc:language", "dc:creator", "meta"] {
        assert!(opf.contains(&tag.to_string()), "{tag}");
    }
    assert!(opf_text.contains("Sample Book") && opf_text.contains("A. Writer") && opf_text.contains("urn:uuid:"));
    assert!(opf.contains(&"meta@property=dcterms:modified".to_string()));
    let hrefs: Vec<&str> = opf.iter().filter_map(|i| i.strip_prefix("item@href=")).collect();
    for href in &hrefs {
        assert!(files.contains_key(&format!("OEBPS/{href}")), "manifest item {href} exists");
    }
    for name in files.keys().filter(|n| n.starts_with("OEBPS/") && *n != "OEBPS/content.opf") {
        assert!(hrefs.contains(&name.trim_start_matches("OEBPS/")), "{name} is in the manifest");
    }
    assert!(opf.contains(&"item@properties=nav".to_string()));
    let spine: Vec<&str> = opf.iter().filter_map(|i| i.strip_prefix("itemref@idref=")).collect();
    assert_eq!(spine, ["ch1", "ch2"], "one chapter per level 1 heading");
}

#[test]
fn the_navigation_and_chapters_are_well_formed_xhtml() {
    let (_, files) = sample_book();
    // The navigation lists the chapters by their headings.
    let (nav, nav_text) = parse("nav.xhtml", &files["OEBPS/nav.xhtml"].1);
    assert!(nav.contains(&"nav@epub:type=toc".to_string()));
    let links: Vec<&str> = nav.iter().filter_map(|i| i.strip_prefix("a@href=")).collect();
    assert_eq!(links, ["chapter-001.xhtml", "chapter-002.xhtml"]);
    assert!(nav_text.contains("Chapter One") && nav_text.contains("Chapter Two"));

    // Chapters are well-formed XHTML with the content in the right one.
    let (c1, t1) = parse("chapter-001.xhtml", &files["OEBPS/chapter-001.xhtml"].1);
    let (_, t2) = parse("chapter-002.xhtml", &files["OEBPS/chapter-002.xhtml"].1);
    assert!(c1.contains(&"html@xmlns=http://www.w3.org/1999/xhtml".to_string()));
    for tag in ["h1", "h2", "strong", "em", "ul", "li", "table", "td"] {
        assert!(c1.contains(&tag.to_string()), "chapter 1 has <{tag}>: {c1:?}");
    }
    assert!(t1.contains("words & <signs>.") && t1.contains("alpha") && !t1.contains("The end."));
    assert!(t2.contains("Chapter Two") && t2.contains("The end."));
}

#[test]
fn the_same_document_gives_the_same_book() {
    let a = export(Format::Epub, &common::sample()).unwrap();
    assert_eq!(a, export(Format::Epub, &common::sample()).unwrap());
}

/// A 1×1 PNG.
const PNG_B64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

#[test]
fn pictures_are_packaged_once_and_listed() {
    let html = format!(
        "<html><body><h1>Pictures</h1><p><img src=\"data:image/png;base64,{PNG_B64}\" alt=\"A dot\"> and again <img src=\"data:image/png;base64,{PNG_B64}\" alt=\"Same dot\"></p></body></html>"
    );
    let doc = jcj_interchange::import(Format::Html, html.as_bytes()).unwrap();
    let bytes = export(Format::Epub, &doc).unwrap();
    let (_, files) = entries(&bytes);
    let images: Vec<&String> = files.keys().filter(|k| k.starts_with("OEBPS/images/")).collect();
    assert_eq!(images, ["OEBPS/images/image-1.png"], "the same picture is packaged once");
    assert!(files["OEBPS/images/image-1.png"].0, "pictures are stored");
    let (opf, _) = parse("content.opf", &files["OEBPS/content.opf"].1);
    assert!(opf.contains(&"item@media-type=image/png".to_string()));
    let (ch, _) = parse("chapter", &files["OEBPS/chapter-001.xhtml"].1);
    assert!(ch.contains(&"img@src=images/image-1.png".to_string()) && ch.contains(&"img@alt=A dot".to_string()), "{ch:?}");
}

#[test]
fn front_matter_and_hostile_text() {
    let mut d = Document::new();
    d.body = vec![
        para_block(Paragraph::with_text("Before any heading \u{1}\u{FFFE} ]]> <script>", CharProps::default())),
        para_block(Paragraph::with_text("", CharProps::default())),
        para_block(Paragraph::with_text("Only Chapter", CharProps::default()).styled("Heading1")),
    ];
    let bytes = export(Format::Epub, &d).unwrap();
    let (_, files) = entries(&bytes);
    let mut chapters = 0;
    for (name, (_, b)) in &files {
        if name.ends_with(".xhtml") || name.ends_with(".opf") || name.ends_with(".xml") {
            let (items, _) = parse(name, b);
            assert!(!items.contains(&"script".to_string()), "{name}");
            chapters += usize::from(name.contains("chapter-"));
        }
    }
    assert_eq!(chapters, 2, "front matter, then the chapter");
    let (_, front) = parse("front", &files["OEBPS/chapter-001.xhtml"].1);
    assert!(front.contains("Before any heading  ]]> <script>"), "{front}");
    // An empty document still makes a book with one chapter.
    let (_, files) = entries(&export(Format::Epub, &Document::new()).unwrap());
    assert!(files.contains_key("OEBPS/chapter-001.xhtml"));
}
