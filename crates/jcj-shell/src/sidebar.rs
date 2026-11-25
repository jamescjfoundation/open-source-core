// SPDX-License-Identifier: MIT OR Apache-2.0
//! The side panels: an icon rail at the left and right edges of the window for the panels docked
//! there, and the open panel on each side (left, right, bottom) with a title bar to pin, move or
//! close it. A click on a rail icon opens its panel; a second click closes it. A pinned panel
//! takes its room beside the document; an unpinned one floats over it.

use egui::{Area, Id, Order, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use jcj_i18n::tr;
use jcj_theme::{Theme, semibold};

use crate::app::App;
use crate::ribbon::Place;
use crate::shell::Shell;
use crate::workspace::{PanelSpec, Side};

/// The rail's width.
pub const RAIL_W: f32 = 40.0;

/// What the user did in a panel's title bar or a rail this frame.
enum Act {
    Toggle(String),
    Hide(String),
    Pin(String, bool),
    Move(String, Side),
}

fn rail(shell: &Shell, ui: &mut Ui, items: &[&PanelSpec], side: Side, acts: &mut Vec<Act>) {
    let t = Theme::get(ui.ctx());
    ui.add_space(6.0);
    for p in items {
        let open = shell.layout.open_on(side) == Some(p.id.as_str());
        let (r, resp) = ui.allocate_exact_size(vec2(RAIL_W - 6.0, 32.0), Sense::click());
        if open {
            ui.painter().rect_filled(r, 6.0, t.accent_soft);
            let x = if side == Side::Left { r.min.x - 2.0 } else { r.max.x + 2.0 };
            ui.painter().line_segment([pos2(x, r.min.y + 6.0), pos2(x, r.max.y - 6.0)], Stroke::new(2.5, t.accent));
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 6.0, t.hover);
        }
        crate::icons::paint(
            ui.painter(),
            Rect::from_center_size(r.center(), vec2(18.0, 18.0)),
            &p.icon,
            if open { t.accent } else { t.text },
            t.accent,
        );
        let title = tr(&p.title);
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, open, &title));
        if resp.on_hover_text(title.as_str()).clicked() {
            acts.push(Act::Toggle(p.id.clone()));
        }
        ui.add_space(2.0);
    }
}

/// A panel's title bar: its title, and pin, move and close.
fn title_bar(ui: &mut Ui, spec: &PanelSpec, pinned: bool, side: Side, acts: &mut Vec<Act>) {
    let t = Theme::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr(&spec.title)).font(semibold(t.type_sizes.title)).color(t.text));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add(egui::Button::new("×").frame(false)).on_hover_text(tr("Close")).clicked() {
                acts.push(Act::Hide(spec.id.clone()));
            }
            let (r, resp) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(r, 4.0, t.hover);
            }
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, pinned, tr("Pin")));
            crate::icons::paint(ui.painter(), r.shrink(2.0), "pin", if pinned { t.accent } else { t.text_dim }, t.accent);
            let tip = if pinned { tr("Unpin: float over the document") } else { tr("Pin beside the document") };
            if resp.on_hover_text(tip).clicked() {
                acts.push(Act::Pin(spec.id.clone(), !pinned));
            }
            ui.menu_button(tr("Move"), |ui| {
                for to in Side::ALL {
                    let label = match to {
                        Side::Left => tr("Move to the left"),
                        Side::Right => tr("Move to the right"),
                        Side::Bottom => tr("Move to the bottom"),
                    };
                    if to != side && ui.button(label).clicked() {
                        acts.push(Act::Move(spec.id.clone(), to));
                        ui.close();
                    }
                }
            });
        });
    });
    ui.separator();
}

/// The rails and the open panels; call before the central area.
pub(crate) fn show<A: App>(shell: &mut Shell, ui: &mut Ui, app: &mut A) {
    let panels = app.panels();
    if panels.is_empty() {
        return;
    }
    let t = Theme::get(ui.ctx());
    let mut acts = Vec::new();
    let frame = egui::Frame::NONE.fill(t.surface).stroke(Stroke::new(1.0, t.border));
    for side in [Side::Left, Side::Right] {
        let items = shell.layout.panels_on(&panels, side);
        if items.is_empty() {
            continue;
        }
        let panel = if side == Side::Left { egui::Panel::left("jcj-rail-left") } else { egui::Panel::right("jcj-rail-right") };
        let resp = panel.exact_size(RAIL_W).resizable(false).frame(frame).show(ui, |ui| rail(shell, ui, &items, side, &mut acts));
        shell.rails.push(resp.response.rect);
    }
    let margin = shell.tokens().spacing.panel_margin.round().clamp(0.0, 64.0) as i8;
    let body = egui::Frame::NONE.fill(t.surface).stroke(Stroke::new(1.0, t.border)).inner_margin(egui::Margin::symmetric(margin + 2, margin - 2));
    let free = ui.available_rect_before_wrap();
    for side in [Side::Left, Side::Right, Side::Bottom] {
        let Some(id) = shell.layout.open_on(side).map(str::to_string) else { continue };
        let Some(spec) = panels.iter().find(|p| p.id == id).cloned() else { continue };
        let pinned = shell.layout.is_pinned(&id);
        let size = *shell.layout.size.get(side);
        let (lo, hi) = side.size_range();
        let prev = shell.chrome.place;
        shell.chrome.place = Place::Panel;
        let mut contents = |shell: &mut Shell, ui: &mut Ui, acts: &mut Vec<Act>| {
            title_bar(ui, &spec, pinned, side, acts);
            egui::ScrollArea::vertical().id_salt(("jcj-panel", id.as_str())).auto_shrink(false).show(ui, |ui| app.panel_ui(&id, ui, shell));
        };
        if pinned {
            let panel = match side {
                Side::Left => egui::Panel::left(("jcj-panel-left", id.as_str())),
                Side::Right => egui::Panel::right(("jcj-panel-right", id.as_str())),
                Side::Bottom => egui::Panel::bottom(("jcj-panel-bottom", id.as_str())),
            };
            let resp = panel.resizable(true).default_size(size).size_range(lo..=hi).frame(body).show(ui, |ui| contents(shell, ui, &mut acts));
            let r = resp.response.rect;
            shell.layout.set_size(side, if side == Side::Bottom { r.height() } else { r.width() });
        } else {
            // Floating over the document, against its side.
            let rect = match side {
                Side::Left => Rect::from_min_size(free.min, vec2(size, free.height())),
                Side::Right => Rect::from_min_size(pos2(free.max.x - size, free.min.y), vec2(size, free.height())),
                Side::Bottom => Rect::from_min_size(pos2(free.min.x, free.max.y - size), vec2(free.width(), size)),
            };
            Area::new(Id::unique(("jcj-float", id.as_str()))).order(Order::Foreground).fixed_pos(rect.min).show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).fill(t.surface).corner_radius(0).show(ui, |ui| {
                    ui.set_min_size(rect.size() - vec2(16.0, 16.0));
                    ui.set_max_size(rect.size() - vec2(16.0, 16.0));
                    contents(shell, ui, &mut acts);
                });
            });
            shell.floating.push(rect);
        }
        shell.chrome.place = prev;
    }
    for act in acts {
        match act {
            Act::Toggle(id) => {
                shell.layout.toggle(&panels, &id);
            }
            Act::Hide(id) => shell.layout.hide(&id),
            Act::Pin(id, on) => shell.layout.set_pinned(&id, on),
            Act::Move(id, side) => {
                shell.layout.move_to(&panels, &id, side);
            }
        }
    }
}
