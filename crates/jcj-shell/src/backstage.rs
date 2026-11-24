// SPDX-License-Identifier: MIT OR Apache-2.0
//! The File page: a full-window page in place of the document, with a column of entries in the
//! app's brand color (New, Open, Save, Export, Print, the app's own pages, Preferences, Keys,
//! About) and the picked entry's page beside it. The shell draws what every app has (recent
//! files, Save and Save As, the preferences, keys, the About page with its credit line); the app
//! adds its own to each page ([`crate::App::file_page_ui`]). Every button runs a command.

use egui::{Align2, Color32, RichText, Sense, Stroke, Ui, pos2, vec2};
use jcj_theme::{Theme, medium, regular, semibold};
use serde_json::json;

use crate::app::App;
use crate::commands::{Commands, ids};
use crate::ribbon::Place;
use crate::ribbon::widgets::{self, Control};
use crate::shell::Shell;

/// A page of the File page.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Page {
    /// New documents (the app adds its templates).
    New,
    /// Open, and the recent documents.
    Open,
    /// Save and Save As.
    Save,
    /// Export (the app's formats).
    Export,
    /// Print (the app's print choices).
    Print,
    /// The preferences.
    Preferences,
    /// Every command's keys, and the user's own.
    Keys,
    /// The app's name, version, credit line and updates.
    About,
    /// One of the app's own pages, by id.
    App(String),
}

impl Page {
    /// The shell's own pages, in order.
    pub const BUILT_IN: [Page; 8] = [Page::New, Page::Open, Page::Save, Page::Export, Page::Print, Page::Preferences, Page::Keys, Page::About];

    /// The page's name in commands (`{"page": "open"}`); an app page's is its id.
    pub fn key(&self) -> &str {
        match self {
            Page::New => "new",
            Page::Open => "open",
            Page::Save => "save",
            Page::Export => "export",
            Page::Print => "print",
            Page::Preferences => "preferences",
            Page::Keys => "keys",
            Page::About => "about",
            Page::App(id) => id,
        }
    }

    /// The page named `k` (anything else is an app page).
    pub fn from_key(k: &str) -> Page {
        Page::BUILT_IN.into_iter().find(|p| p.key() == k).unwrap_or_else(|| Page::App(k.to_string()))
    }

    fn label_and_icon(&self) -> (&'static str, &'static str) {
        match self {
            Page::New => ("New", "plus"),
            Page::Open => ("Open", "open"),
            Page::Save => ("Save", "save"),
            Page::Export => ("Export", "export"),
            Page::Print => ("Print", "print"),
            Page::Preferences => ("Preferences", "options"),
            Page::Keys => ("Keys", "macros"),
            Page::About => ("About", "help"),
            Page::App(_) => ("", ""),
        }
    }
}

/// One of the app's own File pages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageSpec {
    /// Its id (`Page::App(id)`).
    pub id: String,
    /// Its entry's label.
    pub label: String,
    /// Its entry's icon ([`crate::icons`]).
    pub icon: String,
}

impl PageSpec {
    /// A page.
    pub fn new(id: impl Into<String>, label: impl Into<String>, icon: impl Into<String>) -> Self {
        PageSpec { id: id.into(), label: label.into(), icon: icon.into() }
    }
}

/// The entries in the column: the shell's pages the options keep, with the app's before
/// Preferences.
pub fn entries(shell: &Shell, app_pages: &[PageSpec]) -> Vec<(Page, String, String)> {
    let mut out = Vec::new();
    let mut app_done = false;
    for p in &shell.options.file_pages {
        if matches!(p, Page::Preferences | Page::Keys | Page::About) && !app_done {
            out.extend(app_pages.iter().map(|a| (Page::App(a.id.clone()), a.label.clone(), a.icon.clone())));
            app_done = true;
        }
        let (label, icon) = p.label_and_icon();
        if !label.is_empty() {
            out.push((p.clone(), label.to_string(), icon.to_string()));
        }
    }
    if !app_done {
        out.extend(app_pages.iter().map(|a| (Page::App(a.id.clone()), a.label.clone(), a.icon.clone())));
    }
    out
}

/// Draw the File page in `ui` (the central area).
pub(crate) fn show<A: App>(shell: &mut Shell, ui: &mut Ui, app: &mut A) {
    let Some(page) = shell.chrome.file_page.clone() else { return };
    shell.chrome.place = Place::FilePage;
    draw(shell, ui, app, page);
    shell.chrome.place = Place::Other;
}

fn draw<A: App>(shell: &mut Shell, ui: &mut Ui, app: &mut A, page: Page) {
    let t = Theme::get(ui.ctx());
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        shell.chrome.file_page = None;
        return;
    }
    let full = ui.max_rect();
    // The column of entries, in the brand color, with the app's mark at its foot.
    let col = egui::Rect::from_min_size(full.min, vec2(200.0, full.height()));
    ui.painter().rect_filled(col, 0.0, t.brand);
    let mut bottom = col.min.y;
    let mut picked = None;
    let mut back = false;
    let list = entries(shell, &app.file_pages());
    ui.scope_builder(egui::UiBuilder::new().max_rect(col.shrink2(vec2(10.0, 14.0))), |ui| {
        let ink = t.on_brand;
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r, 6.0, Color32::from_white_alpha(30));
        }
        ui.painter().text(pos2(r.min.x + 10.0, r.center().y), Align2::LEFT_CENTER, "←  Back", medium(t.type_sizes.body), ink);
        back = resp.on_hover_text("Escape").clicked();
        ui.add_space(10.0);
        for (p, label, icon) in &list {
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
            if *p == page {
                ui.painter().rect_filled(r, 6.0, Color32::from_white_alpha(30));
                // The current page is marked with the highlight color.
                ui.painter().rect_filled(egui::Rect::from_min_size(pos2(r.min.x, r.min.y + 7.0), vec2(3.0, r.height() - 14.0)), 0.0, t.highlight);
            } else if resp.hovered() {
                ui.painter().rect_filled(r, 6.0, Color32::from_white_alpha(25));
            }
            crate::icons::paint(ui.painter(), egui::Rect::from_center_size(pos2(r.min.x + 18.0, r.center().y), vec2(18.0, 18.0)), icon, ink, ink);
            ui.painter().text(pos2(r.min.x + 36.0, r.center().y), Align2::LEFT_CENTER, label, medium(13.5), ink);
            widgets::record(shell, Some(crate::commands::shell_ids::FILE_PAGE), r);
            if resp.clicked() {
                picked = Some(p.clone());
            }
            bottom = r.max.y;
        }
    });
    // The mark, only where it clears the last entry.
    let foot = col.max.y - 34.0;
    let w = app.mark_width(20.0).min(col.width() - 40.0);
    if w > 0.0 && foot > bottom + 24.0 {
        app.paint_mark(ui.painter(), egui::Rect::from_min_size(pos2(col.min.x + 20.0, foot - 4.0), vec2(w, 20.0)), t.on_brand);
    }
    if back {
        shell.chrome.file_page = None;
        return;
    }
    if let Some(p) = picked {
        shell.chrome.file_page = Some(p);
    }
    let content = egui::Rect::from_min_max(pos2(col.max.x + 32.0, full.min.y + 20.0), pos2(full.max.x - 24.0, full.max.y - 10.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(content), |ui| {
        egui::ScrollArea::vertical().id_salt("jcj-file-page").auto_shrink(false).show(ui, |ui| match &page {
            Page::New => new(shell, ui, app),
            Page::Open => open(shell, ui, app),
            Page::Save => save(shell, ui, app),
            Page::Export => {
                heading(ui, "Export");
                app.file_page_ui(&page, ui, shell);
            }
            Page::Print => {
                heading(ui, "Print");
                app.file_page_ui(&page, ui, shell);
                action(ui, shell, &*app, &Control::new(ids::PRINT).label("Print").about("Send the pages to the printer."));
            }
            Page::Preferences => {
                heading(ui, "Preferences");
                crate::dialogs::prefs_body(shell, ui);
                app.preferences_ui(ui, shell);
            }
            Page::Keys => {
                heading(ui, "Keys");
                crate::dialogs::keys_body(shell, ui, &*app);
            }
            Page::About => about(shell, ui, app),
            Page::App(_) => app.file_page_ui(&page, ui, shell),
        });
    });
}

/// A page's heading.
pub fn heading(ui: &mut Ui, text: &str) {
    let t = Theme::get(ui.ctx());
    ui.label(RichText::new(text).font(semibold(t.type_sizes.heading)).color(t.text));
    ui.add_space(12.0);
}

/// A section's heading on a page.
pub fn sub(ui: &mut Ui, text: &str) {
    let t = Theme::get(ui.ctx());
    ui.add_space(10.0);
    ui.label(RichText::new(text).font(semibold(15.0)).color(t.text));
    ui.add_space(4.0);
}

/// A wide button with an icon, a title (the control's label) and a line under it (its
/// `about`). A click closes the File page and queues the command.
pub fn action(ui: &mut Ui, shell: &mut Shell, cmds: &dyn Commands, c: &Control) -> bool {
    let t = Theme::get(ui.ctx());
    let ok = widgets::enabled(cmds, &c.cmd.id);
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width().min(560.0), 50.0), if ok { Sense::click() } else { Sense::hover() });
    let radius = f32::from(t.radius.card) + 2.0;
    ui.painter().rect_filled(r, radius, if resp.hovered() && ok { t.hover } else { t.surface_high });
    ui.painter().rect_stroke(r, radius, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
    let ink = if ok { t.text } else { t.text_dim };
    let icon = if c.icon.is_empty() { crate::commands::info(cmds, &c.cmd.id).map(|i| i.icon).unwrap_or_default() } else { c.icon.clone() };
    crate::icons::paint(ui.painter(), egui::Rect::from_center_size(pos2(r.min.x + 26.0, r.center().y), vec2(24.0, 24.0)), &icon, ink, t.accent);
    let title =
        if c.label.is_empty() { crate::commands::info(cmds, &c.cmd.id).map_or_else(|| c.cmd.id.clone(), |i| i.label) } else { c.label.clone() };
    ui.painter().text(pos2(r.min.x + 50.0, r.min.y + 17.0), Align2::LEFT_CENTER, title, medium(13.5), ink);
    ui.painter().text(pos2(r.min.x + 50.0, r.min.y + 35.0), Align2::LEFT_CENTER, &c.about, regular(t.type_sizes.caption), t.text_dim);
    ui.add_space(6.0);
    widgets::record(shell, Some(&c.cmd.id), r);
    let resp = resp.on_hover_text(widgets::tip(shell, cmds, &c.cmd.id, ""));
    if resp.clicked() && ok {
        shell.chrome.file_page = None;
        shell.run(c.cmd.clone());
        return true;
    }
    false
}

fn file_name(path: &str) -> String {
    std::path::Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or(path).to_string()
}

fn new<A: App>(shell: &mut Shell, ui: &mut Ui, app: &mut A) {
    heading(ui, "New");
    action(ui, shell, &*app, &Control::new(ids::NEW).icon("file").label("New document").about("Start an empty document."));
    if !shell.recent.is_empty() {
        sub(ui, "Recent");
        let cmds: &dyn Commands = &*app;
        ui.horizontal_wrapped(|ui| {
            for r in shell.recent.clone().into_iter().take(6) {
                let link = ui.link(file_name(&r));
                widgets::record(shell, Some(ids::OPEN_PATH), link.rect);
                if link.on_hover_text(&r).clicked() && widgets::enabled(cmds, ids::OPEN_PATH) {
                    shell.chrome.file_page = None;
                    shell.run(crate::commands::Cmd::with(ids::OPEN_PATH, json!({"path": r})));
                }
            }
        });
    }
    app.file_page_ui(&Page::New, ui, shell);
}

fn open<A: App>(shell: &mut Shell, ui: &mut Ui, app: &mut A) {
    let t = Theme::get(ui.ctx());
    heading(ui, "Open");
    action(ui, shell, &*app, &Control::new(ids::OPEN).label("Open…").about("A document from a file."));
    sub(ui, "Recent");
    if shell.recent.is_empty() {
        ui.label(RichText::new("Documents you open or save show here.").color(t.text_dim));
    }
    let mut pick = None;
    for r in shell.recent.clone() {
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width().min(560.0), 38.0), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(rect, 6.0, t.hover);
        }
        ui.painter().text(pos2(rect.min.x + 10.0, rect.min.y + 12.0), Align2::LEFT_CENTER, file_name(&r), medium(13.0), t.text);
        ui.painter().text(pos2(rect.min.x + 10.0, rect.min.y + 28.0), Align2::LEFT_CENTER, &r, regular(10.5), t.text_dim);
        widgets::record(shell, Some(ids::OPEN_PATH), rect);
        if resp.clicked() {
            pick = Some(r);
        }
    }
    if !shell.dialogs.recovery.is_empty() {
        sub(ui, "Unsaved work");
        if ui.button("Recover Unsaved Documents…").clicked() {
            shell.dialogs.show_recovery = true;
        }
    }
    app.file_page_ui(&Page::Open, ui, shell);
    if let Some(path) = pick {
        shell.chrome.file_page = None;
        shell.run(crate::commands::Cmd::with(ids::OPEN_PATH, json!({"path": path})));
    }
}

fn save<A: App>(shell: &mut Shell, ui: &mut Ui, app: &mut A) {
    let t = Theme::get(ui.ctx());
    heading(ui, "Save");
    let where_ = app.document_path().unwrap_or_else(|| "Not saved to a file yet.".to_string());
    ui.label(RichText::new(where_).color(t.text_dim));
    ui.add_space(8.0);
    action(ui, shell, &*app, &Control::new(ids::SAVE).label("Save").about("Keep the changes in the document's file."));
    action(ui, shell, &*app, &Control::new(ids::SAVE_AS).icon("saveAs").label("Save As…").about("Keep the document in a new file."));
    app.file_page_ui(&Page::Save, ui, shell);
}

fn about<A: App>(shell: &mut Shell, ui: &mut Ui, app: &mut A) {
    let t = Theme::get(ui.ctx());
    let w = app.mark_width(56.0);
    if w > 0.0 {
        let (r, _) = ui.allocate_exact_size(vec2(w, 56.0), Sense::hover());
        app.paint_mark(ui.painter(), r, t.text);
        ui.add_space(16.0);
    }
    heading(ui, "About");
    let info = app.about();
    ui.label(RichText::new(format!("{} {}", shell.name, shell.version)).font(medium(15.0)).color(t.text));
    if !info.summary.is_empty() {
        ui.label(RichText::new(&info.summary).font(regular(t.type_sizes.body)).color(t.text));
    }
    if !info.credit.is_empty() {
        ui.add_space(6.0);
        ui.label(RichText::new(&info.credit).font(regular(t.type_sizes.body)).color(t.text_dim));
    }
    if let Some(site) = &info.website {
        ui.hyperlink(site);
    }
    if !info.acknowledgments.is_empty() {
        sub(ui, "Acknowledgments");
        ui.label(RichText::new(&info.acknowledgments).font(regular(t.type_sizes.body)).color(t.text));
    }
    sub(ui, "Updates");
    crate::updates::actions(shell, ui);
    ui.checkbox(&mut shell.updates.check_at_start, "Check for updates when the app starts");
    app.file_page_ui(&Page::About, ui, shell);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_have_keys_and_app_pages_go_before_preferences() {
        for p in Page::BUILT_IN {
            assert_eq!(Page::from_key(p.key()), p);
        }
        assert_eq!(Page::from_key("templates"), Page::App("templates".into()));
        let shell = Shell::new("Sample", "1.0", jcj_theme::Tokens::default());
        let list = entries(&shell, &[PageSpec::new("compare", "Compare", "compare")]);
        let keys: Vec<&str> = list.iter().map(|(p, _, _)| p.key()).collect();
        assert_eq!(keys, ["new", "open", "save", "export", "print", "compare", "preferences", "keys", "about"]);
    }
}
