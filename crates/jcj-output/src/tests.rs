// SPDX-License-Identifier: MIT OR Apache-2.0
use std::sync::Arc;

use jcj_doc::props::{CharProps, LineSpacing};
use jcj_doc::{Document, Paragraph, para_block};
use jcj_layout::{LayoutCache, Rect, print_layout};

use crate::{OutputError, PdfSettings, RenderSettings, export_pdf, export_pdf_with_layout, render_area, render_page};

/// Two pages of text written for these tests, on a bundled font.
fn doc() -> Document {
    let mut d = Document::new();
    let st = Arc::make_mut(&mut d.styles);
    st.default_chr.font = Some("Source Serif 4".into());
    st.default_para.line_spacing = Some(LineSpacing::Multiple(1.0));
    d.body = (0..70).map(|i| para_block(Paragraph::with_text(&format!("Paragraph number {i} of the sample."), CharProps::default()))).collect();
    d.core.title = "Sample".into();
    d
}

#[test]
fn the_same_document_gives_the_same_pdf_bytes() {
    let d = doc();
    let l = print_layout(&d, &mut LayoutCache::new());
    assert!(l.pages.len() >= 2);
    let s = PdfSettings::deterministic();
    let a = export_pdf(&d, &l, &s).unwrap();
    let b = export_pdf(&d, &print_layout(&d, &mut LayoutCache::new()), &s).unwrap();
    assert!(a.starts_with(b"%PDF-"));
    assert_eq!(a, b);
    // Laying out inside the export gives the same file too.
    assert_eq!(export_pdf_with_layout(&d, &s).unwrap(), a);
}

#[test]
fn a_page_list_writes_those_pages_and_refuses_missing_ones() {
    let d = doc();
    let l = print_layout(&d, &mut LayoutCache::new());
    let all = export_pdf(&d, &l, &PdfSettings::deterministic()).unwrap();
    let one = export_pdf(&d, &l, &PdfSettings { pages: Some(vec![1]), ..PdfSettings::deterministic() }).unwrap();
    assert!(one.len() < all.len());
    let err = export_pdf(&d, &l, &PdfSettings { pages: Some(vec![99]), ..PdfSettings::deterministic() }).unwrap_err();
    assert_eq!(err, OutputError::BadPage(100));
    assert!(!err.to_string().is_empty());
    assert_eq!(export_pdf(&d, &l, &PdfSettings { pages: Some(vec![]), ..PdfSettings::deterministic() }), Err(OutputError::NoPages));
}

#[test]
fn a_page_image_is_paper_with_ink_on_it() {
    let d = doc();
    let l = print_layout(&d, &mut LayoutCache::new());
    let img = render_page(&d, &l, 0, 0.5, &RenderSettings::default()).unwrap();
    assert_eq!(img.width, (d.last_section.page_w * 0.5).ceil() as u32);
    assert_eq!(img.pixels.len(), (img.width * img.height * 4) as usize);
    // White paper in the corner, dark ink somewhere in the text.
    assert_eq!(img.pixel(0, 0), Some([255, 255, 255, 255]));
    assert!(img.pixels.chunks(4).any(|p| p[0] < 128));
    assert!(img.to_png().starts_with(b"\x89PNG"));
    assert_eq!(img.straight_rgba().len(), img.pixels.len());
    // The same page drawn twice is the same image; a dark page is dark.
    assert_eq!(render_page(&d, &l, 0, 0.5, &RenderSettings::default()).unwrap(), img);
    let dark = render_page(&d, &l, 0, 0.5, &RenderSettings { dark: true, ..Default::default() }).unwrap();
    assert!(dark.pixel(0, 0).unwrap()[0] < 128);
    assert!(render_page(&d, &l, 99, 1.0, &RenderSettings::default()).is_none());
}

#[test]
fn an_area_image_has_the_area_size() {
    let d = doc();
    let l = print_layout(&d, &mut LayoutCache::new());
    let img = render_area(&d, &l, 0, Rect::new(72.0, 72.0, 100.0, 50.0), 2.0, &RenderSettings::default()).unwrap();
    assert_eq!((img.width, img.height), (200, 100));
    assert_eq!(img.pixel(200, 0), None);
}
