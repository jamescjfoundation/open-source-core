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

mod read {
    use jcj_layout::{LayoutCache, print_layout};

    use super::doc;
    use crate::{GlyphMode, PdfPage, PdfSettings, ReadError, ReadLimits, export_pdf_with_layout, read_pages};

    /// A page's lines: glyphs grouped by where they land (y), in drawing order.
    fn lines(page: &PdfPage) -> Vec<(f64, String)> {
        let mut out: Vec<(f64, String)> = Vec::new();
        for g in page.glyphs.iter().filter(|g| g.mode == GlyphMode::Fill) {
            let text = g.text.as_deref().unwrap_or("\u{FFFD}");
            match out.last_mut() {
                Some((y, line)) if (*y - g.y).abs() < 1.0 => line.push_str(text),
                _ => out.push((g.y, text.to_owned())),
            }
        }
        out
    }

    fn squash(s: &str) -> String {
        s.chars().filter(|c| !c.is_whitespace()).collect()
    }

    #[test]
    fn an_exported_pdf_reads_back_as_its_text() {
        let d = doc();
        let bytes = export_pdf_with_layout(&d, &PdfSettings::deterministic()).unwrap();
        let pages = read_pages(&bytes, &ReadLimits::default()).unwrap();
        assert_eq!(pages.len(), print_layout(&d, &mut LayoutCache::new()).pages.len());
        let mut all = Vec::new();
        for page in &pages {
            assert!((page.width - f64::from(d.last_section.page_w)).abs() < 0.5);
            assert!(page.height > page.width);
            let lines = lines(page);
            // Top to bottom: PDF y grows upward.
            assert!(lines.windows(2).all(|w| w[1].0 < w[0].0), "lines go down the page");
            for g in &page.glyphs {
                assert!(g.x >= 0.0 && g.x <= page.width && g.y >= 0.0 && g.y <= page.height);
            }
            all.extend(lines.into_iter().map(|(_, l)| l));
        }
        let want: Vec<String> = (0..70).map(|i| squash(&format!("Paragraph number {i} of the sample."))).collect();
        assert_eq!(all.iter().map(|l| squash(l)).collect::<Vec<_>>(), want);
    }

    #[test]
    fn glyphs_on_a_line_move_right() {
        let bytes = export_pdf_with_layout(&doc(), &PdfSettings::deterministic()).unwrap();
        let pages = read_pages(&bytes, &ReadLimits::default()).unwrap();
        let first: Vec<_> = pages[0].glyphs.iter().take_while(|g| (g.y - pages[0].glyphs[0].y).abs() < 1.0).collect();
        assert!(first.len() > 10);
        assert!(first.windows(2).all(|w| w[1].x > w[0].x));
        // The core's export draws every glyph from the page's own transform, so they share it.
        assert!(pages[0].glyphs.iter().all(|g| g.line_y == pages[0].glyphs[0].line_y));
    }

    #[test]
    fn bad_input_is_an_error_not_a_panic() {
        let limits = ReadLimits::default();
        assert!(matches!(read_pages(b"", &limits), Err(ReadError::NotPdf(_))));
        assert!(matches!(read_pages(b"not a pdf at all, just words", &limits), Err(ReadError::NotPdf(_))));
        let bytes = export_pdf_with_layout(&doc(), &PdfSettings::deterministic()).unwrap();
        // Cut short: an error when too little is left, never a panic (the reader repairs what it can).
        assert!(read_pages(&bytes[..8], &limits).is_err());
        assert!(read_pages(&bytes[..100], &limits).is_err());
        for n in [bytes.len() / 3, bytes.len() / 2, bytes.len() - 10] {
            let _ = read_pages(&bytes[..n], &limits);
        }
        // Bytes changed in the middle.
        let mut broken = bytes.clone();
        for b in broken.iter_mut().skip(bytes.len() / 4).step_by(97) {
            *b ^= 0x5a;
        }
        let _ = read_pages(&broken, &limits);
    }

    #[test]
    fn limits_are_errors() {
        let bytes = export_pdf_with_layout(&doc(), &PdfSettings::deterministic()).unwrap();
        let small = ReadLimits { max_input_bytes: 100, ..ReadLimits::default() };
        assert_eq!(read_pages(&bytes, &small), Err(ReadError::TooLarge(100)));
        let one_page = ReadLimits { max_pages: 1, ..ReadLimits::default() };
        assert_eq!(read_pages(&bytes, &one_page), Err(ReadError::TooManyPages(1)));
        let few = ReadLimits { max_glyphs_per_page: 5, ..ReadLimits::default() };
        let err = read_pages(&bytes, &few).unwrap_err();
        assert_eq!(err, ReadError::TooManyGlyphs { page: 1, limit: 5 });
        assert!(!err.to_string().is_empty());
    }
}
