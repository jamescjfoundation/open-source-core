// SPDX-License-Identifier: MIT OR Apache-2.0
//! The watermark: text set diagonally behind every page, its strength adjustable (70 % by
//! default), with a recipient's name for tracking copies; and a name printed at the top of every
//! page for one printing (a copy made for one reader). Kept in the document under [`WATERMARK`]
//! (`core.watermark`).
//!
//! [`apply`] puts it on the pages of a paginated output document, as text boxes anchored to the
//! page (call it from a pagination pass's `finish`, or on any document whose pages start at
//! paragraphs that break the page). The text takes the document's own font. Pictures as
//! watermarks are not offered yet.
//!
//! Ported from the first foundation app's watermark.

use std::sync::Arc;

use jcj_doc::para::{Anchor, Float, ShapeKind, Wrap};
use jcj_doc::props::{Align, CharProps, LineSpacing, Rgb, TextColor};
use jcj_doc::{Block, Document, ExtKey, InlineObject, Paragraph, PartKind, para_block};
use jcj_engine::{CmdError, CmdResult, CommandSpec, Session, params as p};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::data::{clean_line, load, store};

/// Document data: the watermark settings ([`Watermark`]).
pub const WATERMARK: ExtKey<Watermark> = ExtKey::new("core.watermark");
/// Points per inch.
const INCH: f32 = 72.0;
/// Longest watermark text, recipient and page name, characters.
pub const MAX_TEXT_CHARS: usize = 60;
pub const MAX_NAME_CHARS: usize = 80;

/// The watermark settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Watermark {
    pub on: bool,
    pub text: String,
    /// 0 to 100: how strong the text is.
    pub opacity: u8,
    /// Who this copy is for.
    pub recipient: String,
    /// A name printed at the top of every page; set for one printing, cleared by
    /// `watermark.set`.
    pub page_name: String,
}

impl Default for Watermark {
    fn default() -> Self {
        Watermark { on: false, text: String::new(), opacity: 70, recipient: String::new(), page_name: String::new() }
    }
}

impl Watermark {
    /// The settings `doc` holds, or the defaults, within limits.
    pub fn of_doc(doc: &Document) -> Self {
        load(doc, WATERMARK).bounded()
    }

    fn bounded(mut self) -> Self {
        self.text = clean_line(&self.text, MAX_TEXT_CHARS);
        self.recipient = clean_line(&self.recipient, MAX_NAME_CHARS);
        self.page_name = clean_line(&self.page_name, MAX_NAME_CHARS);
        self.opacity = self.opacity.min(100);
        self
    }

    pub fn store(&self, doc: &mut Document) -> Result<(), CmdError> {
        store(doc, WATERMARK, self)
    }

    /// Whether the diagonal text shows.
    pub fn shows(&self) -> bool {
        self.on && (!self.text.is_empty() || !self.recipient.is_empty())
    }

    /// The grey the text is drawn in: stronger is darker, 100 % a mid grey, 0 % white.
    pub fn grey(&self) -> Rgb {
        let g = (255.0 - f32::from(self.opacity) * 1.1).clamp(130.0, 255.0) as u8;
        Rgb(g, g, g)
    }
}

fn line(text: &str, size: f32, bold: bool, color: Rgb, align: Align) -> Arc<Block> {
    let c = CharProps { size: Some(size), bold: bold.then_some(true), color: Some(TextColor::Rgb(color)), ..CharProps::default() };
    let mut p = Paragraph::with_text(text, c);
    p.props.align = Some(align);
    p.props.space_before = Some(0.0);
    p.props.space_after = Some(0.0);
    p.props.line_spacing = Some(LineSpacing::Exactly(size * 1.15));
    para_block(p)
}

/// A text box of `blocks` at `rect` (x, y, width, height, points from the page's top left).
fn text_box(out: &mut Document, blocks: Vec<Arc<Block>>, rect: (f32, f32, f32, f32), rot: f32, behind: bool) -> InlineObject {
    let (x, y, w, h) = rect;
    let story = out.add_part(PartKind::TextBox, blocks);
    if let Some(part) = out.parts.get_mut(&story) {
        part.body.inset = Some([0; 4]);
    }
    let float = Float {
        wrap: if behind { Wrap::BehindText } else { Wrap::InFrontOfText },
        h_rel: Anchor::Page,
        v_rel: Anchor::Page,
        x,
        y,
        rot,
        ..Float::default()
    };
    InlineObject::Shape {
        kind: ShapeKind::Rectangle,
        w,
        h,
        fill: None,
        stroke: None,
        stroke_width: 0.0,
        float,
        story: Some(story),
        freeform: None,
        effects: Default::default(),
        extra: Default::default(),
    }
}

/// The body blocks that start pages: the first, and each paragraph that breaks the page.
pub fn page_starts(out: &Document) -> Vec<usize> {
    out.body
        .iter()
        .enumerate()
        .filter(|(i, b)| *i == 0 || b.as_para().is_some_and(|p| p.props.page_break_before == Some(true)))
        .map(|(i, _)| i)
        .collect()
}

/// Put the watermark (and page name) on every page of the output document `out`. Text offsets
/// do not change: the text boxes are objects added at the end of each page's first paragraph.
pub fn apply(out: &mut Document, w: &Watermark) {
    if !w.shows() && w.page_name.is_empty() {
        return;
    }
    let (pw, ph) = (out.last_section.page_w, out.last_section.page_h);
    let grey = w.grey();
    for at in page_starts(out) {
        let mut objs = Vec::new();
        if w.shows() {
            let (bw, bh) = ((pw - INCH).max(INCH), 2.2 * INCH);
            let mut blocks = Vec::new();
            if !w.text.is_empty() {
                blocks.push(line(&w.text, 66.0, true, grey, Align::Center));
            }
            if !w.recipient.is_empty() {
                blocks.push(line(&w.recipient, 22.0, false, grey, Align::Center));
            }
            objs.push(text_box(out, blocks, ((pw - bw) / 2.0, (ph - bh) / 2.0, bw, bh), 315.0, true));
        }
        if !w.page_name.is_empty() {
            let left = out.last_section.margin_left;
            let blocks = vec![line(&w.page_name, 10.0, true, Rgb(0, 0, 0), Align::Left)];
            objs.push(text_box(out, blocks, (left, 0.5 * INCH, 3.0 * INCH, 14.0), 0.0, false));
        }
        if let Some(Block::Para(p)) = out.body.get_mut(at).map(Arc::make_mut) {
            for obj in objs {
                let end = p.len();
                let _ = p.insert_object(end, obj, &CharProps::default());
            }
            p.touch();
        }
    }
}

fn get(s: &mut Session, _: &Value) -> CmdResult {
    Ok(json!(Watermark::of_doc(&s.doc)))
}

/// `watermark.set {on?, text?, opacity?, recipient?}`.
fn set(s: &mut Session, v: &Value) -> CmdResult {
    let mut w = Watermark::of_doc(&s.doc);
    if let Some(b) = p::bool(v, "on") {
        w.on = b;
    }
    if let Some(t) = p::str(v, "text") {
        w.text = clean_line(t, MAX_TEXT_CHARS);
    }
    if let Some(r) = p::str(v, "recipient") {
        w.recipient = clean_line(r, MAX_NAME_CHARS);
    }
    if let Some(o) = v.get("opacity") {
        let o =
            o.as_u64().and_then(|o| u8::try_from(o).ok()).filter(|o| *o <= 100).ok_or_else(|| CmdError::Params("`opacity` is 0 to 100".into()))?;
        w.opacity = o;
    }
    w.page_name.clear();
    w.store(&mut s.doc)?;
    Ok(json!(w))
}

/// The watermark commands.
pub fn commands() -> Vec<CommandSpec> {
    const LOC: &str = "Design › Watermark";
    vec![
        CommandSpec::new("watermark.get", "Watermark", LOC, get).pure(),
        CommandSpec::new("watermark.set", "Set Watermark", LOC, set)
            .params(r#"{"on"?: bool, "text"?: string, "opacity"?: 0-100, "recipient"?: string}"#),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::session_with;
    use jcj_layout::paginate::{NoContinuations, PaginationPass, Source, paginate};

    /// A pass that puts the watermark on.
    struct WithWatermark;

    impl PaginationPass for WithWatermark {
        fn finish(&mut self, out: &mut Document, _: &[Option<Source>]) {
            let w = Watermark::of_doc(out);
            apply(out, &w);
        }
    }

    #[test]
    fn every_page_gets_the_watermark() {
        let lines: Vec<String> = (0..120).map(|i| format!("Line {i}.")).collect();
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let mut s = session_with(&refs);
        s.run("watermark.set", &json!({"on": true, "text": "DRAFT", "recipient": "Copy 7", "opacity": 40})).unwrap();
        let plain = paginate(&s.doc, &mut NoContinuations);
        let pag = paginate(&s.doc, &mut WithWatermark);
        assert!(pag.pages.len() >= 2);
        assert_eq!(pag.pages.len(), plain.pages.len(), "the watermark takes no room");
        let with: usize = pag
            .doc
            .body
            .iter()
            .filter_map(|b| b.as_para())
            .map(|p| p.objects.iter().filter(|o| matches!(o, InlineObject::Shape { float, .. } if float.rot == 315.0)).count())
            .sum();
        assert_eq!(with, pag.pages.len());
        assert!(s.run("watermark.set", &json!({"opacity": 101})).is_err());
        assert!(pag.matches_plan, "the output lays out on the planned pages");
    }

    #[test]
    fn settings_are_bounded() {
        let mut doc = Document::new();
        doc.ext.set("core.watermark", json!({"on": true, "text": "x".repeat(500), "opacity": 300})).unwrap();
        assert_eq!(Watermark::of_doc(&doc), Watermark::default(), "an opacity out of range reads as the defaults");
        doc.ext.set("core.watermark", json!({"on": true, "text": "x".repeat(500)})).unwrap();
        let w = Watermark::of_doc(&doc);
        assert_eq!(w.text.chars().count(), MAX_TEXT_CHARS);
        assert!(w.shows());
        assert_eq!(Watermark { opacity: 100, ..w.clone() }.grey(), Rgb(145, 145, 145));
    }
}
