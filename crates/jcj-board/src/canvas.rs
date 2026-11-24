// SPDX-License-Identifier: MIT OR Apache-2.0
//! The canvas: cards at their places on a dot grid, with lines between them. Drag a card to
//! move it, drag from the dot on a card's edge to another card to link them, double-click the
//! empty canvas to add a card, click to select (Shift adds to the selection), double-click a
//! card to open it, Delete or Backspace to delete the selection.

use egui::{Color32, Id, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2};
use jcj_theme::Theme;

use crate::card::{card_color, hint, paint_card};
use crate::model::{Board, BoardEvent, Card, CardId, Selection};

/// What a canvas is in the middle of between frames. The app keeps one per canvas.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CanvasState {
    /// A card being dragged, and how far it has moved.
    dragging: Option<(CardId, Vec2)>,
    /// A link being drawn from this card.
    linking: Option<CardId>,
}

impl CanvasState {
    /// Whether a card is being dragged or a link drawn.
    pub fn busy(&self) -> bool {
        self.dragging.is_some() || self.linking.is_some()
    }
}

/// The canvas view. Build one each frame and [`show`](Canvas::show) it.
#[derive(Clone, Debug)]
pub struct Canvas {
    id: Id,
    delete_key: bool,
    links: bool,
    empty_hint: String,
}

impl Canvas {
    /// A canvas known by `id_salt` (two on one screen need different salts).
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        Canvas {
            id: Id::unique(("jcj-canvas", id_salt)),
            delete_key: true,
            links: true,
            empty_hint: "Double-click to add a card; drag the dot on a card's edge to another card to link them.".into(),
        }
    }

    /// Whether Delete and Backspace delete the selection (on by default). Off when the app
    /// handles those keys itself.
    pub fn delete_key(mut self, on: bool) -> Self {
        self.delete_key = on;
        self
    }

    /// Whether links are drawn and can be made (on by default).
    pub fn links(mut self, on: bool) -> Self {
        self.links = on;
        self
    }

    /// What an empty canvas says.
    pub fn empty_hint(mut self, text: impl Into<String>) -> Self {
        self.empty_hint = text.into();
        self
    }

    /// Draw `board` and report what the user did.
    pub fn show(self, ui: &mut Ui, board: &Board, selection: &Selection, state: &mut CanvasState) -> Vec<BoardEvent> {
        let t = Theme::get(ui.ctx());
        let mut events = Vec::new();
        let extent = board.cards.iter().fold(vec2(1600.0, 1000.0), |e, c| vec2(e.x.max(c.x + c.w + 400.0), e.y.max(c.y + c.h + 400.0)));
        egui::ScrollArea::both().id_salt(self.id).auto_shrink(false).show(ui, |ui| {
            let (canvas, bg) = ui.allocate_exact_size(extent, Sense::click());
            let origin = canvas.min;
            let painter = ui.painter_at(canvas);
            painter.rect_filled(canvas, 0.0, ui.visuals().extreme_bg_color);
            // The grid: a dot at every crossing in view.
            let clip = ui.clip_rect().intersect(canvas);
            let g = board.grid.max(5.0);
            let dot = ui.visuals().weak_text_color().gamma_multiply(0.5);
            let mut y = ((clip.min.y - origin.y) / g).floor() * g;
            while origin.y + y <= clip.max.y {
                let mut x = ((clip.min.x - origin.x) / g).floor() * g;
                while origin.x + x <= clip.max.x {
                    painter.circle_filled(origin + vec2(x, y), 1.0, dot);
                    x += g;
                }
                y += g;
            }
            if board.cards.is_empty() {
                hint(&painter, Rect::from_min_size(origin, vec2(clip.width().min(extent.x), 120.0)), &self.empty_hint, &t);
            }
            // Where each card is drawn now (a dragged one follows the pointer).
            let dragging = state.dragging;
            let shown = |c: &Card| {
                let r = Rect::from_min_size(origin + vec2(c.x, c.y), vec2(c.w, c.h));
                match dragging {
                    Some((id, d)) if id == c.id => r.translate(d),
                    _ => r,
                }
            };
            // Links, under the cards.
            let line = Stroke::new(2.0, ui.visuals().text_color().gamma_multiply(0.6));
            if self.links {
                for l in &board.links {
                    let (Some(a), Some(b)) = (board.card(l.from), board.card(l.to)) else { continue };
                    let (ra, rb) = (shown(a), shown(b));
                    let (p, q) = (edge_point(ra, rb.center()), edge_point(rb, ra.center()));
                    let stroke = if l.color.is_empty() { line } else { Stroke::new(2.0, card_color(&l.color)) };
                    painter.line_segment([p, q], stroke);
                    if l.style.head_at_to() {
                        painter.add(arrow_head(q, p, stroke.color));
                    }
                    if l.style.head_at_from() {
                        painter.add(arrow_head(p, q, stroke.color));
                    }
                }
            }
            let pointer = ui.ctx().pointer_interact_pos();
            let shift = ui.input(|i| i.modifiers.shift);
            for c in &board.cards {
                let r = shown(c);
                let resp = ui.interact(r, self.id.with(("card", c.id.0)), Sense::click_and_drag());
                paint_card(&painter, r, c, selection.contains(c.id), &t);
                let name = if c.title.is_empty() { "Untitled card".to_string() } else { c.title.clone() };
                resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, selection.contains(c.id), &name));
                if self.links {
                    let handle = pos2(r.max.x, r.center().y);
                    painter.circle_filled(handle, 5.0, t.accent);
                    let hresp = ui.interact(Rect::from_center_size(handle, vec2(14.0, 14.0)), self.id.with(("link", c.id.0)), Sense::drag());
                    if hresp.drag_started() {
                        state.linking = Some(c.id);
                    }
                }
                if resp.double_clicked() {
                    events.push(BoardEvent::Open(c.id));
                } else if resp.clicked() {
                    events.push(if shift { BoardEvent::Toggle(c.id) } else { BoardEvent::Select(Some(c.id)) });
                }
                if resp.drag_started() && state.linking.is_none() {
                    state.dragging = Some((c.id, Vec2::ZERO));
                    if !selection.contains(c.id) {
                        events.push(BoardEvent::Select(Some(c.id)));
                    }
                }
                if resp.dragged()
                    && let Some((id, d)) = state.dragging.as_mut()
                    && *id == c.id
                {
                    *d += resp.drag_delta();
                }
                if resp.drag_stopped()
                    && let Some((id, d)) = state.dragging.take()
                    && id == c.id
                    && d != Vec2::ZERO
                {
                    events.push(BoardEvent::Moved { id, x: (c.x + d.x).max(0.0), y: (c.y + d.y).max(0.0) });
                }
            }
            // A link being drawn, and where it lands.
            if let Some(from) = state.linking {
                if let (Some(a), Some(p)) = (board.card(from), pointer) {
                    painter.line_segment([shown(a).center(), p], line);
                }
                if ui.input(|i| i.pointer.any_released()) {
                    if let Some(target) = pointer.and_then(|p| board.cards.iter().find(|c| c.id != from && shown(c).contains(p))) {
                        events.push(BoardEvent::Link { from, to: target.id });
                    }
                    state.linking = None;
                }
            }
            if bg.double_clicked()
                && let Some(p) = pointer
            {
                let at = p - origin;
                events.push(BoardEvent::AddAt { x: at.x.max(0.0), y: at.y.max(0.0) });
            } else if bg.clicked() {
                events.push(BoardEvent::Select(None));
            }
        });
        // Delete removes the selection when no field has the keyboard.
        if self.delete_key
            && !selection.is_empty()
            && !ui.ctx().text_edit_focused()
            && ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
        {
            events.push(BoardEvent::Delete(selection.ids()));
        }
        events
    }
}

/// An arrowhead at `tip`, pointing away from `from`.
fn arrow_head(tip: Pos2, from: Pos2, color: Color32) -> Shape {
    let dir = (tip - from).normalized();
    let side = vec2(-dir.y, dir.x);
    Shape::convex_polygon(vec![tip, tip - dir * 10.0 + side * 5.0, tip - dir * 10.0 - side * 5.0], color, Stroke::NONE)
}

/// Where a line from the center of `r` towards `to` leaves `r`'s edge.
pub fn edge_point(r: Rect, to: Pos2) -> Pos2 {
    let c = r.center();
    let d = to - c;
    if d.length() < 1.0 {
        return c;
    }
    let tx = if d.x.abs() > 1e-3 { (r.width() / 2.0) / d.x.abs() } else { f32::INFINITY };
    let ty = if d.y.abs() > 1e-3 { (r.height() / 2.0) / d.y.abs() } else { f32::INFINITY };
    c + d * tx.min(ty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_lines_leave_from_the_edges() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 50.0));
        assert_eq!(edge_point(r, pos2(500.0, 25.0)), pos2(100.0, 25.0));
        assert_eq!(edge_point(r, pos2(50.0, -500.0)), pos2(50.0, 0.0));
        assert_eq!(edge_point(r, r.center()), r.center());
    }
}
