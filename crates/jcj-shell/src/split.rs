// SPDX-License-Identifier: MIT OR Apache-2.0
//! Document tabs and a split view for the central area.
//!
//! [`tabs`] draws a strip of document tabs (select, close) above the document; [`split`] shows two
//! panes side by side or stacked, with a divider the user drags. What the panes hold is the app's:
//! two documents, or two views of one. Promoted for several apps (core issue #4).

use egui::{Align2, CursorIcon, Id, Rect, Sense, Stroke, Ui, UiBuilder, pos2, vec2};
use jcj_theme::{Theme, regular};
use serde::{Deserialize, Serialize};

/// How the central area is split.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Split {
    /// One pane.
    #[default]
    None,
    /// Two panes side by side.
    Side,
    /// Two panes one above the other.
    Stacked,
}

/// Where the divider sits, as a fraction of the area (kept between 0.15 and 0.85).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SplitState {
    /// The first pane's share.
    pub fraction: f32,
}

impl Default for SplitState {
    fn default() -> Self {
        SplitState { fraction: 0.5 }
    }
}

/// The divider's thickness.
pub const DIVIDER: f32 = 6.0;

/// The two panes' rectangles for an area (`None` split: the whole area and an empty one).
pub fn panes(area: Rect, split: Split, fraction: f32) -> (Rect, Rect) {
    let f = fraction.clamp(0.15, 0.85);
    match split {
        Split::None => (area, Rect::NOTHING),
        Split::Side => {
            let x = area.min.x + area.width() * f;
            (Rect::from_min_max(area.min, pos2(x - DIVIDER / 2.0, area.max.y)), Rect::from_min_max(pos2(x + DIVIDER / 2.0, area.min.y), area.max))
        }
        Split::Stacked => {
            let y = area.min.y + area.height() * f;
            (Rect::from_min_max(area.min, pos2(area.max.x, y - DIVIDER / 2.0)), Rect::from_min_max(pos2(area.min.x, y + DIVIDER / 2.0), area.max))
        }
    }
}

/// Show one or two panes in `ui`'s area. Returns which pane was clicked in this frame, if any
/// (0 or 1), so the app can tell which pane has the focus.
pub fn split(ui: &mut Ui, id: Id, how: Split, state: &mut SplitState, first: impl FnOnce(&mut Ui), second: impl FnOnce(&mut Ui)) -> Option<usize> {
    let t = Theme::get(ui.ctx());
    let area = ui.available_rect_before_wrap();
    let (a, b) = panes(area, how, state.fraction);
    let pressed = ui.input(|i| if i.pointer.any_pressed() { i.pointer.interact_pos() } else { None });
    let mut clicked = None;
    ui.scope_builder(UiBuilder::new().max_rect(a).id_salt((id, 0)), |ui| {
        ui.set_clip_rect(a);
        first(ui);
    });
    if pressed.is_some_and(|p| a.contains(p)) {
        clicked = Some(0);
    }
    if how == Split::None {
        ui.allocate_rect(area, Sense::hover());
        return clicked;
    }
    let divider = match how {
        Split::Side => Rect::from_min_max(pos2(a.max.x, area.min.y), pos2(b.min.x, area.max.y)),
        _ => Rect::from_min_max(pos2(area.min.x, a.max.y), pos2(area.max.x, b.min.y)),
    };
    let resp = ui.interact(divider, id.with("divider"), Sense::drag());
    let hover = resp.hovered() || resp.dragged();
    ui.painter().rect_filled(divider, 0.0, if hover { t.accent_soft } else { t.surface });
    let mid = divider.center();
    let line = if how == Split::Side {
        [pos2(mid.x, divider.min.y), pos2(mid.x, divider.max.y)]
    } else {
        [pos2(divider.min.x, mid.y), pos2(divider.max.x, mid.y)]
    };
    ui.painter().line_segment(line, Stroke::new(1.0, t.border));
    if hover {
        ui.ctx().set_cursor_icon(if how == Split::Side { CursorIcon::ResizeHorizontal } else { CursorIcon::ResizeVertical });
    }
    if let Some(p) = resp.interact_pointer_pos()
        && resp.dragged()
    {
        state.fraction = if how == Split::Side { (p.x - area.min.x) / area.width().max(1.0) } else { (p.y - area.min.y) / area.height().max(1.0) }
            .clamp(0.15, 0.85);
    }
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Splitter, true, jcj_i18n::tr("Pane divider")));
    ui.scope_builder(UiBuilder::new().max_rect(b).id_salt((id, 1)), |ui| {
        ui.set_clip_rect(b);
        second(ui);
    });
    if pressed.is_some_and(|p| b.contains(p)) {
        clicked = Some(1);
    }
    ui.allocate_rect(area, Sense::hover());
    clicked
}

/// What the tab strip reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabEvent {
    /// A tab was clicked.
    Select(usize),
    /// A tab's close button was clicked.
    Close(usize),
}

/// The height of the tab strip.
pub const TAB_HEIGHT: f32 = 28.0;

/// A strip of document tabs, the current one marked. Returns what was clicked.
pub fn tabs(ui: &mut Ui, id: Id, titles: &[String], current: usize) -> Option<TabEvent> {
    let t = Theme::get(ui.ctx());
    let (strip, _) = ui.allocate_exact_size(vec2(ui.available_width(), TAB_HEIGHT), Sense::hover());
    ui.painter().rect_filled(strip, 0.0, t.surface_high);
    let mut x = strip.min.x + 4.0;
    let mut event = None;
    for (i, title) in titles.iter().enumerate() {
        let galley = ui.painter().layout_no_wrap(title.clone(), regular(t.type_sizes.body), t.text);
        let w = (galley.size().x + 40.0).clamp(80.0, 240.0);
        let r = Rect::from_min_size(pos2(x, strip.min.y + 3.0), vec2(w, TAB_HEIGHT - 3.0));
        let resp = ui.interact(r, id.with(("tab", i)), Sense::click());
        let on = i == current;
        let fill = if on {
            t.canvas
        } else if resp.hovered() {
            t.hover
        } else {
            t.surface_high
        };
        ui.painter().rect_filled(r, t.radius.control, fill);
        if on {
            ui.painter().line_segment([pos2(r.min.x + 4.0, r.max.y - 1.0), pos2(r.max.x - 4.0, r.max.y - 1.0)], Stroke::new(2.0, t.accent));
        }
        let text_rect = Rect::from_min_max(r.min + vec2(10.0, 0.0), pos2(r.max.x - 22.0, r.max.y));
        ui.painter().with_clip_rect(text_rect).galley(pos2(text_rect.min.x, r.center().y - galley.size().y / 2.0), galley, t.text);
        let close = Rect::from_center_size(pos2(r.max.x - 12.0, r.center().y), vec2(14.0, 14.0));
        let cresp = ui.interact(close, id.with(("close", i)), Sense::click());
        ui.painter().text(close.center(), Align2::CENTER_CENTER, "×", regular(t.type_sizes.body), if cresp.hovered() { t.text } else { t.text_dim });
        let title = title.clone();
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, on, &title));
        cresp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, jcj_i18n::tr("Close tab")));
        if cresp.clicked() {
            event = Some(TabEvent::Close(i));
        } else if resp.clicked() {
            event = Some(TabEvent::Select(i));
        }
        x += w + 2.0;
    }
    event
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    #[test]
    fn panes_share_the_area() {
        let area = Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 600.0));
        let (a, b) = panes(area, Split::Side, 0.3);
        assert!((a.width() - 297.0).abs() < 0.01 && (b.min.x - 303.0).abs() < 0.01);
        let (a, b) = panes(area, Split::Stacked, 0.99);
        assert!((a.height() - 507.0).abs() < 0.01, "the share is kept within reach: {}", a.height());
        assert!(b.height() > 80.0);
        assert_eq!(panes(area, Split::None, 0.5).0, area);
    }

    #[test]
    fn tabs_select_and_close() {
        struct St {
            current: usize,
            closed: Vec<usize>,
        }
        let mut h = Harness::builder().with_size(vec2(600.0, 200.0)).build_ui_state(
            |ui, st: &mut St| {
                let (names, faces) = jcj_theme::with_defaults(&jcj_theme::FontNames::default(), &[]);
                jcj_theme::install_fonts(ui.ctx(), &names, &faces);
                let titles = vec!["Chapter One".to_owned(), "Notes".to_owned()];
                match tabs(ui, Id::unique("tabs"), &titles, st.current) {
                    Some(TabEvent::Select(i)) => st.current = i,
                    Some(TabEvent::Close(i)) => st.closed.push(i),
                    None => {}
                }
                let mut s = SplitState::default();
                split(ui, Id::unique("split"), Split::Side, &mut s, |ui| drop(ui.label("Left")), |ui| drop(ui.label("Right")));
            },
            St { current: 0, closed: Vec::new() },
        );
        h.run();
        h.get_by_label("Notes").click();
        h.run();
        assert_eq!(h.state().current, 1);
        h.get_all_by_label("Close tab").next().unwrap().click();
        h.run();
        assert_eq!(h.state().closed, vec![0]);
        assert!(h.query_by_label("Left").is_some() && h.query_by_label("Right").is_some());
    }
}
