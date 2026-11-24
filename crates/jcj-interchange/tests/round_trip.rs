// SPDX-License-Identifier: MIT OR Apache-2.0
//! Each format, exported from a document built in code and imported again, keeps what it can
//! hold: the text, bold and italic, headings, a list and a table.

use jcj_doc::props::CharProps;
use jcj_doc::{Block, Document, Paragraph};
use jcj_interchange::{Direction, Error, Format, MAX_IMPORT_BYTES, export, import};

mod common;
use common::sample;

/// Every paragraph in reading order (entering tables).
fn paragraphs(d: &Document) -> Vec<Paragraph> {
    jcj_doc::walk::body_paragraphs(d).into_iter().map(|(_, p)| p.clone()).collect()
}

fn all_text(d: &Document) -> String {
    paragraphs(d).iter().map(|p| p.plain_text()).collect::<Vec<_>>().join("\n")
}

fn has_run(d: &Document, text: &str, pick: fn(&CharProps) -> bool) -> bool {
    paragraphs(d).iter().any(|p| {
        p.run_ranges().any(|(r, c)| {
            let resolved = d.styles.resolve_char(p.props.style.as_deref(), c);
            let c = CharProps { bold: Some(resolved.bold), italic: Some(resolved.italic), ..CharProps::default() };
            p.text.get(r).is_some_and(|t| t.contains(text)) && pick(&c)
        })
    })
}

fn heading_level(d: &Document, text: &str) -> Option<u8> {
    paragraphs(d).iter().find(|p| p.plain_text().trim() == text).and_then(|p| d.styles.resolve_para(&p.props).outline_level)
}

fn is_list_item(d: &Document, text: &str) -> bool {
    paragraphs(d).iter().any(|p| p.plain_text().trim() == text && d.styles.resolve_para(&p.props).numbering.is_some_and(|n| n.num != 0))
}

fn has_table(d: &Document) -> bool {
    d.body.iter().any(|b| matches!(&**b, Block::Table(_)))
}

/// What each format keeps of the sample: (format, emphasis, headings, lists, tables).
const KEEPS: [(Format, bool, bool, bool, bool); 6] = [
    (Format::Docx, true, true, true, true),
    (Format::Odt, true, true, true, true),
    (Format::Rtf, true, true, true, true),
    (Format::Markdown, true, true, true, true),
    (Format::Html, true, true, true, true),
    (Format::Text, false, false, false, false),
];

#[test]
fn every_format_round_trips_what_it_holds() {
    let doc = sample();
    for (format, emphasis, headings, lists, tables) in KEEPS {
        let bytes = export(format, &doc).unwrap_or_else(|e| panic!("{format:?}: {e}"));
        assert_eq!(Format::detect(Some(&format!("x.{}", format.extension())), &bytes), Some(format), "{format:?} is recognized");
        let back = import(format, &bytes).unwrap_or_else(|e| panic!("{format:?}: {e}"));
        let text = all_text(&back);
        for want in ["Chapter One", "Plain, bold and italic words & <signs>.", "First item", "Second item", "A Section", "alpha", "Value", "The end."]
        {
            assert!(text.contains(want), "{format:?} lost {want:?}:\n{text}");
        }
        if emphasis {
            assert!(has_run(&back, "bold", |c| c.bold == Some(true)), "{format:?}: bold");
            assert!(has_run(&back, "italic", |c| c.italic == Some(true)), "{format:?}: italic");
            assert!(!has_run(&back, "Plain", |c| c.bold == Some(true)), "{format:?}: plain stays plain");
        }
        if headings {
            assert_eq!(heading_level(&back, "Chapter One"), Some(0), "{format:?}: heading 1");
            assert_eq!(heading_level(&back, "A Section"), Some(1), "{format:?}: heading 2");
        }
        if lists {
            assert!(is_list_item(&back, "First item") && is_list_item(&back, "Second item"), "{format:?}: list");
        }
        if tables {
            assert!(has_table(&back), "{format:?}: table");
        }
    }
}

#[test]
fn epub_import_is_a_listed_gap() {
    let bytes = export(Format::Epub, &sample()).unwrap();
    assert_eq!(Format::sniff(&bytes), Some(Format::Epub));
    assert!(!Format::Epub.can_import());
    assert_eq!(import(Format::Epub, &bytes).unwrap_err(), Error::Unsupported { format: Format::Epub, direction: Direction::Import });
}

#[test]
fn hostile_input_is_an_error_not_a_crash() {
    for format in Format::ALL.into_iter().filter(|f| f.can_import()) {
        for bytes in [&b""[..], b"\x00\x01\x02\x03", b"PK\x03\x04garbage", b"{\\rtf1 {{{{{{{{", b"<html><table><tr><td><table>"] {
            // Readers are lenient: they may make a document of it, but never panic.
            let _ = import(format, bytes);
        }
    }
    assert!(matches!(import(Format::Docx, b"not a zip"), Err(Error::Malformed { format: Format::Docx, .. })));
    let big = vec![b'a'; MAX_IMPORT_BYTES + 1];
    assert!(matches!(import(Format::Text, &big), Err(Error::TooLarge { .. })));
    // A document survives a truncated DOCX the same way.
    let docx = export(Format::Docx, &sample()).unwrap();
    for cut in (0..docx.len()).step_by(997) {
        let _ = import(Format::Docx, &docx[..cut]);
    }
}

#[test]
fn imports_always_give_a_document_with_a_paragraph() {
    let d = import(Format::Text, b"").unwrap();
    assert!(!d.body.is_empty());
}
