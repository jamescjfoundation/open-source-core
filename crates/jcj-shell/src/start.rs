// SPDX-License-Identifier: MIT OR Apache-2.0
//! The start window and the template chooser.
//!
//! An app that makes new documents or projects from templates lists them with
//! [`crate::App::templates`]. The shell then shows them on the File page's New page, and, when the
//! app turns on [`crate::ShellOptions::start_window`], in a start window drawn in place of the
//! document whenever the app says it has none open ([`crate::App::has_document`]): the templates,
//! Open, and the recent files. Picking a template runs [`ids::NEW_FROM_TEMPLATE`] with
//! `{"template": id}`; the app builds the new document.

use egui::{Align2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use jcj_i18n::tr;
use jcj_theme::{Theme, medium, regular, semibold};
use serde_json::json;

use crate::app::App;
use crate::commands::{Cmd, ids};
use crate::shell::Shell;

/// A template an app offers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TemplateSpec {
    /// The id passed back in `{"template": id}`.
    pub id: String,
    /// The name on its tile.
    pub name: String,
    /// One or two lines on what it holds.
    pub about: String,
    /// A heading templates are grouped under (empty: no heading).
    pub group: String,
    /// What the template holds, drawn on its tile; `None` for a tile with its name and
    /// description only.
    pub preview: Option<TemplatePreview>,
}

impl TemplateSpec {
    /// A template with an id, a name and a description.
    pub fn new(id: impl Into<String>, name: impl Into<String>, about: impl Into<String>) -> Self {
        TemplateSpec { id: id.into(), name: name.into(), about: about.into(), group: String::new(), preview: None }
    }

    /// The same template under a group heading.
    pub fn in_group(mut self, group: impl Into<String>) -> Self {
        self.group = group.into();
        self
    }

    /// The same template with a preview of what it holds: its tile is taller
    /// ([`TILE_WITH_PREVIEW`]) and shows it above the name.
    pub fn with_preview(mut self, preview: TemplatePreview) -> Self {
        self.preview = Some(preview);
        self
    }
}

/// Most lines a template's preview keeps.
pub const PREVIEW_LINES: usize = 6;
/// Most characters a line of a preview keeps.
pub const PREVIEW_LINE_CHARS: usize = 120;

/// A preview of what a template holds: its first lines, drawn as text on a small page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TemplatePreview {
    lines: Vec<String>,
    monospace: bool,
}

impl TemplatePreview {
    /// The template's first lines. Empty lines are kept, and at most [`PREVIEW_LINES`] of
    /// [`PREVIEW_LINE_CHARS`] characters each; a line holding line breaks counts as several.
    /// Text is shown as it is given, not translated.
    pub fn lines<S: AsRef<str>>(lines: impl IntoIterator<Item = S>) -> Self {
        let lines = lines
            .into_iter()
            .flat_map(|l| {
                // An empty line is a line too (`"".lines()` gives none).
                let text = l.as_ref();
                let parts: Vec<&str> = if text.is_empty() { vec![""] } else { text.lines().collect() };
                parts.into_iter().map(|l| l.chars().take(PREVIEW_LINE_CHARS).collect::<String>()).collect::<Vec<_>>()
            })
            .take(PREVIEW_LINES)
            .collect();
        TemplatePreview { lines, monospace: false }
    }

    /// The same lines in a fixed-width font, for a template whose text is laid out by columns.
    pub fn monospace(mut self) -> Self {
        self.monospace = true;
        self
    }

    /// The lines the tile shows.
    pub fn text_lines(&self) -> &[String] {
        &self.lines
    }

    /// Whether they are shown in a fixed-width font.
    pub fn is_monospace(&self) -> bool {
        self.monospace
    }
}

/// The size of a template tile.
pub const TILE: egui::Vec2 = vec2(200.0, 92.0);
/// The size of a template tile with a preview ([`TemplateSpec::with_preview`]).
pub const TILE_WITH_PREVIEW: egui::Vec2 = vec2(210.0, 150.0);
/// The height of the small page a preview is drawn on.
const PREVIEW_PAGE: f32 = 80.0;

/// Draw the templates as tiles, grouped under their headings. Returns the id of the one clicked
/// (and queues [`ids::NEW_FROM_TEMPLATE`] for it).
pub fn chooser(ui: &mut Ui, shell: &mut Shell, templates: &[TemplateSpec]) -> Option<String> {
    let t = Theme::get(ui.ctx());
    let mut picked = None;
    let mut groups: Vec<&str> = Vec::new();
    for tpl in templates {
        if !groups.contains(&tpl.group.as_str()) {
            groups.push(&tpl.group);
        }
    }
    for group in groups {
        if !group.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new(tr(group)).font(medium(t.type_sizes.body)).color(t.text_dim));
        }
        ui.horizontal_wrapped(|ui| {
            for tpl in templates.iter().filter(|x| x.group == group) {
                let (rect, resp) = ui.allocate_exact_size(if tpl.preview.is_some() { TILE_WITH_PREVIEW } else { TILE }, Sense::click());
                let fill = if resp.hovered() { t.hover } else { t.surface_high };
                ui.painter().rect(rect, t.radius.card, fill, Stroke::new(1.0, t.border), StrokeKind::Inside);
                // A preview takes the top of the tile; the name and description go under it.
                let top = match &tpl.preview {
                    Some(preview) => paint_preview(ui, &t, preview, rect) + 10.0,
                    None => rect.min.y,
                };
                ui.painter().text(pos2(rect.min.x + 12.0, top + 18.0), Align2::LEFT_CENTER, tr(&tpl.name), semibold(t.type_sizes.title), t.text);
                let about = ui.painter().layout(tr(&tpl.about), regular(t.type_sizes.caption), t.text_dim, rect.width() - 24.0);
                ui.painter().with_clip_rect(rect.shrink(4.0)).galley(pos2(rect.min.x + 12.0, top + 34.0), about, t.text_dim);
                let name = tr(&tpl.name);
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
                crate::widgets::record(shell, Some(ids::NEW_FROM_TEMPLATE), rect);
                if resp.clicked() {
                    picked = Some(tpl.id.clone());
                }
            }
        });
    }
    if let Some(id) = &picked {
        shell.chrome.file_page = None;
        shell.run(Cmd::with(ids::NEW_FROM_TEMPLATE, json!({ "template": id })));
    }
    picked
}

/// Draw `preview` on a small page at the top of the tile `rect`; returns the page's bottom.
fn paint_preview(ui: &Ui, t: &Theme, preview: &TemplatePreview, rect: Rect) -> f32 {
    // Paper, as a document's page is drawn, whatever the interface's theme.
    let (paper, ink) = (egui::Color32::WHITE, egui::Color32::from_gray(60));
    let page = Rect::from_min_size(rect.min + vec2(12.0, 12.0), vec2(rect.width() - 24.0, PREVIEW_PAGE));
    ui.painter().rect(page, 3.0, paper, Stroke::new(1.0, t.border), StrokeKind::Inside);
    let size = (t.type_sizes.caption - 2.0).max(7.0);
    let font = if preview.monospace { jcj_theme::mono(size) } else { regular(size) };
    let mut job = egui::text::LayoutJob::simple(preview.lines.join("\n"), font, ink, page.width() - 12.0);
    job.wrap.max_rows = PREVIEW_LINES;
    let galley = ui.painter().layout_job(job);
    ui.painter().with_clip_rect(page.shrink(1.0)).galley(page.min + vec2(6.0, 6.0), galley, ink);
    page.max.y
}

/// Draw the start window: the app's name, its templates, Open, and the recent files.
pub fn start_window<A: App>(ui: &mut Ui, shell: &mut Shell, app: &mut A) {
    let t = Theme::get(ui.ctx());
    let full = ui.max_rect();
    let content = Rect::from_min_max(pos2(full.min.x + 40.0, full.min.y + 28.0), pos2(full.max.x - 40.0, full.max.y - 16.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(content), |ui| {
        egui::ScrollArea::vertical().id_salt("jcj-start").auto_shrink(false).show(ui, |ui| {
            let w = app.mark_width(28.0);
            ui.horizontal(|ui| {
                if w > 0.0 {
                    let (r, _) = ui.allocate_exact_size(vec2(w, 28.0), Sense::hover());
                    app.paint_mark(ui.painter(), r, t.brand);
                    ui.add_space(8.0);
                }
                ui.label(RichText::new(&shell.name).font(semibold(t.type_sizes.heading)).color(t.text));
            });
            ui.add_space(14.0);
            let templates = app.templates();
            if !templates.is_empty() {
                ui.label(RichText::new(tr("Start something new")).font(medium(t.type_sizes.title)).color(t.text));
                ui.add_space(4.0);
                chooser(ui, shell, &templates);
                ui.add_space(16.0);
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new(tr("Open")).font(medium(t.type_sizes.title)).color(t.text));
                ui.add_space(8.0);
                let b = ui.button(tr("Open…"));
                crate::widgets::record(shell, Some(ids::OPEN), b.rect);
                if b.clicked() {
                    shell.run(Cmd::new(ids::OPEN));
                }
            });
            ui.add_space(6.0);
            if shell.recent.is_empty() {
                ui.label(RichText::new(tr("Files you open or save show here.")).color(t.text_dim));
            }
            for r in shell.recent.clone().into_iter().take(10) {
                let name = std::path::Path::new(&r).file_name().and_then(|n| n.to_str()).unwrap_or(&r).to_string();
                let link = ui.link(RichText::new(&name).font(medium(t.type_sizes.body)));
                crate::widgets::record(shell, Some(ids::OPEN_PATH), link.rect);
                if link.on_hover_text(&r).clicked() {
                    shell.run(Cmd::with(ids::OPEN_PATH, json!({ "path": r })));
                }
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{CommandInfo, Commands};
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    struct Blank {
        open: bool,
        ran: Vec<Cmd>,
    }

    impl Commands for Blank {
        fn command(&self, id: &str) -> Option<CommandInfo> {
            self.all_commands().into_iter().find(|c| c.id == id)
        }
        fn all_commands(&self) -> Vec<CommandInfo> {
            vec![CommandInfo::new(ids::NEW_FROM_TEMPLATE, "New from Template", ""), CommandInfo::new(ids::OPEN, "Open", "")]
        }
    }

    impl App for Blank {
        fn document_name(&self) -> String {
            "Untitled".into()
        }
        fn run(&mut self, cmd: &Cmd, _shell: &mut Shell) -> Result<(), String> {
            self.ran.push(cmd.clone());
            if cmd.id == ids::NEW_FROM_TEMPLATE {
                self.open = true;
            }
            Ok(())
        }
        fn document_ui(&mut self, ui: &mut Ui, _shell: &mut Shell) {
            ui.label("The document");
        }
        fn has_document(&self) -> bool {
            self.open
        }
        fn templates(&self) -> Vec<TemplateSpec> {
            vec![
                TemplateSpec::new("plain", "Plain", "An empty document.").in_group("General"),
                TemplateSpec::new("letter", "Letter", "A letter.").in_group("General"),
            ]
        }
    }

    struct State {
        shell: Shell,
        app: Blank,
    }

    fn harness(start_window: bool) -> Harness<'static, State> {
        let shell = Shell::new("Test App", "1.0", jcj_theme::Tokens::default())
            .with_options(crate::ShellOptions { start_window, ..crate::ShellOptions::default() });
        Harness::builder()
            .with_size(vec2(1000.0, 700.0))
            .build_ui_state(|ui, s: &mut State| s.shell.ui(ui, &mut s.app), State { shell, app: Blank { open: false, ran: Vec::new() } })
    }

    #[test]
    fn the_start_window_shows_templates_and_picking_one_runs_the_command() {
        let mut h = harness(true);
        h.run();
        assert!(h.query_by_label("The document").is_none(), "no document yet: the start window shows");
        h.get_by_label("Letter").click();
        h.run();
        let ran = &h.state().app.ran;
        assert!(ran.iter().any(|c| c.id == ids::NEW_FROM_TEMPLATE && c.params == json!({ "template": "letter" })));
        assert!(h.query_by_label("The document").is_some(), "once the app has a document it is shown");
    }

    #[test]
    fn a_preview_keeps_a_few_short_lines() {
        let long = "x".repeat(PREVIEW_LINE_CHARS + 10);
        let p = TemplatePreview::lines(["Title", "", "First line\nA second line", long.as_str(), "5", "6", "7"]);
        assert_eq!(p.text_lines().len(), PREVIEW_LINES);
        assert_eq!(&p.text_lines()[..4], ["Title", "", "First line", "A second line"]);
        assert_eq!(p.text_lines()[4].chars().count(), PREVIEW_LINE_CHARS);
        assert!(!p.is_monospace() && p.clone().monospace().is_monospace());
        let tpl = TemplateSpec::new("a", "A", "About.").with_preview(p.clone());
        assert_eq!(tpl.preview, Some(p));
        assert_eq!(TemplateSpec::new("a", "A", "About.").preview, None, "no preview unless the app gives one");
    }

    #[test]
    fn a_tile_with_a_preview_is_taller_and_one_without_is_as_before() {
        struct Previewed(Blank);
        impl Commands for Previewed {
            fn command(&self, id: &str) -> Option<CommandInfo> {
                self.0.command(id)
            }
        }
        impl App for Previewed {
            fn document_name(&self) -> String {
                self.0.document_name()
            }
            fn run(&mut self, cmd: &Cmd, shell: &mut Shell) -> Result<(), String> {
                self.0.run(cmd, shell)
            }
            fn document_ui(&mut self, ui: &mut Ui, shell: &mut Shell) {
                self.0.document_ui(ui, shell);
            }
            fn has_document(&self) -> bool {
                self.0.has_document()
            }
            fn templates(&self) -> Vec<TemplateSpec> {
                vec![
                    TemplateSpec::new("plain", "Plain", "An empty document."),
                    TemplateSpec::new("letter", "Letter", "A letter.").with_preview(TemplatePreview::lines(["Dear reader,", "", "Yours."])),
                ]
            }
        }
        let shell = Shell::new("Test App", "1.0", jcj_theme::Tokens::default())
            .with_options(crate::ShellOptions { start_window: true, ..crate::ShellOptions::default() });
        let mut h = Harness::builder()
            .with_size(vec2(1000.0, 700.0))
            .build_ui_state(|ui, s: &mut (Shell, Previewed)| s.0.ui(ui, &mut s.1), (shell, Previewed(Blank { open: false, ran: Vec::new() })));
        h.run();
        assert_eq!(h.get_by_label("Plain").rect().size(), TILE);
        assert_eq!(h.get_by_label("Letter").rect().size(), TILE_WITH_PREVIEW);
        h.get_by_label("Letter").click();
        h.run();
        assert!(h.state().1.0.ran.iter().any(|c| c.params == json!({ "template": "letter" })));
    }

    #[test]
    fn without_the_option_the_document_area_is_unchanged() {
        let mut h = harness(false);
        h.run();
        assert!(h.query_by_label("The document").is_some());
        assert!(h.query_by_label("Letter").is_none());
    }
}
