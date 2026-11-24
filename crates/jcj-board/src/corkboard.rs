// SPDX-License-Identifier: MIT OR Apache-2.0
//! The corkboard: every card in its order, in rows of equal cards. Dragging a card onto another
//! moves it there: before the card dropped on when it came from later in the order, after it
//! when it came from earlier ([`crate::drop_target`]). A click selects a card (Shift adds to the
//! selection), a double-click opens it.

use egui::{Id, LayerId, Order, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use jcj_theme::Theme;

use crate::card::{hint, paint_card};
use crate::model::{Board, BoardEvent, Selection, drop_target};

/// A corkboard card's size, points.
pub const CORK_CARD: Vec2 = vec2(230.0, 150.0);

/// Space between cards, points.
const GAP: f32 = 12.0;

/// The payload of a card being dragged: its index in the order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CardDrag(usize);

/// The corkboard view. Build one each frame and [`show`](Corkboard::show) it.
#[derive(Clone, Debug)]
pub struct Corkboard {
    id: Id,
    card: Vec2,
    per_row: usize,
    empty_hint: String,
}

impl Corkboard {
    /// A corkboard known by `id_salt` (two on one screen need different salts).
    pub fn new(id_salt: impl std::hash::Hash + std::fmt::Debug) -> Self {
        Corkboard { id: Id::unique(("jcj-corkboard", id_salt)), card: CORK_CARD, per_row: 0, empty_hint: "No cards yet.".into() }
    }

    /// Each card's size when the row fits as many as it can.
    pub fn card_size(mut self, size: Vec2) -> Self {
        self.card = size;
        self
    }

    /// Cards per row (1 to 9); 0 fits as many as the width takes.
    pub fn per_row(mut self, n: usize) -> Self {
        self.per_row = n.min(9);
        self
    }

    /// What an empty board says.
    pub fn empty_hint(mut self, text: impl Into<String>) -> Self {
        self.empty_hint = text.into();
        self
    }

    /// Cards per row and each card's size for a row `width` wide.
    pub fn grid(&self, width: f32) -> (usize, Vec2) {
        let width = if width.is_finite() { width.max(0.0) } else { 0.0 };
        if self.per_row > 0 {
            let n = self.per_row;
            let w = ((width - GAP * (n as f32 - 1.0)) / n as f32).max(120.0);
            (n, vec2(w, self.card.y))
        } else {
            let n = ((width + GAP) / (self.card.x + GAP)).floor().max(1.0) as usize;
            (n, self.card)
        }
    }

    /// Draw `board` and report what the user did.
    pub fn show(self, ui: &mut Ui, board: &Board, selection: &Selection) -> Vec<BoardEvent> {
        let t = Theme::get(ui.ctx());
        let mut events = Vec::new();
        egui::ScrollArea::vertical().id_salt(self.id).auto_shrink(false).show(ui, |ui| {
            let (n, card) = self.grid(ui.available_width());
            let count = board.cards.len();
            let rows = count.div_ceil(n).max(1);
            let height = rows as f32 * (card.y + GAP);
            let (area, bg) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
            if count == 0 {
                hint(ui.painter(), Rect::from_min_size(area.min, vec2(area.width(), card.y)), &self.empty_hint, &t);
            }
            if bg.clicked() {
                events.push(BoardEvent::Select(None));
            }
            let shift = ui.input(|i| i.modifiers.shift);
            for (i, c) in board.cards.iter().enumerate() {
                let (row, col) = (i / n, i % n);
                let r = Rect::from_min_size(area.min + vec2(col as f32 * (card.x + GAP), row as f32 * (card.y + GAP)), card);
                let resp = ui.interact(r, self.id.with(("card", c.id.0)), Sense::click_and_drag());
                if resp.drag_started() || resp.dragged() {
                    resp.dnd_set_drag_payload(CardDrag(i));
                }
                let selected = selection.contains(c.id);
                if resp.dragged() {
                    // Faded where it was, and following the pointer.
                    paint_card(ui.painter(), r, c, selected, &t);
                    ui.painter().rect_filled(r, f32::from(t.radius.card), t.surface.gamma_multiply(0.6));
                    if let Some(p) = ui.ctx().pointer_interact_pos() {
                        let layer = LayerId::new(Order::Tooltip, self.id.with("drag"));
                        let ghost = Rect::from_center_size(p, card);
                        paint_card(&ui.ctx().layer_painter(layer), ghost, c, true, &t);
                    }
                } else {
                    paint_card(ui.painter(), r, c, selected, &t);
                }
                if let Some(from) = resp.dnd_release_payload::<CardDrag>() {
                    if from.0 != i {
                        events.push(BoardEvent::Reorder { from: from.0, before: drop_target(from.0, i, count) });
                    }
                } else if resp.dnd_hover_payload::<CardDrag>().is_some_and(|f| f.0 != i) {
                    // Where it would land: a bar before or after this card.
                    let from = resp.dnd_hover_payload::<CardDrag>().map_or(i, |f| f.0);
                    let x = if from < i { r.max.x + GAP / 2.0 } else { r.min.x - GAP / 2.0 };
                    ui.painter().line_segment([pos2(x, r.min.y), pos2(x, r.max.y)], Stroke::new(3.0, t.accent));
                    ui.painter().rect_stroke(r, f32::from(t.radius.card), Stroke::new(1.0, t.accent), StrokeKind::Outside);
                }
                if resp.double_clicked() {
                    events.push(BoardEvent::Open(c.id));
                } else if resp.clicked() {
                    events.push(if shift { BoardEvent::Toggle(c.id) } else { BoardEvent::Select(Some(c.id)) });
                }
                resp.on_hover_text(if c.title.is_empty() { "Untitled card" } else { c.title.as_str() });
            }
        });
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_fit_the_width_or_the_count_asked() {
        let c = Corkboard::new("t");
        assert_eq!(c.grid(1000.0), (4, CORK_CARD));
        assert_eq!(c.grid(10.0).0, 1, "at least one");
        assert_eq!(c.grid(f32::NAN).0, 1);
        let (n, size) = Corkboard::new("t").per_row(3).grid(624.0);
        assert_eq!((n, size.x), (3, 200.0));
        assert_eq!(Corkboard::new("t").per_row(3).grid(100.0).1.x, 120.0, "never narrower than 120");
    }
}
