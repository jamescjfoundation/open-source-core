// SPDX-License-Identifier: MIT OR Apache-2.0
//! PDF/A-2b and PDF/UA-1 for an exported PDF: the declarations and pieces the standards ask for
//! that the writer does not add itself.
//!
//! [`declare`] writes them as an incremental update (the original bytes kept, like
//! [`crate::set_page_boxes`]): the XMP metadata again with the standards' identification
//! (`pdfaid`, `pdfuaid`), the author and the document's language; the catalog again with an
//! output intent (an sRGB profile, [`srgb_profile`]) for PDF/A, `DisplayDocTitle` for PDF/UA and
//! the document's language; and the information dictionary again with the author, so it agrees
//! with the XMP. The same input gives the same output.
//!
//! It declares; it does not make a file conform. The writer's PDFs are tagged and embed their
//! fonts, which the rest of both standards needs; check the result with a validator.

use std::fmt::Write as _;

use crate::boxes::{trailer, without_prev};
use crate::read::{ReadError, ReadLimits};

/// What to declare and the facts the declarations carry.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Conformance {
    /// Declare PDF/A-2b (an output intent and the `pdfaid` identification).
    pub pdf_a: bool,
    /// Declare PDF/UA-1 (`DisplayDocTitle` and the `pdfuaid` identification).
    pub pdf_ua: bool,
    /// The author, written into the XMP and the information dictionary when not empty.
    pub author: String,
    /// The document's language (BCP 47, `en-GB`), set on the catalog and in the XMP when not
    /// empty.
    pub language: String,
}

/// Why the declarations could not be written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConformanceError {
    /// The PDF could not be read.
    Read(ReadError),
    /// The PDF is not one this can update (encrypted, cross-references in a stream, no XMP
    /// metadata or catalog where expected).
    Unsupported(&'static str),
}

impl std::fmt::Display for ConformanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConformanceError::Read(e) => e.fmt(f),
            ConformanceError::Unsupported(why) => write!(f, "the standards cannot be declared: {why}"),
        }
    }
}

impl std::error::Error for ConformanceError {}

fn big_endian(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_be_bytes());
}

/// An s15Fixed16 number.
fn fixed(v: f64) -> u32 {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "colorimetry values within ±16")]
    let n = (v * 65536.0).round() as i32;
    n as u32
}

/// A small ICC version 2 display profile for sRGB (D50 colorants and white point as the
/// standard's Bradford-adapted primaries, a 2.2 tone curve), made here so no profile file needs
/// shipping. Enough for an output intent; not for colour management of images.
pub fn srgb_profile() -> Vec<u8> {
    let xyz = |x: f64, y: f64, z: f64| {
        let mut t = b"XYZ \0\0\0\0".to_vec();
        for v in [x, y, z] {
            big_endian(&mut t, fixed(v));
        }
        t
    };
    let text = |s: &str| {
        let mut t = b"text\0\0\0\0".to_vec();
        t.extend_from_slice(s.as_bytes());
        t.push(0);
        t
    };
    let desc = |s: &str| {
        let mut t = b"desc\0\0\0\0".to_vec();
        big_endian(&mut t, u32::try_from(s.len() + 1).unwrap_or(0));
        t.extend_from_slice(s.as_bytes());
        t.push(0);
        // No Unicode or ScriptCode description.
        t.extend_from_slice(&[0; 8]);
        t.extend_from_slice(&[0; 3]);
        t.extend_from_slice(&[0; 67]);
        t
    };
    // A gamma of 2.2 as u8Fixed8 (0x0233).
    let curve = b"curv\0\0\0\0\0\0\0\x01\x02\x33".to_vec();
    let tags: Vec<(&[u8; 4], Vec<u8>)> = vec![
        (b"desc", desc("sRGB")),
        (b"cprt", text("No copyright: public domain")),
        (b"wtpt", xyz(0.9642, 1.0, 0.8249)),
        (b"rXYZ", xyz(0.4361, 0.2225, 0.0139)),
        (b"gXYZ", xyz(0.3851, 0.7169, 0.0971)),
        (b"bXYZ", xyz(0.1431, 0.0606, 0.7141)),
        (b"rTRC", curve.clone()),
        (b"gTRC", curve.clone()),
        (b"bTRC", curve),
    ];
    let table_len = 4 + 12 * tags.len();
    let mut offset = 128 + table_len;
    let mut table = Vec::new();
    let mut data = Vec::new();
    big_endian(&mut table, u32::try_from(tags.len()).unwrap_or(0));
    for (sig, body) in &tags {
        while !offset.is_multiple_of(4) {
            data.push(0);
            offset += 1;
        }
        table.extend_from_slice(*sig);
        big_endian(&mut table, u32::try_from(offset).unwrap_or(0));
        big_endian(&mut table, u32::try_from(body.len()).unwrap_or(0));
        data.extend_from_slice(body);
        offset += body.len();
    }
    let size = 128 + table.len() + data.len();
    let mut out = Vec::with_capacity(size);
    big_endian(&mut out, u32::try_from(size).unwrap_or(0));
    out.extend_from_slice(b"none"); // preferred CMM: none
    out.extend_from_slice(&[2, 0x10, 0, 0]); // version 2.1
    out.extend_from_slice(b"mntr"); // display device profile
    out.extend_from_slice(b"RGB ");
    out.extend_from_slice(b"XYZ ");
    out.extend_from_slice(&[0; 12]); // date
    out.extend_from_slice(b"acsp");
    out.extend_from_slice(&[0; 4]); // platform
    out.extend_from_slice(&[0; 4]); // flags
    out.extend_from_slice(&[0; 4]); // manufacturer
    out.extend_from_slice(&[0; 4]); // model
    out.extend_from_slice(&[0; 8]); // attributes
    out.extend_from_slice(&[0; 4]); // rendering intent: perceptual
    for v in [0.9642, 1.0, 0.8249] {
        big_endian(&mut out, fixed(v)); // the PCS illuminant, D50
    }
    out.extend_from_slice(&[0; 4]); // creator
    out.extend_from_slice(&[0; 44]); // id and reserved
    out.extend_from_slice(&table);
    out.extend_from_slice(&data);
    out
}

/// Each object's offset from a classic cross-reference section at `at`, and the section before
/// it (`/Prev`), until the first.
fn offsets(bytes: &[u8], mut at: usize) -> Option<std::collections::BTreeMap<u32, usize>> {
    let mut out = std::collections::BTreeMap::new();
    for _ in 0..64 {
        // The section, its trailer and `startxref` are ASCII; what follows may not be.
        let tail = bytes.get(at..)?;
        let end = tail.windows(9).position(|w| w == b"startxref")?;
        let text = std::str::from_utf8(&tail[..end]).ok()?;
        let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
        if lines.next()? != "xref" {
            return None;
        }
        loop {
            let head = lines.next()?;
            if head.starts_with("trailer") {
                break;
            }
            let mut h = head.split_whitespace();
            let (first, count): (u32, u32) = (h.next()?.parse().ok()?, h.next()?.parse().ok()?);
            for n in 0..count {
                let e = lines.next()?;
                let mut f = e.split_whitespace();
                let off: usize = f.next()?.parse().ok()?;
                let _gen = f.next()?;
                if f.next()? == "n" {
                    out.entry(first + n).or_insert(off);
                }
            }
        }
        let t = text.find("trailer")?;
        let rest = &text[t..];
        match rest.find("/Prev").and_then(|i| rest[i + 5..].split_whitespace().next()?.trim_end_matches(">>").parse::<usize>().ok()) {
            Some(prev) => at = prev,
            None => return Some(out),
        }
    }
    Some(out)
}

/// The number after `/key` in a dictionary's text, as `N G R` (the object number).
fn reference(dict: &str, key: &str) -> Option<u32> {
    let i = dict.find(key)?;
    let rest = dict[i + key.len()..].trim_start();
    let end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// An object's text, `N 0 obj` to `endobj`, at `off`.
fn object(bytes: &[u8], off: usize) -> Option<&[u8]> {
    let b = bytes.get(off..)?;
    let end = b.windows(6).position(|w| w == b"endobj")?;
    Some(&b[..end])
}

/// A dictionary object's inner text (between its outermost `<<` and `>>`).
fn dict_inner(obj: &[u8]) -> Option<String> {
    let s = std::str::from_utf8(obj).ok()?;
    let a = s.find("<<")?;
    let b = s.rfind(">>")?;
    (b > a).then(|| s[a + 2..b].to_owned())
}

/// A PDF text string (literal, escaped).
fn pdf_string(s: &str) -> String {
    if s.is_ascii() {
        let mut out = String::from("(");
        for c in s.chars() {
            if matches!(c, '(' | ')' | '\\') {
                out.push('\\');
            }
            out.push(c);
        }
        out.push(')');
        return out;
    }
    // UTF-16BE with a byte order mark, as hex.
    let mut out = String::from("<FEFF");
    for u in s.encode_utf16() {
        let _ = write!(out, "{u:04X}");
    }
    out.push('>');
    out
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Remove `/key value` from a dictionary's text when the value is a simple one: a name, a
/// number, a string or a reference (not a nested dictionary or array).
fn drop_simple(dict: &str, key: &str) -> String {
    let Some(i) = dict.find(&format!("{key}(")).or_else(|| dict.find(&format!("{key} ("))).or_else(|| dict.find(&format!("{key}/"))) else {
        return dict.to_owned();
    };
    let rest = &dict[i + key.len()..];
    let skip = rest.len() - rest.trim_start().len();
    let v = &rest[skip..];
    let len = if v.starts_with('(') {
        let mut depth = 0;
        let mut esc = false;
        let mut n = 0;
        for (k, c) in v.char_indices() {
            n = k + c.len_utf8();
            if esc {
                esc = false;
                continue;
            }
            match c {
                '\\' => esc = true,
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
        n
    } else {
        1 + v[1..].find(|c: char| c.is_whitespace() || matches!(c, '/' | '>' | '<' | '[' | '(')).unwrap_or(v.len() - 1)
    };
    format!("{}{}", &dict[..i], &rest[skip + len..])
}

/// Declare PDF/A-2b and PDF/UA-1 on a PDF the core's writer made (see the module).
pub fn declare(bytes: &[u8], c: &Conformance, limits: &ReadLimits) -> Result<Vec<u8>, ConformanceError> {
    if bytes.len() > limits.max_input_bytes {
        return Err(ConformanceError::Read(ReadError::TooLarge(limits.max_input_bytes)));
    }
    if !bytes.starts_with(b"%PDF-") {
        return Err(ConformanceError::Read(ReadError::NotPdf("it does not start as a PDF".into())));
    }
    let (prev, trailer_text) = trailer(bytes).ok_or(ConformanceError::Unsupported("the PDF's trailer could not be found"))?;
    if trailer_text.contains("/Encrypt") {
        return Err(ConformanceError::Unsupported("the PDF is encrypted"));
    }
    if !bytes.get(prev..).is_some_and(|b| b.starts_with(b"xref")) {
        return Err(ConformanceError::Unsupported("the PDF keeps its cross-references in a stream"));
    }
    let offs = offsets(bytes, prev).ok_or(ConformanceError::Unsupported("the PDF's cross-references could not be read"))?;
    let size: u32 = reference(&trailer_text, "/Size").ok_or(ConformanceError::Unsupported("the PDF's trailer has no size"))?;
    let root = reference(&trailer_text, "/Root").ok_or(ConformanceError::Unsupported("the PDF's trailer names no catalog"))?;
    let info = reference(&trailer_text, "/Info");
    let catalog_obj = offs.get(&root).and_then(|o| object(bytes, *o)).ok_or(ConformanceError::Unsupported("the catalog could not be found"))?;
    let catalog = dict_inner(catalog_obj).ok_or(ConformanceError::Unsupported("the catalog could not be read"))?;
    let meta = reference(&catalog, "/Metadata").ok_or(ConformanceError::Unsupported("the PDF has no XMP metadata"))?;
    let meta_obj = offs.get(&meta).and_then(|o| object(bytes, *o)).ok_or(ConformanceError::Unsupported("the XMP metadata could not be found"))?;
    let xmp = {
        let s = meta_obj.windows(6).position(|w| w == b"stream").ok_or(ConformanceError::Unsupported("the XMP metadata is not a stream"))?;
        let dict = std::str::from_utf8(&meta_obj[..s]).unwrap_or("");
        if dict.contains("/Filter") {
            return Err(ConformanceError::Unsupported("the XMP metadata is compressed"));
        }
        let mut body = &meta_obj[s + 6..];
        if body.starts_with(b"\r\n") {
            body = &body[2..];
        } else if body.starts_with(b"\n") {
            body = &body[1..];
        }
        let end = body.windows(9).rposition(|w| w == b"endstream").unwrap_or(body.len());
        String::from_utf8(body[..end].to_vec()).map_err(|_| ConformanceError::Unsupported("the XMP metadata is not UTF-8"))?
    };
    // The XMP: the identifications, the author and the language.
    let mut x = xmp.trim_end().to_owned();
    let close = x.rfind("</rdf:RDF>").ok_or(ConformanceError::Unsupported("the XMP metadata could not be read"))?;
    let mut added = String::new();
    if c.pdf_a {
        added.push_str(r#"<rdf:Description rdf:about="" xmlns:pdfaid="http://www.aiim.org/pdfa/ns/id/"><pdfaid:part>2</pdfaid:part><pdfaid:conformance>B</pdfaid:conformance></rdf:Description>"#);
    }
    if c.pdf_ua {
        added.push_str(
            r#"<rdf:Description rdf:about="" xmlns:pdfuaid="http://www.aiim.org/pdfua/ns/id/"><pdfuaid:part>1</pdfuaid:part></rdf:Description>"#,
        );
        // PDF/A allows only properties of schemas it knows or that the file declares: declare
        // PDF/UA's.
        if c.pdf_a {
            added.push_str(concat!(
                r#"<rdf:Description rdf:about="" xmlns:pdfaExtension="http://www.aiim.org/pdfa/ns/extension/" "#,
                r#"xmlns:pdfaSchema="http://www.aiim.org/pdfa/ns/schema#" xmlns:pdfaProperty="http://www.aiim.org/pdfa/ns/property#">"#,
                r#"<pdfaExtension:schemas><rdf:Bag><rdf:li rdf:parseType="Resource">"#,
                r#"<pdfaSchema:schema>PDF/UA Universal Accessibility Schema</pdfaSchema:schema>"#,
                r#"<pdfaSchema:namespaceURI>http://www.aiim.org/pdfua/ns/id/</pdfaSchema:namespaceURI>"#,
                r#"<pdfaSchema:prefix>pdfuaid</pdfaSchema:prefix>"#,
                r#"<pdfaSchema:property><rdf:Seq><rdf:li rdf:parseType="Resource">"#,
                r#"<pdfaProperty:name>part</pdfaProperty:name><pdfaProperty:valueType>Integer</pdfaProperty:valueType>"#,
                r#"<pdfaProperty:category>internal</pdfaProperty:category>"#,
                r#"<pdfaProperty:description>Indicates which part of ISO 14289 the file follows</pdfaProperty:description>"#,
                r#"</rdf:li></rdf:Seq></pdfaSchema:property></rdf:li></rdf:Bag></pdfaExtension:schemas></rdf:Description>"#,
            ));
        }
    }
    if !c.author.trim().is_empty() && !x.contains("<dc:creator>") {
        let _ = write!(
            added,
            r#"<rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:creator><rdf:Seq><rdf:li>{}</rdf:li></rdf:Seq></dc:creator></rdf:Description>"#,
            xml(c.author.trim())
        );
    }
    x.insert_str(close, &added);
    if !c.language.trim().is_empty()
        && let (Some(a), Some(b)) = (x.find("<dc:language><rdf:Bag><rdf:li>"), x.find("</rdf:li></rdf:Bag></dc:language>"))
        && a < b
    {
        let from = a + "<dc:language><rdf:Bag><rdf:li>".len();
        x.replace_range(from..b, &xml(c.language.trim()));
    }
    // Room to grow in place, as XMP packets have.
    let padding = " ".repeat(64);
    let xmp_bytes = format!("{x}{padding}");
    let mut out = bytes.to_vec();
    if !out.ends_with(b"\n") {
        out.push(b'\n');
    }
    let mut written: Vec<(u32, usize)> = Vec::new();
    let mut obj = |out: &mut Vec<u8>, num: u32, body: &[u8]| {
        written.push((num, out.len()));
        out.extend_from_slice(format!("{num} 0 obj\n").as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };
    obj(&mut out, meta, format!("<< /Type /Metadata /Subtype /XML /Length {} >>\nstream\n{xmp_bytes}\nendstream", xmp_bytes.len()).as_bytes());
    // The catalog.
    let mut cat = catalog.clone();
    if !c.language.trim().is_empty() {
        cat = drop_simple(&cat, "/Lang");
        let _ = write!(cat, " /Lang {}", pdf_string(c.language.trim()));
    }
    if c.pdf_ua && !cat.contains("/ViewerPreferences") {
        cat.push_str(" /ViewerPreferences << /DisplayDocTitle true >>");
    }
    let mut next = size;
    if c.pdf_a && !cat.contains("/OutputIntents") {
        let (icc, intent) = (next, next + 1);
        next += 2;
        let profile = srgb_profile();
        let mut body = format!("<< /N 3 /Length {} >>\nstream\n", profile.len()).into_bytes();
        body.extend_from_slice(&profile);
        body.extend_from_slice(b"\nendstream");
        obj(&mut out, icc, &body);
        obj(
            &mut out,
            intent,
            format!("<< /Type /OutputIntent /S /GTS_PDFA1 /OutputConditionIdentifier (sRGB) /Info (sRGB) /DestOutputProfile {icc} 0 R >>").as_bytes(),
        );
        let _ = write!(cat, " /OutputIntents [{intent} 0 R]");
    }
    obj(&mut out, root, format!("<<{cat}>>").as_bytes());
    // The information dictionary, with the author.
    if let Some(info) = info
        && !c.author.trim().is_empty()
        && let Some(d) = offs.get(&info).and_then(|o| object(bytes, *o)).and_then(dict_inner)
        && !d.contains("/Author")
    {
        obj(&mut out, info, format!("<<{d} /Author {}>>", pdf_string(c.author.trim())).as_bytes());
    }
    let entries = without_prev(&trailer_text).ok_or(ConformanceError::Unsupported("the PDF's trailer could not be read"))?;
    let entries = if next > size { entries.replacen(&format!("/Size {size}"), &format!("/Size {next}"), 1) } else { entries };
    written.sort_by_key(|w| w.0);
    let xref = out.len();
    let mut s = String::from("xref\n");
    for (num, off) in &written {
        let _ = write!(s, "{num} 1\n{off:010} 00000 n \n");
    }
    let _ = write!(s, "trailer\n<< {entries} /Prev {prev} >>\nstartxref\n{xref}\n%%EOF\n");
    out.extend_from_slice(s.as_bytes());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_profile_is_a_well_formed_icc_profile() {
        let p = srgb_profile();
        assert_eq!(u32::from_be_bytes([p[0], p[1], p[2], p[3]]) as usize, p.len());
        assert_eq!(&p[36..40], b"acsp");
        assert_eq!(&p[12..20], b"mntrRGB ");
        let tags = u32::from_be_bytes([p[128], p[129], p[130], p[131]]) as usize;
        assert_eq!(tags, 9);
        for t in 0..tags {
            let at = 132 + 12 * t;
            let off = u32::from_be_bytes([p[at + 4], p[at + 5], p[at + 6], p[at + 7]]) as usize;
            let len = u32::from_be_bytes([p[at + 8], p[at + 9], p[at + 10], p[at + 11]]) as usize;
            assert!(off.is_multiple_of(4) && off + len <= p.len(), "tag {t}");
        }
        assert_eq!(srgb_profile(), p, "the same bytes every time");
    }

    #[test]
    fn simple_values_are_dropped_from_a_dictionary() {
        assert_eq!(drop_simple("/Type/Catalog/Lang(en-US)/Pages 1 0 R", "/Lang"), "/Type/Catalog/Pages 1 0 R");
        assert_eq!(drop_simple("/Lang (a\\)b) /X 1", "/Lang"), " /X 1");
        assert_eq!(pdf_string("Gulliver (1726)"), "(Gulliver \\(1726\\))");
        assert_eq!(pdf_string("Ünd"), "<FEFF00DC006E0064>");
    }
}
