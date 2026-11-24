// SPDX-License-Identifier: MIT OR Apache-2.0
//! Headers and footers with fields, kept in the document under [`HEADER_FOOTER`]
//! (`core.header_footer`).
//!
//! Each band (header, footer) has a left, a center and a right text. Fields in braces fill in on
//! each page: `{page}` the page number (a locked page's label), `{date}` the date (today, or the
//! fixed text set), `{title}` the document's title, `{revision}` the current revision set's name
//! (empty until there is one), `{collated}` every revision set with marks in the document. An
//! app adds its own fields through a [`FieldSource`] (a `{section}` field showing the heading the
//! page is in, say). Other text is printed as it is. Page 1 shows the bands only when asked. The
//! default is the page number and a period at the top right, none on page 1.
//!
//! [`apply`] writes the bands into a paginated output document as its header and footer parts.
//!
//! Ported from the first foundation app's headers and footers (its decision 0021); its one
//! app-specific field became the [`FieldSource`] hook.

use std::sync::Arc;

use jcj_doc::props::{Align, CharProps, TabAlign, TabLeader, TabStop};
use jcj_doc::{Block, Document, ExtKey, InlineObject, Paragraph, PartKind, para_block};
use jcj_engine::{CmdError, CmdResult, CommandSpec, Session};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::data::{load, store};

/// Document data: the header and footer settings ([`HeaderFooter`]).
pub const HEADER_FOOTER: ExtKey<HeaderFooter> = ExtKey::new("core.header_footer");
/// Longest text one slot keeps, characters.
pub const MAX_SLOT_CHARS: usize = 200;
/// The fields every document has.
pub const FIELDS: [&str; 5] = ["page", "date", "title", "revision", "collated"];
/// Points per inch.
const INCH: f32 = 72.0;

/// What a field shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldValue {
    /// Fixed text.
    Text(String),
    /// A live field the layout fills in on each page: its instruction (`PAGE`,
    /// `STYLEREF "Heading 1"`).
    Live(String),
}

/// Fields an app adds to the bands (`{section}`). Asked first, so an app may also change what a
/// built-in field shows.
pub trait FieldSource {
    /// What `{name}` shows in `doc`; `None` when the app has no such field.
    fn field(&self, name: &str, doc: &Document) -> Option<FieldValue>;
    /// The names of the app's fields, for a settings window.
    fn names(&self) -> Vec<String> {
        Vec::new()
    }
}

/// No fields of the app's own.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoFields;

impl FieldSource for NoFields {
    fn field(&self, _: &str, _: &Document) -> Option<FieldValue> {
        None
    }
}

/// One band's three slots.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Band {
    pub left: String,
    pub center: String,
    pub right: String,
}

impl Band {
    pub fn is_empty(&self) -> bool {
        self.left.trim().is_empty() && self.center.trim().is_empty() && self.right.trim().is_empty()
    }

    fn cleaned(&self) -> Band {
        let c = |s: &str| s.chars().filter(|c| !c.is_control()).take(MAX_SLOT_CHARS).collect::<String>();
        Band { left: c(&self.left), center: c(&self.center), right: c(&self.right) }
    }
}

/// The header and footer settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HeaderFooter {
    pub header: Band,
    pub footer: Band,
    /// Show the bands on page 1 too.
    pub first_page: bool,
    /// The text `{date}` shows; empty for today's date.
    pub date: String,
    /// Show the footer on page 1, apart from the header (`None`: as `first_page` says).
    pub first_page_footer: Option<bool>,
    /// The number of the first page (documents without page locks), 1 by default.
    pub start_page: u32,
}

impl Default for HeaderFooter {
    fn default() -> Self {
        HeaderFooter {
            header: Band { right: "{page}.".into(), ..Band::default() },
            footer: Band::default(),
            first_page: false,
            date: String::new(),
            first_page_footer: None,
            start_page: 1,
        }
    }
}

impl HeaderFooter {
    /// The settings `doc` holds, or the defaults; bad data reads as the defaults, and texts are
    /// cut to their limits.
    pub fn of_doc(doc: &Document) -> Self {
        let mut h = load(doc, HEADER_FOOTER);
        h.header = h.header.cleaned();
        h.footer = h.footer.cleaned();
        h.date = h.date.chars().filter(|c| !c.is_control()).take(MAX_SLOT_CHARS).collect();
        h.start_page = h.start_page.clamp(1, 100_000);
        h
    }

    pub fn store(&self, doc: &mut Document) -> Result<(), CmdError> {
        store(doc, HEADER_FOOTER, self)
    }
}

/// `headerFooter.get`: the settings.
fn get(s: &mut Session, _: &Value) -> CmdResult {
    Ok(serde_json::to_value(HeaderFooter::of_doc(&s.doc)).unwrap_or_default())
}

/// `headerFooter.set {header?, footer?, firstPage?, firstPageFooter?, startPage?, date?}`: change
/// the settings given (one undo step).
fn set(s: &mut Session, v: &Value) -> CmdResult {
    let mut merged = serde_json::to_value(HeaderFooter::of_doc(&s.doc)).unwrap_or_default();
    let Some(obj) = v.as_object() else { return Err(CmdError::Params("expected an object".into())) };
    for (k, val) in obj {
        match (k.as_str(), val) {
            ("header" | "footer", Value::Object(slots)) => {
                for (slot, text) in slots {
                    if !matches!(slot.as_str(), "left" | "center" | "right") || !text.is_string() {
                        return Err(CmdError::Params(format!("`{k}.{slot}` must be left, center or right, as text")));
                    }
                    if let Some(band) = merged.get_mut(k).and_then(Value::as_object_mut) {
                        band.insert(slot.clone(), text.clone());
                    }
                }
            }
            ("firstPage", Value::Bool(_)) | ("date", Value::String(_)) | ("firstPageFooter", Value::Bool(_) | Value::Null) => {
                if let Some(m) = merged.as_object_mut() {
                    m.insert(k.clone(), val.clone());
                }
            }
            // A window's number field may send 5.0.
            ("startPage", Value::Number(n)) => {
                if let Some(m) = merged.as_object_mut() {
                    m.insert(k.clone(), serde_json::json!(n.as_f64().unwrap_or(1.0).clamp(1.0, 100_000.0) as u32));
                }
            }
            _ => return Err(CmdError::Params(format!("unknown or bad setting `{k}`"))),
        }
    }
    let mut h: HeaderFooter = serde_json::from_value(merged).map_err(|e| CmdError::Params(e.to_string()))?;
    h.header = h.header.cleaned();
    h.footer = h.footer.cleaned();
    h.store(&mut s.doc)?;
    Ok(serde_json::to_value(&h).unwrap_or_default())
}

/// The header and footer commands.
pub fn commands() -> Vec<CommandSpec> {
    const LOC: &str = "Insert › Header and Footer";
    vec![
        CommandSpec::new("headerFooter.get", "Header and Footer Settings", LOC, get).pure(),
        CommandSpec::new("headerFooter.set", "Set Header and Footer", LOC, set).params(
            r#"{"header"?: {"left"?, "center"?, "right"?: text with {page} {date} {title} {revision} {collated} and the app's fields}, "footer"?: {...}, "firstPage"?: bool, "firstPageFooter"?: bool|null, "startPage"?: number, "date"?: string (empty for today)}"#,
        ),
    ]
}

fn field(instr: &str) -> InlineObject {
    InlineObject::Field { instr: instr.into(), result: String::new(), locked: false, dirty: false, code: false, form: String::new() }
}

/// What the bands need to fill fields in.
struct Fill<'a> {
    doc: &'a Document,
    date: String,
    revision: String,
    collated: String,
    fields: &'a dyn FieldSource,
}

impl Fill<'_> {
    /// What `{name}` shows on a page labelled `label` (`None`: the live page number).
    fn value(&self, name: &str, label: Option<&str>) -> Option<FieldValue> {
        if let Some(v) = self.fields.field(name, self.doc) {
            return Some(v);
        }
        Some(match name {
            "page" => match label {
                Some(l) => FieldValue::Text(l.to_string()),
                None => FieldValue::Live("PAGE".into()),
            },
            "date" => FieldValue::Text(self.date.clone()),
            "title" => FieldValue::Text(self.doc.core.title.clone()),
            "revision" => FieldValue::Text(self.revision.clone()),
            "collated" => FieldValue::Text(self.collated.clone()),
            _ => return None,
        })
    }

    /// Append `text` to `p`, its fields filled in or made live.
    fn push_slot(&self, p: &mut Paragraph, text: &str, label: Option<&str>) {
        let plain = CharProps::default();
        let mut rest = text;
        while !rest.is_empty() {
            let next = rest.find('{').and_then(|open| rest.get(open..).and_then(|r| r.find('}')).map(|close| (open, open + close)));
            let Some((open, close)) = next else {
                let at = p.len();
                let _ = p.insert_text(at, rest, &plain);
                break;
            };
            let at = p.len();
            let _ = p.insert_text(at, rest.get(..open).unwrap_or(""), &plain);
            let name = rest.get(open + 1..close).unwrap_or("");
            let at = p.len();
            match self.value(name, label) {
                Some(FieldValue::Text(t)) => {
                    let _ = p.insert_text(at, &t, &plain);
                }
                Some(FieldValue::Live(instr)) => {
                    let _ = p.insert_object(at, field(&instr), &plain);
                }
                None => {
                    let _ = p.insert_text(at, rest.get(open..=close).unwrap_or(""), &plain);
                }
            }
            rest = rest.get(close + 1..).unwrap_or("");
        }
    }

    /// The line for `band`: one slot alone is aligned where it belongs; more use tab stops at the
    /// center and the right margin.
    fn band_line(&self, band: &Band, style: &str, label: Option<&str>) -> Paragraph {
        let mut p = Paragraph::new();
        p.props.style = Some(style.into());
        let used: Vec<(&str, Align)> = [(&band.left, Align::Left), (&band.center, Align::Center), (&band.right, Align::Right)]
            .into_iter()
            .filter(|(t, _)| !t.trim().is_empty())
            .map(|(t, a)| (t.as_str(), a))
            .collect();
        if let [(text, align)] = used.as_slice() {
            p.props.align = Some(*align);
            self.push_slot(&mut p, text, label);
            return p;
        }
        let page = &self.doc.last_section;
        let width = (page.page_w - page.margin_left - page.margin_right).max(INCH);
        p.props.tabs = Some(vec![
            TabStop { pos: width / 2.0, align: TabAlign::Center, leader: TabLeader::None },
            TabStop { pos: width, align: TabAlign::Right, leader: TabLeader::None },
        ]);
        self.push_slot(&mut p, &band.left, label);
        for slot in [&band.center, &band.right] {
            let at = p.len();
            let _ = p.insert_text(at, "\t", &CharProps::default());
            self.push_slot(&mut p, slot, label);
        }
        p
    }
}

/// The header line for `band` in `doc` (for previews and tests): fields filled in, `{page}` as
/// `label` or the live page number.
pub fn band_line(doc: &Document, band: &Band, label: Option<&str>, fields: &dyn FieldSource) -> Paragraph {
    let h = HeaderFooter::of_doc(doc);
    fill_for(doc, &h, fields).band_line(band, "Header", label)
}

fn fill_for<'a>(doc: &'a Document, h: &HeaderFooter, fields: &'a dyn FieldSource) -> Fill<'a> {
    let date = if h.date.trim().is_empty() { crate::dates::today() } else { h.date.clone() };
    let rev = crate::revisions::Revisions::of_doc(doc);
    Fill { doc, date, revision: rev.current.clone(), collated: crate::revisions::collated(doc), fields }
}

/// Give the output document `doc` its header and footer, as its settings say. With `labels` (a
/// locked document: one per page), each page is a section of its own whose header and footer
/// show its label for `{page}`; pages start where a paragraph breaks the page. The lines use the
/// document's Header and Footer paragraph styles.
pub fn apply(doc: &mut Document, labels: Option<&[String]>, fields: &dyn FieldSource) {
    let h = HeaderFooter::of_doc(doc);
    let source = doc.clone();
    let fill = fill_for(&source, &h, fields);
    let blank = |style: &str| {
        let mut p = Paragraph::new();
        p.props.style = Some(style.into());
        para_block(p)
    };
    let Some(labels) = labels else {
        let header = (!h.header.is_empty()).then(|| doc.add_part(PartKind::Header, vec![para_block(fill.band_line(&h.header, "Header", None))]));
        let footer = (!h.footer.is_empty()).then(|| doc.add_part(PartKind::Footer, vec![para_block(fill.band_line(&h.footer, "Footer", None))]));
        let first_header = (!h.first_page).then(|| doc.add_part(PartKind::Header, vec![blank("Header")]));
        let footer_first = h.first_page_footer.unwrap_or(h.first_page);
        let first_footer = (!footer_first).then(|| doc.add_part(PartKind::Footer, vec![blank("Footer")]));
        let s = &mut doc.last_section;
        if h.start_page != 1 {
            s.page_num_start = Some(h.start_page);
            s.page_num_set = true;
        }
        s.headers.default = header;
        s.footers.default = footer;
        s.headers.even = None;
        s.footers.even = None;
        s.title_page = !h.first_page || !footer_first;
        s.headers.first = if h.first_page { header } else { first_header };
        s.footers.first = if footer_first { footer } else { first_footer };
        s.header = 0.5 * INCH;
        s.footer = 0.5 * INCH;
        return;
    };
    // One section per page: the last paragraph of each page but the last ends one.
    let starts = crate::watermark::page_starts(doc);
    let mut base = doc.last_section.clone();
    base.headers = Default::default();
    base.footers = Default::default();
    base.title_page = false;
    base.header = 0.5 * INCH;
    base.footer = 0.5 * INCH;
    let mut sections = Vec::with_capacity(starts.len());
    for page in 0..starts.len() {
        let label = labels.get(page).map_or("", String::as_str);
        let mut sect = base.clone();
        // Page 1 shows the bands only when asked.
        let show = page > 0 || h.first_page;
        let header = if show && !h.header.is_empty() { para_block(fill.band_line(&h.header, "Header", Some(label))) } else { blank("Header") };
        sect.headers.default = Some(doc.add_part(PartKind::Header, vec![header]));
        let show_footer = page > 0 || h.first_page_footer.unwrap_or(h.first_page);
        let footer = if show_footer && !h.footer.is_empty() { para_block(fill.band_line(&h.footer, "Footer", Some(label))) } else { blank("Footer") };
        sect.footers.default = Some(doc.add_part(PartKind::Footer, vec![footer]));
        sections.push(sect);
    }
    let Some(last) = sections.pop() else { return };
    for (i, sect) in sections.into_iter().enumerate() {
        let end = starts.get(i + 1).map_or(0, |n| n.saturating_sub(1));
        if let Some(Block::Para(p)) = doc.body.get_mut(end).map(Arc::make_mut) {
            p.section = Some(Box::new(sect));
            p.touch();
        }
    }
    doc.last_section = last;
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::tests::session_with;

    /// An app with a `{section}` field: the heading the page is in.
    struct Sections;

    impl FieldSource for Sections {
        fn field(&self, name: &str, _: &Document) -> Option<FieldValue> {
            (name == "section").then(|| FieldValue::Live("STYLEREF \"Heading 1\"".into()))
        }
        fn names(&self) -> Vec<String> {
            vec!["section".into()]
        }
    }

    #[test]
    fn settings_read_bounded_and_set_in_one_step() {
        let mut s = session_with(&["One."]);
        assert_eq!(HeaderFooter::of_doc(&s.doc), HeaderFooter::default());
        s.run("headerFooter.set", &json!({"footer": {"center": "{title} - {date}"}, "firstPage": true})).unwrap();
        let h = HeaderFooter::of_doc(&s.doc);
        assert_eq!(h.footer.center, "{title} - {date}");
        assert_eq!(h.header.right, "{page}.", "the rest is kept");
        assert!(h.first_page);
        assert!(s.run("headerFooter.set", &json!({"header": {"middle": "x"}})).is_err());
        assert!(s.run("headerFooter.set", &json!({"colour": 1})).is_err());
        s.run("edit.undo", &json!({})).unwrap();
        assert_eq!(HeaderFooter::of_doc(&s.doc), HeaderFooter::default());
        let mut doc = Document::new();
        doc.ext.set("core.header_footer", json!({"header": {"left": "x".repeat(1000)}, "firstPage": "yes"})).unwrap();
        assert_eq!(HeaderFooter::of_doc(&doc), HeaderFooter::default(), "a bad value reads as the defaults");
        doc.ext.set("core.header_footer", json!({"header": {"left": "x".repeat(1000)}})).unwrap();
        assert_eq!(HeaderFooter::of_doc(&doc).header.left.len(), MAX_SLOT_CHARS);
    }

    #[test]
    fn slots_fill_fields_and_the_app_adds_its_own() {
        let mut s = session_with(&["One."]);
        s.doc.core.title = "THE TEST".into();
        s.run("revision.next", &json!({})).unwrap();
        s.run("headerFooter.set", &json!({"date": "1 May"})).unwrap();
        let p = band_line(&s.doc, &Band { left: "{title} {revision} {date} {nope} {section}".into(), ..Band::default() }, None, &NoFields);
        assert_eq!(p.text, "THE TEST Blue 1 May {nope} {section}");
        assert_eq!(p.props.align, Some(Align::Left));
        let p = band_line(&s.doc, &Band { left: "A".into(), center: "{section}".into(), right: "{page}.".into() }, None, &Sections);
        assert_eq!(p.text.matches('\t').count(), 2);
        assert_eq!(p.objects.len(), 2, "section and page are live fields");
        let p = band_line(&s.doc, &Band { right: "{page}.".into(), ..Band::default() }, Some("12A"), &Sections);
        assert_eq!(p.text, "12A.");
        assert_eq!(Sections.names(), ["section"]);
    }

    #[test]
    fn start_page_and_first_page_footer() {
        let mut s = session_with(&["One."]);
        s.run("headerFooter.set", &json!({"startPage": 5.0, "firstPageFooter": true, "footer": {"left": "{collated}"}})).unwrap();
        let h = HeaderFooter::of_doc(&s.doc);
        assert_eq!((h.start_page, h.first_page_footer), (5, Some(true)));
        let mut out = s.doc.clone();
        apply(&mut out, None, &NoFields);
        assert_eq!(out.last_section.page_num_start, Some(5));
        assert!(out.last_section.footers.first.is_some());
        assert!(out.last_section.headers.default.is_some());
        assert!(s.run("headerFooter.set", &json!({"startPage": "five"})).is_err());
    }

    #[test]
    fn labelled_pages_get_a_section_each() {
        let mut s = session_with(&["One.", "Two.", "Three."]);
        if let Some(Block::Para(p)) = s.doc.body.get_mut(2).map(Arc::make_mut) {
            p.props.page_break_before = Some(true);
        }
        let mut out = s.doc.clone();
        apply(&mut out, Some(&["1".to_string(), "1A".to_string()]), &NoFields);
        assert!(out.body[1].as_para().is_some_and(|p| p.section.is_some()), "page 1 ends a section");
        let header = out.last_section.headers.default.and_then(|id| out.parts.get(&id)).and_then(|part| part.blocks.first().cloned());
        assert_eq!(header.and_then(|b| b.as_para().map(|p| p.text.clone())).as_deref(), Some("1A."));
    }
}
