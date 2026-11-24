// SPDX-License-Identifier: MIT OR Apache-2.0
//! The ribbon's tabs: the app supplies them ([`TabSpec`]); the shell draws the strip with the
//! File button first and, at the right, the switch that hides the ribbon's groups.

use egui::{Sense, Stroke, Ui, pos2, vec2};
use jcj_theme::{Theme, medium, regular};

use crate::commands::shell_ids;
use crate::ribbon::{mark, widgets};
use crate::shell::Shell;

/// A ribbon tab the app offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabSpec {
    /// Its id (the app draws the tab's groups for it).
    pub id: String,
    /// Its name on the strip.
    pub name: String,
    /// The key tip letter that opens it (not `F`, which is the File page's).
    pub letter: String,
    /// Contextual: shown only with some view or selection, in the accent color.
    pub contextual: bool,
}

impl TabSpec {
    /// A tab.
    pub fn new(id: impl Into<String>, name: impl Into<String>, letter: impl Into<String>) -> Self {
        TabSpec { id: id.into(), name: name.into(), letter: letter.into(), contextual: false }
    }

    /// A contextual tab.
    pub fn contextual(mut self) -> Self {
        self.contextual = true;
        self
    }
}

/// The tab strip: the File button, the tabs, and the switch that hides the groups.
pub(crate) fn strip(shell: &mut Shell, ui: &mut Ui, tabs: &[TabSpec]) {
    let t = Theme::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let label = egui::RichText::new(shell.options.file_label.as_str()).font(medium(13.0)).color(t.on_brand);
        let file = ui.add(egui::Button::new(label).fill(t.brand).corner_radius(t.radius.control));
        widgets::record(shell, Some(shell_ids::FILE_PAGE), file.rect);
        if file.on_hover_text("New, open, save, export, print, preferences").clicked() {
            shell.run(crate::commands::Cmd::new(shell_ids::FILE_PAGE));
        }
        if shell.chrome.keytips && shell.chrome.keytip_stage == 0 {
            mark(ui, "F");
        }
        for tab in tabs {
            let current = shell.chrome.tab == tab.id;
            let color = if current {
                t.accent
            } else if tab.contextual {
                t.accent.gamma_multiply(0.8)
            } else {
                t.text
            };
            let g = ui.painter().layout_no_wrap(tab.name.clone(), medium(13.0), color);
            let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 20.0, 28.0), Sense::click());
            if resp.hovered() && !current {
                ui.painter().rect_filled(r.shrink2(vec2(0.0, 3.0)), f32::from(t.radius.control), t.hover);
            }
            ui.painter().galley(pos2(r.center().x - g.size().x / 2.0, r.center().y - g.size().y / 2.0), g, color);
            if current {
                ui.painter().line_segment([pos2(r.min.x + 8.0, r.max.y - 2.0), pos2(r.max.x - 8.0, r.max.y - 2.0)], Stroke::new(2.5, t.accent));
            }
            if shell.chrome.keytips && shell.chrome.keytip_stage == 0 {
                mark(ui, &tab.letter);
            }
            if resp.clicked() {
                if shell.chrome.collapsed {
                    shell.chrome.peek = !(current && shell.chrome.peek);
                }
                shell.chrome.tab = tab.id.clone();
            }
            if resp.double_clicked() {
                shell.chrome.collapsed = !shell.chrome.collapsed;
                shell.chrome.peek = false;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (label, icon) = if shell.chrome.collapsed { ("Show ribbon", "chevronDown") } else { ("Hide ribbon", "chevronUp") };
            let g = ui.painter().layout_no_wrap(label.to_string(), regular(11.5), t.text_dim);
            let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 22.0, 22.0), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(r, f32::from(t.radius.control), t.hover);
            }
            crate::icons::paint(
                ui.painter(),
                egui::Rect::from_center_size(pos2(r.min.x + 9.0, r.center().y), vec2(12.0, 12.0)),
                icon,
                t.text_dim,
                t.accent,
            );
            ui.painter().galley(pos2(r.min.x + 18.0, r.center().y - g.size().y / 2.0), g, t.text_dim);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
            if resp.on_hover_text("Ribbon: show the groups, or the tabs only (double-click a tab too)").clicked() {
                shell.chrome.collapsed = !shell.chrome.collapsed;
                shell.chrome.peek = false;
            }
        });
    });
}
