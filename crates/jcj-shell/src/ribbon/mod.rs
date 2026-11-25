// SPDX-License-Identifier: MIT OR Apache-2.0
//! The window's chrome above the document: a quick-access row (the app's mark, save, undo,
//! redo, the document's name and state, the side panels' switches and a command search box), the
//! ribbon's tab strip, and the current tab's groups, which the app draws ([`crate::App`]) with
//! the controls in [`widgets`]. Key tips: F10 (or Alt+Space) shows a letter on every tab, then on
//! every control of the tab picked; typing the letters runs the control.

use std::collections::HashMap;

use egui::{Align2, Event, Key, Rect, Sense, Stroke, Ui, pos2, vec2};
use jcj_i18n::{tl, tr, tr_with};
use jcj_theme::{Theme, regular, semibold};

use crate::app::App;
use crate::backstage::Page;
use crate::commands::{Cmd, Commands, ids, shell_ids};
use crate::shell::Shell;
use crate::workspace::Side;

pub mod tabs;
pub mod widgets;

pub use tabs::TabSpec;

/// Where a control was drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Place {
    /// Anywhere else (the document, a dialog).
    #[default]
    Other,
    /// The quick-access row.
    Quick,
    /// The tab strip and the current tab's groups.
    Ribbon,
    /// The File page.
    FilePage,
    /// The status bar.
    Status,
    /// A side panel.
    Panel,
}

/// A control drawn this frame: where, the command it runs (none for a drop-down, whose items
/// say theirs), and where on the screen. Tests read these to check that every control runs a
/// command the app has.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawn {
    /// The part of the window.
    pub place: Place,
    /// The command it runs.
    pub id: Option<String>,
    /// Where it is.
    pub rect: Rect,
}

/// What typing a control's key tip does.
#[derive(Clone, Debug, PartialEq)]
pub enum Keytip {
    /// Run the control's command with its parameters.
    Run(Cmd),
    /// Open a drop-down (a menu, or a group folded to fit), whose items then show letters.
    Open(egui::Id),
}

/// The chrome's state.
#[derive(Debug, Default)]
pub struct Chrome {
    /// The current tab's id.
    pub tab: String,
    /// The ribbon shows its tabs only (a click opens a tab's groups for one use).
    pub collapsed: bool,
    /// The ribbon's groups show for one use while collapsed.
    pub peek: bool,
    /// The File page, in place of the document.
    pub file_page: Option<Page>,
    /// Key tips are showing.
    pub keytips: bool,
    /// Key tips show the tabs' letters (0) or the controls' (1).
    pub keytip_stage: u8,
    /// The letters typed so far.
    pub keytip_typed: String,
    /// The controls that showed key tips in the last frame, with their letters.
    pub keytip_targets: Vec<(String, Keytip)>,
    /// Each tab's controls in the order they were first seen: a control's letters come from its
    /// place here, so they stay the same for the run however the tab's groups fold.
    keytip_order: HashMap<String, Vec<String>>,
    /// The first-run tour's step, while it shows.
    pub tour: Option<usize>,
    /// The tour was finished or skipped this run.
    pub tour_done: bool,
    /// The ribbon's groups are being drawn (only their controls take key tips).
    pub in_ribbon: bool,
    /// The part of the window being drawn.
    pub place: Place,
    /// The controls drawn this frame.
    pub drawn: Vec<Drawn>,
    /// Where the quick-access row, the ribbon's groups and the status bar were drawn this frame.
    pub areas: Vec<(Place, Rect)>,
    /// Draw drop-down menus open and no group folded (for checks that see every control).
    pub unfold_menus: bool,
}

/// Letters for the `n`th control of a tab. The first 19 are single letters; after them come "Z"
/// and the letters again ("ZA", "ZB", … "ZZA"). No control's letters begin another's, so a
/// letter typed is either a whole tip or the start of longer ones, never both, and a control's
/// letters do not depend on how many controls the tab has.
pub fn letters(n: usize) -> String {
    const L: &[u8] = b"ABCDEFGHJKLMNQSTUXY";
    let mut out = String::new();
    let mut n = n;
    while n >= L.len() {
        out.push('Z');
        n -= L.len();
    }
    out.push(char::from(L.get(n).copied().unwrap_or(b'A')));
    out
}

impl Chrome {
    /// The place of control `key` in the current tab's order. A control seen for the first time
    /// goes after the ones known, so no control's letters ever change.
    pub(crate) fn keytip_place(&mut self, key: &str) -> usize {
        let order = self.keytip_order.entry(self.tab.clone()).or_default();
        match order.iter().position(|k| k == key) {
            Some(i) => i,
            None => {
                order.push(key.to_string());
                order.len() - 1
            }
        }
    }

    pub(crate) fn end_keytips(&mut self) {
        self.keytips = false;
        self.keytip_stage = 0;
        self.keytip_typed.clear();
    }

    /// Start key tips, or end them.
    pub fn toggle_keytips(&mut self) {
        self.keytips = !self.keytips;
        self.keytip_stage = 0;
        self.keytip_typed.clear();
    }
}

/// Key tips: F10 or Alt+Space shows the tab letters; a letter opens that tab and shows its
/// controls' letters; typing those runs the control; Escape ends.
pub(crate) fn handle_keytips(shell: &mut Shell, ui: &mut Ui, tabs: &[TabSpec]) {
    let events = ui.input(|i| i.events.clone());
    let start = ui.input(|i| {
        i.events.iter().any(|e| matches!(e, Event::Key { key: Key::F10, pressed: true, .. }))
            || (i.modifiers.alt
                && !i.modifiers.command
                && !i.modifiers.shift
                && i.events.iter().any(|e| matches!(e, Event::Key { key: Key::Space, pressed: true, .. })))
    });
    if start {
        shell.chrome.toggle_keytips();
        ui.input_mut(|i| i.events.clear());
        return;
    }
    if !shell.chrome.keytips {
        return;
    }
    let mut consumed = false;
    for e in events {
        match e {
            Event::Key { key: Key::Escape, pressed: true, .. } => {
                shell.chrome.end_keytips();
                egui::Popup::close_all(ui.ctx());
                consumed = true;
            }
            Event::Text(t) => {
                consumed = true;
                let up = t.to_uppercase();
                if shell.chrome.keytip_stage == 0 {
                    if up == "F" {
                        shell.chrome.file_page = Some(Page::New);
                        shell.chrome.end_keytips();
                    } else if let Some(tab) = tabs.iter().find(|t| t.letter.eq_ignore_ascii_case(&up)) {
                        shell.chrome.tab = tab.id.clone();
                        shell.chrome.keytip_stage = 1;
                    }
                } else {
                    shell.chrome.keytip_typed.push_str(&up);
                    let typed = shell.chrome.keytip_typed.clone();
                    let target = shell.chrome.keytip_targets.iter().find(|(l, _)| *l == typed).map(|(_, t)| t.clone());
                    match target {
                        // A drop-down opens, and its items take letters too.
                        Some(Keytip::Open(popup)) => {
                            egui::Popup::open_id(ui.ctx(), popup);
                            shell.chrome.keytip_typed.clear();
                        }
                        Some(Keytip::Run(c)) => {
                            shell.chrome.end_keytips();
                            egui::Popup::close_all(ui.ctx());
                            shell.run(c);
                        }
                        None => {}
                    }
                    let typed = shell.chrome.keytip_typed.clone();
                    if !shell.chrome.keytip_targets.iter().any(|(l, _)| l.starts_with(&typed)) {
                        shell.chrome.keytip_typed.clear();
                    }
                }
            }
            _ => {}
        }
    }
    if consumed {
        ui.input_mut(|i| i.events.retain(|e| !matches!(e, Event::Text(_) | Event::Key { .. })));
    }
}

/// The quick-access row, the tab strip and the ribbon's groups.
pub(crate) fn show<A: App>(shell: &mut Shell, ui: &mut Ui, app: &mut A, tabs: &[TabSpec]) {
    let t = Theme::get(ui.ctx());
    shell.chrome.keytip_targets.clear();
    // A contextual tab goes when its view does.
    if !tabs.iter().any(|x| x.id == shell.chrome.tab) {
        shell.chrome.tab = tabs.first().map(|x| x.id.clone()).unwrap_or_default();
    }
    if shell.options.quick_row {
        shell.chrome.place = Place::Quick;
        let quick = egui::Panel::top("jcj-quick")
            .frame(egui::Frame::NONE.fill(t.surface_high).inner_margin(egui::Margin::symmetric(8, 4)))
            .show(ui, |ui| quick_row(shell, ui, &*app));
        shell.chrome.areas.push((Place::Quick, quick.response.rect));
    }
    if !shell.options.ribbon {
        shell.chrome.place = Place::Other;
        return;
    }
    shell.chrome.place = Place::Ribbon;
    let show_groups = (!shell.chrome.collapsed || shell.chrome.peek) && !tabs.is_empty();
    egui::Panel::top("jcj-ribbon")
        .frame(egui::Frame::NONE.fill(t.surface).stroke(Stroke::new(1.0, t.border)).inner_margin(egui::Margin {
            left: 8,
            right: 8,
            top: 0,
            bottom: if show_groups { 4 } else { 0 },
        }))
        .show(ui, |ui| {
            tabs::strip(shell, ui, tabs);
            if show_groups {
                ui.add_space(2.0);
                let avail = ui.available_width();
                let tab = shell.chrome.tab.clone();
                crate::ribbon_fit::begin(ui.ctx(), &tab, avail);
                let groups = egui::ScrollArea::horizontal()
                    .id_salt("jcj-ribbon-scroll")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            shell.chrome.in_ribbon = true;
                            app.ribbon_ui(&tab, ui, shell);
                            shell.chrome.in_ribbon = false;
                        });
                    });
                shell.chrome.areas.push((Place::Ribbon, groups.inner_rect));
                crate::ribbon_fit::end(ui.ctx());
            }
        });
    shell.chrome.place = Place::Other;
}

fn quick_row<A: App>(shell: &mut Shell, ui: &mut Ui, app: &A) {
    let t = Theme::get(ui.ctx());
    let cmds: &dyn Commands = app;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        // The app's mark, small, where the window's corner is.
        let w = app.mark_width(14.0);
        if w > 0.0 {
            let (r, _) = ui.allocate_exact_size(vec2(w + 10.0, 24.0), Sense::hover());
            app.paint_mark(ui.painter(), Rect::from_min_size(pos2(r.min.x + 2.0, r.center().y - 7.0), vec2(w, 14.0)), t.text);
        }
        for (id, icon) in [(ids::SAVE, "save"), (ids::UNDO, "undo"), (ids::REDO, "redo")] {
            if cmds.command(id).is_some() {
                widgets::small(ui, shell, cmds, &widgets::Control::new(id).icon(icon));
            }
        }
        ui.add_space(10.0);
        let state = if app.is_dirty() {
            tr("Not saved")
        } else if app.has_file() {
            tr("Saved")
        } else {
            tr("Not saved yet")
        };
        ui.add(egui::Label::new(egui::RichText::new(app.document_name()).font(semibold(t.type_sizes.body)).color(t.text)).selectable(false));
        ui.add(egui::Label::new(egui::RichText::new(format!("— {state}")).font(regular(t.type_sizes.body)).color(t.text_dim)).selectable(false));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Show and hide the sides that have panels.
            let panels = app.panels();
            for side in [Side::Right, Side::Bottom, Side::Left] {
                if shell.layout.panels_on(&panels, side).is_empty() {
                    continue;
                }
                let open = shell.layout.open_on(side).is_some();
                let (label, icon) = match side {
                    Side::Left => (tl("Left panel"), "chevronLeft"),
                    Side::Right => (tl("Right panel"), "chevronRight"),
                    Side::Bottom => (tl("Bottom panel"), "chevronDown"),
                };
                let c = widgets::Control::with(shell_ids::SIDE, serde_json::json!({"side": side.key()}))
                    .icon(icon)
                    .label(label)
                    .on(open)
                    .about(tl("Show or hide the panel on this side."));
                widgets::small(ui, shell, cmds, &c);
            }
            // The command search box: the palette.
            let key = widgets::key_for(shell, cmds, shell_ids::PALETTE);
            let (r, resp) = ui.allocate_exact_size(vec2(300.0, 24.0), Sense::click());
            let radius = f32::from(t.radius.control) + 1.0;
            ui.painter().rect_filled(r, radius, t.input);
            ui.painter().rect_stroke(r, radius, Stroke::new(1.0, if resp.hovered() { t.accent } else { t.border }), egui::StrokeKind::Inside);
            crate::icons::paint(
                ui.painter(),
                Rect::from_center_size(pos2(r.min.x + 13.0, r.center().y), vec2(14.0, 14.0)),
                "find",
                t.text_dim,
                t.accent,
            );
            ui.painter().text(pos2(r.min.x + 26.0, r.center().y), Align2::LEFT_CENTER, tr("Search commands"), regular(12.0), t.text_dim);
            ui.painter().text(pos2(r.max.x - 8.0, r.center().y), Align2::RIGHT_CENTER, key, regular(11.0), t.text_dim);
            widgets::record(shell, Some(shell_ids::PALETTE), r);
            if resp.on_hover_text(widgets::tip(shell, cmds, shell_ids::PALETTE, tl("Find any command by name."))).clicked() {
                shell.run(Cmd::new(shell_ids::PALETTE));
            }
        });
    });
}

/// A key tip letter beside the control just drawn.
pub(crate) fn mark(ui: &mut Ui, letter: &str) {
    let t = Theme::get(ui.ctx());
    let r = ui.min_rect();
    let at = pos2(ui.cursor().min.x - 6.0, r.max.y - 4.0);
    let g = ui.painter().layout_no_wrap(letter.to_string(), semibold(10.0), t.surface_high);
    let bx = Rect::from_center_size(at, g.size() + vec2(6.0, 2.0));
    ui.painter().rect_filled(bx, 3.0, t.text);
    ui.painter().galley(bx.min + vec2(3.0, 1.0), g, t.surface_high);
}

/// The first-run tour's card, while it shows: the steps the app gave
/// ([`crate::ShellOptions::tour`]), one at a time.
pub(crate) fn tour(shell: &mut Shell, ctx: &egui::Context) {
    let Some(step) = shell.chrome.tour else { return };
    let Some((head, body)) = shell.options.tour.get(step).cloned() else {
        shell.chrome.tour = None;
        shell.chrome.tour_done = true;
        return;
    };
    let count = shell.options.tour.len();
    let t = Theme::get(ctx);
    let mut next = false;
    let mut skip = false;
    egui::Window::new("tour")
        .title_bar(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 170.0])
        .frame(egui::Frame::popup(&ctx.style_of(ctx.theme())).fill(t.surface_high).inner_margin(14))
        .show(ctx, |ui| {
            ui.set_max_width(380.0);
            let (at, of) = ((step + 1).to_string(), count.to_string());
            ui.label(egui::RichText::new(tr_with("{step} of {count}", &[("step", &at), ("count", &of)])).font(regular(11.0)).color(t.text_dim));
            ui.label(egui::RichText::new(tr(&head)).font(semibold(16.0)).color(t.text));
            ui.label(egui::RichText::new(tr(&body)).font(regular(12.5)).color(t.text));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                next = ui.button(if step + 1 == count { tr("Done") } else { tr("Next") }).clicked();
                skip = ui.button(tr("Skip the tour")).clicked();
            });
        });
    if skip || (next && step + 1 == count) {
        shell.chrome.tour = None;
        shell.chrome.tour_done = true;
    } else if next {
        shell.chrome.tour = Some(step + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_count_on() {
        assert_eq!(letters(0), "A");
        assert_eq!(letters(18), "Y");
        assert_eq!(letters(19), "ZA");
        assert_eq!(letters(37), "ZY");
        assert_eq!(letters(38), "ZZA");
    }

    #[test]
    fn no_letters_begin_another_control_s() {
        let all: Vec<String> = (0..400).map(letters).collect();
        for (i, a) in all.iter().enumerate() {
            for (j, b) in all.iter().enumerate() {
                assert!(i == j || !b.starts_with(a.as_str()), "{a} (control {i}) begins {b} (control {j})");
            }
        }
    }

    #[test]
    fn controls_keep_their_places_as_others_show_and_hide() {
        let mut c = Chrome::default();
        assert_eq!([c.keytip_place("a"), c.keytip_place("c")], [0, 1]);
        // A frame where "b" shows between them (a group unfolded): it goes last.
        assert_eq!([c.keytip_place("a"), c.keytip_place("b"), c.keytip_place("c")], [0, 2, 1]);
        // A frame where "b" is hidden again.
        assert_eq!([c.keytip_place("a"), c.keytip_place("c")], [0, 1]);
        // Each tab has its own order.
        c.tab = "view".into();
        assert_eq!(c.keytip_place("c"), 0);
    }
}
