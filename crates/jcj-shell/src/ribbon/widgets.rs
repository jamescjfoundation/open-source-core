// SPDX-License-Identifier: MIT OR Apache-2.0
//! Ribbon controls: large and small buttons, toggles, galleries, drop-downs and groups. Every
//! control runs a command by id ([`Control`]); it is greyed out when the app says the command
//! cannot run (or does not have it), and its tooltip shows the command's name, key and a line on
//! what it does. Groups shrink to fit the window through [`crate::ribbon_fit`]. A click queues
//! the command; the shell runs it once the frame is drawn.

use egui::{Align2, Color32, Rect, Response, Sense, Stroke, Ui, pos2, vec2};
use jcj_theme::{Theme, medium, regular};
use serde_json::Value;

use crate::commands::{Cmd, Commands, info};
use crate::ribbon::{Drawn, Keytip, letters};
use crate::ribbon_fit;
use crate::shell::Shell;

/// Height of a group's controls, and of its title row under them.
pub const CONTENT_H: f32 = 72.0;
/// Height of a group's title row.
pub const TITLE_H: f32 = 16.0;
/// A large button's width.
pub const BIG_W: f32 = 58.0;
/// A small button's height.
pub const SMALL_H: f32 = 21.0;
/// Label size of small buttons.
const BODY_SMALL: f32 = 12.0;

/// A control: the command it runs, and how it shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Control {
    /// The command, with the parameters this control runs it with.
    pub cmd: Cmd,
    /// Its icon ([`crate::icons`]); empty: the command's own icon.
    pub icon: String,
    /// Its label; empty: the command's name.
    pub label: String,
    /// A line on what it does, for its tooltip.
    pub about: String,
    /// Drawn pressed (a toggle that is on, the current choice).
    pub on: bool,
    /// A gallery tile's second line (a key, a hint).
    pub second: String,
    /// A gallery tile's width; 0 for the usual.
    pub width: f32,
}

impl Control {
    /// A control for command `id` with no parameters.
    pub fn new(id: impl Into<String>) -> Self {
        Control::of(Cmd::new(id))
    }

    /// A control for command `id` with `params`.
    pub fn with(id: impl Into<String>, params: Value) -> Self {
        Control::of(Cmd::with(id, params))
    }

    /// A control for `cmd`.
    pub fn of(cmd: Cmd) -> Self {
        Control { cmd, icon: String::new(), label: String::new(), about: String::new(), on: false, second: String::new(), width: 0.0 }
    }

    /// With this icon.
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }

    /// With this label.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// With this tooltip line.
    pub fn about(mut self, about: impl Into<String>) -> Self {
        self.about = about.into();
        self
    }

    /// Drawn pressed when `on`.
    pub fn on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }

    /// A tile's second line.
    pub fn second(mut self, second: impl Into<String>) -> Self {
        self.second = second.into();
        self
    }

    /// A tile's width.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    fn label_or_name(&self, cmds: &dyn Commands) -> String {
        if self.label.is_empty() { info(cmds, &self.cmd.id).map_or_else(|| self.cmd.id.clone(), |c| c.label) } else { self.label.clone() }
    }

    fn icon_name(&self, cmds: &dyn Commands) -> String {
        if self.icon.is_empty() { info(cmds, &self.cmd.id).map(|c| c.icon).unwrap_or_default() } else { self.icon.clone() }
    }
}

/// The key shown for command `id`: the user's own first, else the command's.
pub fn key_for(shell: &Shell, cmds: &dyn Commands, id: &str) -> String {
    let key = shell.keys.user().iter().find(|(_, c)| c.as_str() == id).map(|(k, _)| k.clone());
    let key = key.unwrap_or_else(|| info(cmds, id).map(|c| c.first_shortcut().to_string()).unwrap_or_default());
    if key.is_empty() { key } else { crate::keys::display(&key, shell.mac) }
}

/// The tooltip for command `id`: its name and key, then `about`, then why it cannot run now.
pub fn tip(shell: &Shell, cmds: &dyn Commands, id: &str, about: &str) -> String {
    let name = info(cmds, id).map_or_else(|| id.to_string(), |c| c.label);
    let key = key_for(shell, cmds, id);
    let mut out = if key.is_empty() { name } else { format!("{name}   {key}") };
    if !about.is_empty() {
        out.push('\n');
        out.push_str(about);
    }
    if info(cmds, id).is_none() {
        out.push_str("\n(Not available)");
    } else if let Some(why) = cmds.why_disabled(id) {
        out.push_str(&format!("\n(Not now: {why})"));
    }
    out
}

/// Whether command `id` can run now.
pub fn enabled(cmds: &dyn Commands, id: &str) -> bool {
    info(cmds, id).is_some() && (id.starts_with("shell.") || cmds.why_disabled(id).is_none())
}

/// Note that a control running command `id` (none: a drop-down) was drawn at `r`.
pub fn record(shell: &mut Shell, id: Option<&str>, r: Rect) {
    let place = shell.chrome.place;
    shell.chrome.drawn.push(Drawn { place, id: id.map(str::to_string), rect: r });
}

/// The name a control is known by in its tab's key tip order: its command and the parameters
/// that tell controls apart (not those that follow the window's state).
fn keytip_key(shell: &Shell, what: &Keytip, name: &str) -> String {
    match what {
        Keytip::Run(c) => {
            let mut p = c.params.clone();
            if let Some(o) = p.as_object_mut() {
                o.retain(|k, _| !shell.options.keytip_ignored_params.iter().any(|x| x == k));
            }
            format!("{}{p}", c.id)
        }
        Keytip::Open(_) => format!("menu:{name}"),
    }
}

/// When key tips show a tab's controls, the letters of the control drawn at `r` (`name` tells
/// drop-downs apart), drawn under it and kept for the keys typed next frame. Two controls that
/// run one command with the same parameters share letters.
fn note_keytip(ui: &Ui, shell: &mut Shell, r: Rect, what: Keytip, name: &str) {
    if !shell.chrome.keytips || shell.chrome.keytip_stage != 1 || !shell.chrome.in_ribbon {
        return;
    }
    let key = keytip_key(shell, &what, name);
    let letters = letters(shell.chrome.keytip_place(&key));
    let t = Theme::get(ui.ctx());
    let at = pos2(r.center().x, r.max.y - 4.0);
    let g = ui.painter().layout_no_wrap(letters.clone(), jcj_theme::semibold(10.0), t.surface_high);
    let bx = Rect::from_center_size(at, g.size() + vec2(6.0, 2.0));
    ui.painter().rect_filled(bx, 3.0, t.text);
    ui.painter().galley(bx.min + vec2(3.0, 1.0), g, t.surface_high);
    if !shell.chrome.keytip_targets.iter().any(|(l, _)| *l == letters) {
        shell.chrome.keytip_targets.push((letters, what));
    }
}

/// The background of a control: hovered, on, or plain.
fn bg(ui: &Ui, r: Rect, hovered: bool, on: bool, t: &Theme) {
    let radius = f32::from(t.radius.control);
    if on {
        ui.painter().rect_filled(r, radius, t.accent_soft);
        ui.painter().rect_stroke(r, radius, Stroke::new(1.0, t.accent.gamma_multiply(0.5)), egui::StrokeKind::Inside);
    } else if hovered {
        ui.painter().rect_filled(r, radius, t.hover);
    }
}

/// After drawing a control at `r`: record it, note its key tip, give it its tooltip and queue
/// its command when clicked. Returns whether it was clicked.
fn finish(ui: &Ui, shell: &mut Shell, cmds: &dyn Commands, c: &Control, r: Rect, resp: Response, ok: bool) -> bool {
    record(shell, Some(&c.cmd.id), r);
    note_keytip(ui, shell, r, Keytip::Run(c.cmd.clone()), "");
    let resp = resp.on_hover_text(tip(shell, cmds, &c.cmd.id, &c.about));
    if resp.clicked() && ok {
        shell.run(c.cmd.clone());
        return true;
    }
    false
}

/// A large button: icon over a label of up to two lines.
pub fn big(ui: &mut Ui, shell: &mut Shell, cmds: &dyn Commands, c: &Control) -> bool {
    let t = Theme::get(ui.ctx());
    let ok = enabled(cmds, &c.cmd.id);
    let (r, resp) = ui.allocate_exact_size(vec2(BIG_W, CONTENT_H), if ok { Sense::click() } else { Sense::hover() });
    bg(ui, r, resp.hovered() && ok, c.on, &t);
    let ink = if ok { t.text } else { t.text_dim.gamma_multiply(0.6) };
    let accent = if ok { t.accent } else { ink };
    crate::icons::paint(ui.painter(), Rect::from_center_size(pos2(r.center().x, r.min.y + 19.0), vec2(26.0, 26.0)), &c.icon_name(cmds), ink, accent);
    let mut job = egui::text::LayoutJob::simple(c.label_or_name(cmds), regular(t.type_sizes.caption), ink, BIG_W - 4.0);
    job.halign = egui::Align::Center;
    job.wrap.max_rows = 2;
    let g = ui.painter().layout_job(job);
    ui.painter().galley(pos2(r.center().x, r.min.y + 37.0), g, ink);
    finish(ui, shell, cmds, c, r, resp, ok)
}

/// A small button: icon and a label (the label goes in a compact group; the tooltip keeps it).
/// An empty label (not the command's name) draws the icon alone.
pub fn small(ui: &mut Ui, shell: &mut Shell, cmds: &dyn Commands, c: &Control) -> bool {
    let t = Theme::get(ui.ctx());
    let ok = enabled(cmds, &c.cmd.id);
    let compact = ribbon_fit::compact(ui.ctx());
    let label = c.label.as_str();
    let galley = ui.painter().layout_no_wrap(label.to_string(), regular(BODY_SMALL), t.text);
    let label_w = galley.size().x + 8.0;
    if !compact && !label.is_empty() {
        ribbon_fit::add_saving(ui.ctx(), label_w);
    }
    let w = 24.0 + if compact || label.is_empty() { 0.0 } else { label_w };
    let (r, resp) = ui.allocate_exact_size(vec2(w, SMALL_H), if ok { Sense::click() } else { Sense::hover() });
    bg(ui, r, resp.hovered() && ok, c.on, &t);
    let ink = if ok { t.text } else { t.text_dim.gamma_multiply(0.6) };
    let accent = if ok { t.accent } else { ink };
    crate::icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + 12.0, r.center().y), vec2(16.0, 16.0)), &c.icon_name(cmds), ink, accent);
    if !compact && !label.is_empty() {
        ui.painter().text(pos2(r.min.x + 24.0, r.center().y), Align2::LEFT_CENTER, label, regular(BODY_SMALL), ink);
    }
    finish(ui, shell, cmds, c, r, resp, ok)
}

/// A drop-down button in a group: `add` draws its items (with [`item`], [`check`], [`choice`]).
/// `about` is its tooltip, and tells it apart from other drop-downs for key tips.
pub fn drop(ui: &mut Ui, shell: &mut Shell, label: impl Into<egui::RichText>, about: &str, add: impl FnOnce(&mut Ui, &mut Shell)) {
    let resp = ui.add(egui::Button::new(label.into())).on_hover_text(about);
    popup_menu(ui, shell, &resp, about, add);
}

/// The menu under `resp` (a drop-down button or chip), opened by a click or its key tip.
pub fn popup_menu(ui: &mut Ui, shell: &mut Shell, resp: &Response, name: &str, add: impl FnOnce(&mut Ui, &mut Shell)) {
    record(shell, None, resp.rect);
    let popup = egui::Popup::default_response_id(resp);
    note_keytip(ui, shell, resp.rect, Keytip::Open(popup), name);
    let unfold = shell.chrome.unfold_menus;
    let menu = egui::Popup::menu(resp);
    if unfold { menu.open(true) } else { menu }.show(|ui| add(ui, shell));
}

/// A menu (of a menu bar or a panel), or a submenu.
pub fn menu(ui: &mut Ui, shell: &mut Shell, title: &str, add: impl FnOnce(&mut Ui, &mut Shell)) {
    if shell.chrome.unfold_menus {
        let resp = ui.button(title);
        egui::Popup::menu(&resp).open(true).show(|ui| add(ui, shell));
    } else {
        ui.menu_button(title, |ui| add(ui, shell));
    }
}

/// A plain button that runs the control's command.
pub fn button(ui: &mut Ui, shell: &mut Shell, cmds: &dyn Commands, c: &Control) -> bool {
    let ok = enabled(cmds, &c.cmd.id);
    let resp = ui.add_enabled(ok, egui::Button::new(c.label_or_name(cmds)));
    let r = resp.rect;
    finish(ui, shell, cmds, c, r, resp, ok)
}

/// An item of a drop-down menu, with its key, greyed out when it cannot run. Closes the menu.
pub fn item(ui: &mut Ui, shell: &mut Shell, cmds: &dyn Commands, c: &Control) -> bool {
    let ok = enabled(cmds, &c.cmd.id);
    let key = key_for(shell, cmds, &c.cmd.id);
    let resp = ui.add_enabled(ok, egui::Button::new(c.label_or_name(cmds)).shortcut_text(key));
    let r = resp.rect;
    let clicked = finish(ui, shell, cmds, c, r, resp, ok);
    if clicked {
        ui.close();
    }
    clicked
}

/// A menu item that is on or off (`c.on`).
pub fn check(ui: &mut Ui, shell: &mut Shell, cmds: &dyn Commands, c: &Control) -> bool {
    let ok = enabled(cmds, &c.cmd.id);
    let mut checked = c.on;
    let resp = ui.add_enabled(ok, egui::Checkbox::new(&mut checked, c.label_or_name(cmds)));
    let r = resp.rect;
    finish(ui, shell, cmds, c, r, resp, ok)
}

/// A choice among several in a menu (`c.on` marks the current one). Closes the menu.
pub fn choice(ui: &mut Ui, shell: &mut Shell, cmds: &dyn Commands, c: &Control) -> bool {
    let ok = enabled(cmds, &c.cmd.id);
    let resp = ui.add_enabled(ok, egui::Button::selectable(c.on, c.label_or_name(cmds)));
    let r = resp.rect;
    let clicked = finish(ui, shell, cmds, c, r, resp, ok);
    if clicked {
        ui.close();
    }
    clicked
}

/// Small buttons stacked three to a column.
pub fn stack(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.allocate_ui_with_layout(vec2(0.0, CONTENT_H), egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.spacing_mut().item_spacing.y = 1.5;
        ui.spacing_mut().button_padding = vec2(4.0, 1.0);
        ui.set_max_height(CONTENT_H);
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        add(ui);
    });
}

/// A group: its controls in a row with its title under them. Shrinks (compact: small buttons
/// lose their labels) or folds into one drop-down when the tab is wider than the window.
pub fn group(ui: &mut Ui, shell: &mut Shell, title: &str, add: impl FnOnce(&mut Ui, &mut Shell)) {
    let t = Theme::get(ui.ctx());
    let level = if shell.chrome.unfold_menus || !shell.options.ribbon_fit { ribbon_fit::FULL } else { ribbon_fit::level(ui.ctx(), title) };
    let start = ui.cursor().min.x;
    if level == ribbon_fit::COLLAPSED {
        let (r, resp) = ui.allocate_exact_size(vec2(62.0, CONTENT_H + TITLE_H), Sense::click());
        bg(ui, r, resp.hovered(), false, &t);
        ui.painter().text(pos2(r.center().x, r.min.y + 26.0), Align2::CENTER_CENTER, "▾", regular(16.0), t.text_dim);
        ui.painter().text(pos2(r.center().x, r.max.y - 8.0), Align2::CENTER_CENTER, title, regular(t.type_sizes.caption), t.text_dim);
        let popup = ui.make_persistent_id(("ribbon-group", title));
        record(shell, None, r);
        note_keytip(ui, shell, r, Keytip::Open(popup), title);
        if resp.clicked() {
            egui::Popup::toggle_id(ui.ctx(), popup);
        }
        egui::Popup::new(popup, ui.ctx().clone(), egui::PopupAnchor::from(&resp), ui.layer_id()).open_memory(None).show(|ui| {
            ui.horizontal(|ui| add(ui, shell));
        });
        divider(ui, &t);
        ribbon_fit::record(ui.ctx(), title, level, ui.cursor().min.x - start);
        return;
    }
    ribbon_fit::set_compact(ui.ctx(), level == ribbon_fit::COMPACT);
    // The controls in a band of fixed height, the title in the row under it, so every group's
    // title lines up.
    let top = ui.cursor().min;
    let inner = ui.allocate_ui_with_layout(vec2(ui.available_width(), CONTENT_H), egui::Layout::left_to_right(egui::Align::Min), |ui| {
        ui.set_height(CONTENT_H);
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        add(ui, shell);
    });
    let w = inner.response.rect.width().max(40.0);
    let title_rect = Rect::from_min_size(pos2(top.x, top.y + CONTENT_H), vec2(w, TITLE_H));
    ui.painter().text(title_rect.center(), Align2::CENTER_CENTER, title, regular(t.type_sizes.caption), t.text_dim);
    // Reserve the title row's height beside the band.
    ui.allocate_space(vec2(0.0, CONTENT_H + TITLE_H - inner.response.rect.height()));
    ribbon_fit::end_compact(ui.ctx());
    divider(ui, &t);
    ribbon_fit::record(ui.ctx(), title, level, ui.cursor().min.x - start);
}

fn divider(ui: &mut Ui, t: &Theme) {
    let (r, _) = ui.allocate_exact_size(vec2(9.0, CONTENT_H + TITLE_H), Sense::hover());
    ui.painter().line_segment([pos2(r.center().x, r.min.y + 6.0), pos2(r.center().x, r.max.y - 4.0)], Stroke::new(1.0, t.border));
}

/// A tile in a gallery: icon, name and a second line (`c.second`). `c.on` marks the current one.
pub fn tile(ui: &mut Ui, shell: &mut Shell, cmds: &dyn Commands, c: &Control) -> bool {
    let t = Theme::get(ui.ctx());
    let ok = enabled(cmds, &c.cmd.id);
    let w = if c.width > 0.0 { c.width } else { 112.0 };
    let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), if ok { Sense::click() } else { Sense::hover() });
    bg(ui, r, resp.hovered() && ok, c.on, &t);
    ui.painter().rect_stroke(r, f32::from(t.radius.control), Stroke::new(1.0, if c.on { t.accent } else { t.border }), egui::StrokeKind::Inside);
    let ink = if ok { t.text } else { t.text_dim };
    let name = c.label_or_name(cmds);
    // A one-line tile has a smaller icon, to leave its name room.
    let (at, size) = if c.second.is_empty() { (12.0, 15.0) } else { (13.0, 18.0) };
    crate::icons::paint(ui.painter(), Rect::from_center_size(pos2(r.min.x + at, r.center().y), vec2(size, size)), &c.icon_name(cmds), ink, t.accent);
    if c.second.is_empty() {
        ui.painter().text(pos2(r.min.x + 23.0, r.center().y), Align2::LEFT_CENTER, name, regular(11.0), ink);
    } else {
        ui.painter().text(pos2(r.min.x + 25.0, r.min.y + 9.5), Align2::LEFT_CENTER, name, medium(11.5), ink);
        ui.painter().text(pos2(r.min.x + 25.0, r.min.y + 21.5), Align2::LEFT_CENTER, &c.second, regular(9.5), t.text_dim);
    }
    finish(ui, shell, cmds, c, r, resp, ok)
}

/// A control drawn as text (the quick-access row, the status bar). `text` is what shows.
pub fn link(ui: &mut Ui, shell: &mut Shell, cmds: &dyn Commands, c: &Control, text: impl Into<egui::RichText>) -> bool {
    let ok = enabled(cmds, &c.cmd.id);
    let resp = ui.add_enabled(ok, egui::Button::new(text.into()).frame(false));
    let r = resp.rect;
    finish(ui, shell, cmds, c, r, resp, ok)
}

/// A color chip with a label and a ▾ (a menu goes under it with [`popup_menu`]).
pub fn chip(ui: &mut Ui, color: Color32, label: &str, about: &str) -> Response {
    let t = Theme::get(ui.ctx());
    let g = ui.painter().layout_no_wrap(format!("{label} ▾"), regular(12.0), t.text);
    let (r, resp) = ui.allocate_exact_size(vec2(g.size().x + 30.0, SMALL_H), Sense::click());
    bg(ui, r, resp.hovered(), false, &t);
    let sw = Rect::from_center_size(pos2(r.min.x + 11.0, r.center().y), vec2(12.0, 12.0));
    ui.painter().rect_filled(sw, 3.0, color);
    ui.painter().rect_stroke(sw, 3.0, Stroke::new(1.0, t.border), egui::StrokeKind::Middle);
    ui.painter().galley(pos2(r.min.x + 22.0, r.center().y - g.size().y / 2.0), g, t.text);
    resp.on_hover_text(about)
}
