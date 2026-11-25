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

mod facts {
    use super::doc;
    use crate::{PdfSettings, ReadError, ReadLimits, export_pdf_with_layout, read_facts};

    /// A PDF written by hand: one page with trim and bleed boxes, a link, a font that is not
    /// embedded, a 4 by 2 pixel image drawn one inch by half an inch, and two bookmarks, one
    /// inside the other.
    fn hand_made() -> Vec<u8> {
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R /Outlines 8 0 R >>".to_owned(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] /TrimBox [9 9 291 391] /BleedBox [0 0 300 400] \
             /Resources << /Font << /F1 4 0 R >> /XObject << /Im1 5 0 R >> >> /Contents 6 0 R /Annots [7 0 R] >>"
                .to_owned(),
            "<< /Type /Font /Subtype /Type1 /BaseFont /ABCDEF+Plain >>".to_owned(),
            "<< /Type /XObject /Subtype /Image /Width 4 /Height 2 /ColorSpace /DeviceGray /BitsPerComponent 8 /Length 8 >>\nstream\nabcdefgh\nendstream"
                .to_owned(),
            {
                let content = "q 72 0 0 36 50 50 cm /Im1 Do Q BT /F1 12 Tf 50 200 Td (Hi) Tj ET";
                format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len())
            },
            "<< /Type /Annot /Subtype /Link /Rect [0 0 10 10] >>".to_owned(),
            "<< /Type /Outlines /First 9 0 R /Last 9 0 R /Count 2 >>".to_owned(),
            "<< /Title (One) /Parent 8 0 R /First 10 0 R /Last 10 0 R /Count 1 >>".to_owned(),
            "<< /Title (Two) /Parent 9 0 R >>".to_owned(),
        ];
        let mut out = b"%PDF-1.7\n".to_vec();
        let mut offsets = Vec::new();
        for (i, o) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
        }
        let xref = out.len();
        out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
        for off in offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes());
        out
    }

    #[test]
    fn a_hand_made_pdf_gives_its_boxes_link_font_image_and_bookmarks() {
        let facts = read_facts(&hand_made(), &ReadLimits::default()).unwrap();
        assert_eq!(facts.pages.len(), 1);
        let p = &facts.pages[0];
        assert_eq!((p.media.width(), p.media.height()), (300.0, 400.0));
        assert_eq!(p.crop, p.media);
        let trim = p.trim.unwrap();
        assert_eq!((trim.width(), trim.height()), (282.0, 382.0));
        assert_eq!(p.bleed.unwrap().margins_around(&trim), [9.0; 4]);
        assert_eq!(p.rotation, 0);
        assert_eq!(p.annotations, vec!["Link".to_owned()]);
        assert_eq!(facts.annotation_count(), 1);
        assert_eq!(p.images.len(), 1);
        let im = p.images[0];
        assert_eq!((im.pixel_width, im.pixel_height), (4, 2));
        assert!((im.placed_width - 72.0).abs() < 1e-6 && (im.placed_height - 36.0).abs() < 1e-6, "{im:?}");
        assert!((im.ppi() - 4.0).abs() < 1e-6);
        assert_eq!(p.fonts, vec!["Plain".to_owned()]);
        assert_eq!(facts.fonts.len(), 1);
        assert!(facts.fonts[0].subset && !facts.fonts[0].embedded && facts.fonts[0].kind == "Type1");
        assert_eq!(facts.unembedded_fonts().len(), 1);
        assert_eq!(facts.bookmarks, 2);
    }

    #[test]
    fn an_exported_pdf_embeds_its_fonts_and_has_no_trim_box() {
        let d = doc();
        let bytes = export_pdf_with_layout(&d, &PdfSettings { links: false, ..PdfSettings::deterministic() }).unwrap();
        let facts = read_facts(&bytes, &ReadLimits::default()).unwrap();
        assert!(facts.pages.len() >= 2);
        assert!(!facts.fonts.is_empty());
        assert!(facts.unembedded_fonts().is_empty(), "{:?}", facts.fonts);
        for p in &facts.pages {
            assert!(p.trim.is_none() && p.bleed.is_none() && p.images.is_empty() && p.annotations.is_empty());
            assert!(p.media.width() > 500.0 && !p.fonts.is_empty());
        }
    }

    #[test]
    fn bad_input_and_limits_are_errors_not_panics() {
        let limits = ReadLimits::default();
        assert!(matches!(read_facts(b"", &limits), Err(ReadError::NotPdf(_))));
        let bytes = hand_made();
        for n in (0..bytes.len()).step_by(7) {
            let _ = read_facts(&bytes[..n], &limits);
        }
        assert_eq!(read_facts(&bytes, &ReadLimits { max_input_bytes: 10, ..limits }), Err(ReadError::TooLarge(10)));
        let two = export_pdf_with_layout(&doc(), &PdfSettings::deterministic()).unwrap();
        assert_eq!(read_facts(&two, &ReadLimits { max_pages: 1, ..limits }), Err(ReadError::TooManyPages(1)));
    }
}

mod boxes {
    use super::doc;
    use crate::{BoxError, Boxes, PageBox, PdfSettings, ReadLimits, export_pdf_with_layout, read_facts, read_pages, set_page_boxes};

    #[test]
    fn trim_and_bleed_boxes_are_set_and_read_back() {
        let pdf = export_pdf_with_layout(&doc(), &PdfSettings::deterministic()).unwrap();
        let limits = ReadLimits::default();
        let before = read_facts(&pdf, &limits).unwrap();
        let boxes: Vec<Boxes> = before
            .pages
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let m = p.media;
                // Bleed on the outside, top and bottom: the trim sits against the spine side.
                let trim = if i % 2 == 0 {
                    PageBox { x0: m.x0, y0: m.y0 + 9.0, x1: m.x1 - 9.0, y1: m.y1 - 9.0 }
                } else {
                    PageBox { x0: m.x0 + 9.0, y0: m.y0 + 9.0, x1: m.x1, y1: m.y1 - 9.0 }
                };
                Boxes { trim, bleed: Some(m) }
            })
            .collect();
        let out = set_page_boxes(&pdf, &boxes, &limits).unwrap();
        assert!(out.starts_with(&pdf), "the original bytes are kept");
        assert_eq!(out, set_page_boxes(&pdf, &boxes, &limits).unwrap(), "the same bytes every time");
        let after = read_facts(&out, &limits).unwrap();
        for (p, b) in after.pages.iter().zip(&boxes) {
            assert_eq!(p.trim, Some(b.trim));
            assert_eq!(p.bleed, b.bleed);
            assert_eq!(p.media, b.bleed.unwrap());
        }
        assert_eq!(after.fonts, before.fonts);
        // The text reads back as before.
        let (a, b) = (read_pages(&pdf, &limits).unwrap(), read_pages(&out, &limits).unwrap());
        assert_eq!(a, b);
        // Not twice, not the wrong count, not on rubbish.
        assert_eq!(set_page_boxes(&out, &boxes, &limits), Err(BoxError::HasBoxes(1)));
        assert_eq!(set_page_boxes(&pdf, &boxes[..1], &limits), Err(BoxError::Count(boxes.len(), 1)));
        assert!(matches!(set_page_boxes(b"not a pdf", &[], &limits), Err(BoxError::Read(_))));
        let trim_only: Vec<Boxes> = boxes.iter().map(|b| Boxes { bleed: None, ..*b }).collect();
        let out = set_page_boxes(&pdf, &trim_only, &limits).unwrap();
        assert!(read_facts(&out, &limits).unwrap().pages.iter().all(|p| p.bleed.is_none() && p.trim.is_some()));
    }
}
