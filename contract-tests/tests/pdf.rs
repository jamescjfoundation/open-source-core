// SPDX-License-Identifier: MIT OR Apache-2.0
//! PDF output is deterministic: the same document gives the same bytes, in this process and in
//! the next (the SHA-256 in `golden/sample-pdf.sha256`). Rewrite the golden file with
//! `UPDATE_GOLDEN=1 cargo test -p contract-tests --test pdf` when a change is intended.

use contract_tests::{check_golden, document, long_para, para, sha256_hex};
use jcj_doc::Pos;
use jcj_doc::props::{Align, CharProps};
use jcj_engine::Selection;
use jcj_layout::{LayoutCache, print_layout};
use jcj_output::{PdfSettings, export_pdf};
use serde_json::json;

/// Two pages: a centered bold title, body paragraphs, a drop cap, a footnote, a page break.
fn sample() -> jcj_doc::Document {
    let title_props = CharProps { bold: Some(true), size: Some(20.0), ..Default::default() };
    let mut title = jcj_doc::Paragraph::with_text("A Sample Document", title_props);
    title.props.align = Some(Align::Center);
    title.props.space_after = Some(12.0);
    let mut opening = long_para(4);
    opening.props.drop_cap = Some(2);
    let mut later = para("This paragraph starts the second page.");
    later.props.page_break_before = Some(true);
    let mut paras = vec![title, opening];
    paras.extend((0..4).map(|_| long_para(3)));
    paras.push(later);
    paras.push(long_para(2));
    let mut s = jcj_engine::session(document(paras));
    s.sel = Selection::caret(Pos::body(1, 3));
    s.run("references.footnote", &json!({"text": "A note at the foot of the first page."})).unwrap();
    let mut d = s.doc.clone();
    d.core.title = "A Sample Document".into();
    d
}

fn pdf() -> Vec<u8> {
    let d = sample();
    let l = print_layout(&d, &mut LayoutCache::new());
    assert_eq!(l.pages.len(), 2);
    export_pdf(&d, &l, &PdfSettings::deterministic()).unwrap()
}

#[test]
fn the_same_document_gives_the_same_bytes() {
    let a = pdf();
    let b = pdf();
    assert!(a.starts_with(b"%PDF-"));
    assert_eq!(a.len(), b.len());
    assert!(a == b, "two exports of the same document differ");
}

#[test]
fn the_sample_pdf_matches_its_golden_hash() {
    if let Err(e) = check_golden("sample-pdf.sha256", &sha256_hex(&pdf())) {
        panic!("{e}");
    }
}
