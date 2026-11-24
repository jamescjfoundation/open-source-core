// SPDX-License-Identifier: MIT OR Apache-2.0
//! The status bar: the app's own part at the left ([`crate::App::status_ui`]), then the shell's
//! message (a command's error, a failed auto-save), and at the right the theme switch.

use egui::{RichText, Ui};
use jcj_theme::{Mode, Theme, regular};
use serde_json::json;

use crate::app::App;
use crate::commands::{Commands, shell_ids};
use crate::ribbon::Place;
use crate::ribbon::widgets::{self, Control};
use crate::shell::Shell;

/// Draw the status bar along the bottom of the window.
pub(crate) fn status_bar<A: App>(shell: &mut Shell, ui: &mut Ui, app: &mut A) {
    let t = Theme::get(ui.ctx());
    let frame = egui::Frame::NONE.fill(t.surface).stroke(egui::Stroke::new(1.0, t.border)).inner_margin(egui::Margin::symmetric(10, 3));
    shell.chrome.place = Place::Status;
    let bar = egui::Panel::bottom("jcj-status").frame(frame).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            app.status_ui(ui, shell);
            if !shell.message.is_empty() {
                ui.add(egui::Label::new(RichText::new(shell.message.as_str()).color(t.danger)).selectable(false));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let cmds: &dyn Commands = &*app;
                let (text, to) = if shell.mode == Mode::Light { ("Dark", Mode::Dark) } else { ("Light", Mode::Light) };
                let c = Control::with(shell_ids::THEME, json!({"mode": to})).about("Switch theme.");
                widgets::link(ui, shell, cmds, &c, RichText::new(text).font(regular(12.0)).color(t.text_dim));
            });
        });
    });
    shell.chrome.areas.push((Place::Status, bar.response.rect));
    shell.chrome.place = Place::Other;
}
