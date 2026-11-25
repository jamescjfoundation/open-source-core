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
}

impl TemplateSpec {
    /// A template with an id, a name and a description.
    pub fn new(id: impl Into<String>, name: impl Into<String>, about: impl Into<String>) -> Self {
        TemplateSpec { id: id.into(), name: name.into(), about: about.into(), group: String::new() }
    }

    /// The same template under a group heading.
    pub fn in_group(mut self, group: impl Into<String>) -> Self {
        self.group = group.into();
        self
    }
}

/// The size of a template tile.
pub const TILE: egui::Vec2 = vec2(200.0, 92.0);

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
                let (rect, resp) = ui.allocate_exact_size(TILE, Sense::click());
                let fill = if resp.hovered() { t.hover } else { t.surface_high };
                ui.painter().rect(rect, t.radius.card, fill, Stroke::new(1.0, t.border), StrokeKind::Inside);
                ui.painter().text(
                    pos2(rect.min.x + 12.0, rect.min.y + 18.0),
                    Align2::LEFT_CENTER,
                    tr(&tpl.name),
                    semibold(t.type_sizes.title),
                    t.text,
                );
                let about = ui.painter().layout(tr(&tpl.about), regular(t.type_sizes.caption), t.text_dim, rect.width() - 24.0);
                ui.painter().with_clip_rect(rect.shrink(4.0)).galley(pos2(rect.min.x + 12.0, rect.min.y + 34.0), about, t.text_dim);
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
    fn without_the_option_the_document_area_is_unchanged() {
        let mut h = harness(false);
        h.run();
        assert!(h.query_by_label("The document").is_some());
        assert!(h.query_by_label("Letter").is_none());
    }
}
