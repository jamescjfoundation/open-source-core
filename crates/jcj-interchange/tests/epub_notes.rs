// SPDX-License-Identifier: MIT OR Apache-2.0
//! The EPUB writer's notes (core issues #21 and #33): the caller chooses the numbering, the
//! link back and a heading, and learns the words the writer added; with no choice made, the
//! file is byte for byte what the writer has always written.

use std::io::{Cursor, Read};

use jcj_doc::para::{InlineObject, NoteKind};
use jcj_doc::props::CharProps;
use jcj_doc::{Document, Paragraph, PartKind, para_block};
use jcj_interchange::{EpubOptions, Format, NoteBacklink, NoteNumbering, export, export_epub};
use sha2::{Digest, Sha256};

/// Two chapters. The first calls two notes (the second with a mark of its own), the second one.
fn noted() -> Document {
    let mut d = Document::new();
    d.core.title = "Noted".into();
    let note = |d: &mut Document, kind: PartKind, text: &str| d.add_part(kind, vec![para_block(Paragraph::with_text(text, CharProps::default()))]);
    let a = note(&mut d, PartKind::Footnote, "The first note.");
    let b = note(&mut d, PartKind::Endnote, "The second note.");
    let c = note(&mut d, PartKind::Footnote, "The third note.");
    let call = |text: &str, at: usize, kind: NoteKind, id: u32, custom: &str| {
        let mut p = Paragraph::with_text(text, CharProps::default());
        p.insert_object(at, InlineObject::NoteRef { kind, id, custom: custom.into() }, &CharProps::default()).unwrap();
        p
    };
    let mut one = call("Words that call two notes.", 5, NoteKind::Footnote, a, "");
    let end = one.text.len();
    one.insert_object(end, InlineObject::NoteRef { kind: NoteKind::Endnote, id: b, custom: "*".into() }, &CharProps::default()).unwrap();
    let two = call("More words, one note.", 4, NoteKind::Footnote, c, "");
    d.body = vec![
        para_block(Paragraph::with_text("One", CharProps::default()).styled("Heading1")),
        para_block(one),
        para_block(Paragraph::with_text("Two", CharProps::default()).styled("Heading1")),
        para_block(two),
    ];
    d
}

fn chapter(bytes: &[u8], n: usize) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut text = String::new();
    zip.by_name(&format!("OEBPS/chapter-{n:03}.xhtml")).unwrap().read_to_string(&mut text).unwrap();
    text
}

/// A hash of every entry's name and content, in the package's order: what the writer wrote,
/// whatever the compressor did with it.
fn sha(bytes: &[u8]) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut h = Sha256::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).unwrap();
        let mut content = Vec::new();
        f.read_to_end(&mut content).unwrap();
        h.update(f.name().as_bytes());
        h.update([0]);
        h.update(&content);
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Fail unless `xml` is well formed (quick-xml checks that end tags match).
fn well_formed(xml: &str) {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().check_end_names = true;
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(_) => {}
            Err(e) => panic!("not well formed: {e}\n{xml}"),
        }
    }
}

/// The letters and digits of some words, sorted: what a check that reads a file back compares
/// when marks stand directly against words.
fn letters<'a>(words: impl IntoIterator<Item = &'a String>) -> Vec<char> {
    let mut all: Vec<char> = words.into_iter().flat_map(|w| w.chars()).filter(|c| c.is_alphanumeric()).collect();
    all.sort_unstable();
    all
}

/// The document's own words: its text, then its notes.
fn source_words(doc: &Document) -> Vec<String> {
    let mut words = jcj_doc::verify::document_words(doc);
    for part in doc.parts.values() {
        words.extend(part.blocks.iter().filter_map(|b| b.as_para()).flat_map(jcj_doc::verify::paragraph_words));
    }
    words
}

/// What the writer wrote for `noted()` before it took options, hashed entry by entry.
const AS_ALWAYS: &str = "98f9fdff1242ae6a31f488552bc17b02bff8e7f32f2bffe14c96944919bc34d1";

#[test]
fn with_no_choice_made_the_file_is_what_it_always_was() {
    let doc = noted();
    let bytes = export(Format::Epub, &doc).unwrap();
    assert_eq!(sha(&bytes), AS_ALWAYS, "the default output changed");
    let same = export_epub(&doc, &EpubOptions::default()).unwrap();
    assert_eq!(same.bytes, bytes, "the default options are the old writer, byte for byte");
    // What it added: each number where it is called, then each again in the list with "Back".
    // The second note's own mark, `*`, holds no letter or digit and is no word.
    assert_eq!(same.added_words, ["1", "1", "Back", "Back", "1", "1", "Back"]);
    assert_eq!(EpubOptions::default().note_backlink, NoteBacklink::Text("Back".into()));
    assert_eq!(EpubOptions::default().note_numbering, NoteNumbering::PerChapter);
}

#[test]
fn the_caller_chooses_the_numbering_the_link_and_a_heading() {
    let options = EpubOptions::default()
        .note_numbering(NoteNumbering::Continuous)
        .note_backlink(NoteBacklink::Symbol { label: "Volver al texto".into() })
        .note_heading("Notas");
    let out = export_epub(&noted(), &options).unwrap();
    let (one, two) = (chapter(&out.bytes, 1), chapter(&out.bytes, 2));
    well_formed(&one);
    well_formed(&two);
    // The third note of the document is number 3 in the second chapter's file, where it is still
    // that file's first note; the list counts from 3 too.
    assert!(two.contains("id=\"noteref-1\" href=\"#note-1\"><sup>3</sup>"), "{two}");
    assert!(two.contains("<ol start=\"3\">") && two.contains("<span class=\"mark\">3</span>"), "{two}");
    assert!(one.contains("<ol>\n") && one.contains("<sup>*</sup>"), "{one}");
    for ch in [&one, &two] {
        assert!(ch.contains("role=\"doc-endnotes\">\n<h2>Notas</h2>\n"), "{ch}");
        assert!(ch.contains("role=\"doc-backlink\" aria-label=\"Volver al texto\" title=\"Volver al texto\">\u{21A9}</a>"), "{ch}");
        assert!(!ch.contains("Back"), "no word the caller did not choose: {ch}");
    }
    assert_eq!(out.added_words, ["1", "Notas", "1", "3", "Notas", "3"]);
}

#[test]
fn the_link_back_says_what_the_caller_says_or_is_left_out() {
    let said = export_epub(&noted(), &EpubOptions::default().note_backlink(NoteBacklink::Text("Volver <al> texto".into()))).unwrap();
    let ch = chapter(&said.bytes, 2);
    well_formed(&ch);
    assert!(ch.contains("role=\"doc-backlink\">Volver &lt;al&gt; texto</a>"), "{ch}");
    assert_eq!(said.added_words[said.added_words.len() - 4..], ["1", "Volver", "<al>", "texto"]);

    for none in [NoteBacklink::None, NoteBacklink::Text("  ".into())] {
        let out = export_epub(&noted(), &EpubOptions::default().note_backlink(none).note_heading("")).unwrap();
        let ch = chapter(&out.bytes, 1);
        well_formed(&ch);
        assert!(!ch.contains("doc-backlink") && !ch.contains("<h2>"), "{ch}");
        assert!(ch.contains("<p>The first note.</p>\n</li>"), "{ch}");
        assert_eq!(out.added_words, ["1", "1", "1", "1"], "only the numbers");
    }
    // A symbol with no name is still a link, and still adds no word.
    let bare = export_epub(&noted(), &EpubOptions::default().note_backlink(NoteBacklink::Symbol { label: String::new() })).unwrap();
    assert!(chapter(&bare.bytes, 1).contains("role=\"doc-backlink\">\u{21A9}</a>"));
    assert_eq!(bare.added_words, ["1", "1", "1", "1"]);
}

#[test]
fn a_check_of_the_file_leaves_out_exactly_the_added_words() {
    let doc = noted();
    for options in [
        EpubOptions::default(),
        EpubOptions::default().note_numbering(NoteNumbering::Continuous).note_heading("Notes to the chapter"),
        EpubOptions::default().note_backlink(NoteBacklink::Symbol { label: "Back to the text".into() }),
        EpubOptions::default().note_backlink(NoteBacklink::None),
    ] {
        let out = export_epub(&doc, &options).unwrap();
        let back = jcj_interchange::import(Format::Epub, &out.bytes).unwrap();
        let read = jcj_doc::verify::document_words(&back);
        let source = source_words(&doc);
        assert_eq!(letters(&read), letters(source.iter().chain(&out.added_words)), "{options:?}");
    }
}

#[test]
fn a_document_without_notes_is_unchanged_by_any_choice() {
    let mut doc = Document::new();
    doc.body = vec![para_block(Paragraph::with_text("No notes here.", CharProps::default()))];
    let plain = export(Format::Epub, &doc).unwrap();
    let chosen = export_epub(&doc, &EpubOptions::default().note_numbering(NoteNumbering::Continuous).note_heading("Notes")).unwrap();
    assert_eq!(chosen.bytes, plain);
    assert!(chosen.added_words.is_empty());
}
