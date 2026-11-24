// SPDX-License-Identifier: MIT OR Apache-2.0
//! RTF with direct formatting: what an app that lays out its own paragraphs writes comes back
//! when the RTF is read again.

use jcj_doc::props::{Align, CharProps, Underline};
use jcj_doc::{Document, ParaProps, Paragraph, Run, para_block};
use jcj_interchange::{Format, export, export_rtf_direct, import};

/// Written for this test: three paragraphs at their own indents, a bold and an underlined run,
/// a page break and text outside ASCII.
fn sample() -> Document {
    let mut d = Document::new();
    let mut first = Paragraph::with_text("INT. A ROOM - DAY", CharProps::default());
    first.props = ParaProps { indent_left: Some(0.0), space_before: Some(24.0), ..ParaProps::default() };

    let mut second = Paragraph::with_text("Bold then underlined, café {braces} \\ done.", CharProps::default());
    second.props = ParaProps {
        indent_left: Some(158.4),
        indent_first: Some(-18.0),
        space_before: Some(12.0),
        align: Some(Align::Center),
        ..ParaProps::default()
    };
    let bold = "Bold".len();
    let then = " then ".len();
    let under = "underlined".len();
    let rest = second.text.len() - bold - then - under;
    second.runs = vec![
        Run { len: bold, props: CharProps { bold: Some(true), ..CharProps::default() } },
        Run { len: then, props: CharProps { italic: Some(true), ..CharProps::default() } },
        Run { len: under, props: CharProps { underline: Some(Underline::Single), ..CharProps::default() } },
        Run { len: rest, props: CharProps::default() },
    ];

    let mut third = Paragraph::with_text("Next page.\nA second line.", CharProps::default());
    third.props = ParaProps { page_break_before: Some(true), indent_left: Some(72.0), ..ParaProps::default() };
    d.body = vec![para_block(first), para_block(second), para_block(third)];
    d
}

fn paragraphs(d: &Document) -> Vec<Paragraph> {
    jcj_doc::walk::body_paragraphs(d).into_iter().map(|(_, p)| p.clone()).collect()
}

/// The resolved bold, italic and underline of the run holding `text`.
fn run_style(d: &Document, p: &Paragraph, text: &str) -> Option<(bool, bool, bool)> {
    p.run_ranges().find(|(r, _)| p.text.get(r.clone()).is_some_and(|t| t.contains(text))).map(|(_, c)| {
        let rc = d.styles.resolve_char(p.props.style.as_deref(), c);
        (rc.bold, rc.italic, rc.underline != Underline::None)
    })
}

#[test]
fn indents_spacing_and_run_formatting_come_back() {
    let d = sample();
    let bytes = export_rtf_direct(&d).unwrap();
    let rtf = String::from_utf8(bytes.clone()).unwrap();
    assert!(rtf.starts_with("{\\rtf1"));
    assert!(rtf.is_ascii());
    assert!(rtf.contains("\\paperw12240"), "Letter width in twips");

    let back = import(Format::Rtf, &bytes).unwrap();
    let ps = paragraphs(&back);
    let texts: Vec<String> = ps.iter().map(|p| p.plain_text()).collect();
    assert_eq!(texts, ["INT. A ROOM - DAY", "Bold then underlined, café {braces} \\ done.", "Next page.\nA second line."]);

    let rp: Vec<_> = ps.iter().map(|p| back.styles.resolve_para(&p.props)).collect();
    assert_eq!(rp[0].indent_left, 0.0);
    assert_eq!(rp[0].space_before, 24.0);
    assert!(!rp[0].page_break_before);
    assert!((rp[1].indent_left - 158.4).abs() < 0.05);
    assert_eq!(rp[1].indent_first, -18.0);
    assert_eq!(rp[1].space_before, 12.0);
    assert_eq!(rp[1].align, Align::Center);
    assert_eq!(rp[2].indent_left, 72.0);
    assert!(rp[2].page_break_before);

    assert_eq!(run_style(&back, &ps[1], "Bold"), Some((true, false, false)));
    assert_eq!(run_style(&back, &ps[1], " then "), Some((false, true, false)));
    assert_eq!(run_style(&back, &ps[1], "underlined"), Some((false, false, true)));
    assert_eq!(run_style(&back, &ps[1], "café"), Some((false, false, false)));
}

#[test]
fn fonts_sizes_and_the_page_come_back() {
    let mut d = sample();
    {
        let s = std::sync::Arc::make_mut(&mut d.styles);
        s.default_chr.font = Some("Source Serif 4".into());
        s.default_chr.size = Some(12.0);
    }
    d.last_section.margin_left = 108.0;
    let bytes = export_rtf_direct(&d).unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("{\\f0\\fnil Source Serif 4;}"));
    let back = import(Format::Rtf, &bytes).unwrap();
    let ps = paragraphs(&back);
    let rc = back.styles.resolve_char(ps[0].props.style.as_deref(), &ps[0].runs[0].props);
    assert_eq!(rc.font, "Source Serif 4");
    assert_eq!(rc.size, 12.0);
    assert_eq!(back.last_section.margin_left, 108.0);
    assert_eq!(back.last_section.page_w, d.last_section.page_w);
}

#[test]
fn objects_are_dropped_and_the_plain_rtf_export_is_unchanged() {
    let mut d = sample();
    let mut p = Paragraph::with_text("before\u{FFFC}after", CharProps::default());
    p.objects.clear();
    d.body.push(para_block(p));
    let rtf = String::from_utf8(export_rtf_direct(&d).unwrap()).unwrap();
    assert!(rtf.contains("beforeafter"));
    // The ordinary RTF export still writes paragraph kinds, not these indents.
    let plain = String::from_utf8(export(Format::Rtf, &sample()).unwrap()).unwrap();
    assert!(!plain.contains("\\li3168"));
    assert!(rtf.contains("\\li3168"));
}
