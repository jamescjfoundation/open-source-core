// SPDX-License-Identifier: MIT OR Apache-2.0
//! The board views driven headlessly: clicks select, drags reorder (corkboard) and move
//! (canvas), and the events apply to the model.

use egui::{Modifiers, Pos2, vec2};
use egui_kittest::Harness;
use jcj_board::{Board, BoardEvent, CORK_CARD, Canvas, CanvasState, CardId, Corkboard, Selection};

#[derive(Default)]
struct State {
    board: Board,
    selection: Selection,
    canvas: CanvasState,
    events: Vec<BoardEvent>,
    origin: Pos2,
    on_canvas: bool,
    fonts: bool,
}

fn state(n: usize, on_canvas: bool) -> State {
    let mut board = Board { snap: false, ..Board::default() };
    for i in 0..n {
        board.add_card(&format!("Card {i}"), "Some words on it.").unwrap();
    }
    State { board, on_canvas, ..State::default() }
}

fn harness(s: State) -> Harness<'static, State> {
    Harness::builder().with_size(vec2(800.0, 600.0)).with_step_dt(0.05).build_ui_state(
        |ui, s: &mut State| {
            // egui has no fonts of its own built in: install the core's defaults, as the shell does.
            if !s.fonts {
                jcj_theme::install_fonts(ui.ctx(), &jcj_theme::FontNames::default(), &[]);
                s.fonts = true;
            }
            s.origin = ui.cursor().min;
            let events = if s.on_canvas {
                Canvas::new("test").show(ui, &s.board, &s.selection, &mut s.canvas)
            } else {
                Corkboard::new("test").show(ui, &s.board, &s.selection)
            };
            for e in &events {
                s.board.apply(e, &mut s.selection);
            }
            s.events.extend(events);
        },
        s,
    )
}

/// The center of corkboard card `i` (three to a row at this width).
fn cork_center(s: &State, i: usize) -> Pos2 {
    let (row, col) = (i / 3, i % 3);
    s.origin + vec2(col as f32 * (CORK_CARD.x + 12.0), row as f32 * (CORK_CARD.y + 12.0)) + CORK_CARD / 2.0
}

fn titles(s: &State) -> Vec<String> {
    s.board.cards.iter().map(|c| c.title.clone()).collect()
}

#[test]
fn corkboard_clicks_select_and_drags_reorder() {
    let mut h = harness(state(3, false));
    h.run();
    let first = cork_center(h.state(), 0);
    h.hover_at(first);
    h.drag_at(first);
    h.drop_at(first);
    h.run();
    assert_eq!(h.state().selection.ids(), [CardId(1)]);
    // Shift-click adds the third card.
    let third = cork_center(h.state(), 2);
    h.event_modifiers(egui::Event::PointerMoved(third), Modifiers::SHIFT);
    h.event_modifiers(
        egui::Event::PointerButton { pos: third, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::SHIFT },
        Modifiers::SHIFT,
    );
    h.event_modifiers(
        egui::Event::PointerButton { pos: third, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::SHIFT },
        Modifiers::SHIFT,
    );
    h.run();
    assert_eq!(h.state().selection.ids(), [CardId(1), CardId(3)]);

    // Drag the first card onto the third: it goes after it.
    h.hover_at(first);
    h.drag_at(first);
    h.run();
    for k in 1..=10 {
        h.hover_at(first + (third - first) * (k as f32 / 10.0));
        h.run();
    }
    h.drop_at(third);
    h.run();
    assert!(h.state().events.contains(&BoardEvent::Reorder { from: 0, before: None }), "{:?}", h.state().events);
    assert_eq!(titles(h.state()), ["Card 1", "Card 2", "Card 0"]);
}

#[test]
fn canvas_drags_move_cards_and_double_clicks_add_them() {
    let mut h = harness(state(2, true));
    h.run();
    let card = h.state().board.cards[0].clone();
    let start = h.state().origin + vec2(card.x + 40.0, card.y + 60.0);
    h.hover_at(start);
    h.drag_at(start);
    h.run();
    for k in 1..=10 {
        h.hover_at(start + vec2(30.0 * k as f32, 20.0 * k as f32));
        h.run();
    }
    h.drop_at(start + vec2(300.0, 200.0));
    h.run();
    let moved = h.state().board.card(card.id).unwrap();
    assert_eq!((moved.x, moved.y), (card.x + 300.0, card.y + 200.0), "{:?}", h.state().events);
    assert_eq!(h.state().selection.ids(), [card.id]);

    // A double-click on the empty canvas adds a card there.
    let empty = h.state().origin + vec2(700.0, 500.0);
    h.hover_at(empty);
    h.run();
    for _ in 0..2 {
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos: empty, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
            h.step();
        }
    }
    h.run();
    assert_eq!(h.state().board.cards.len(), 3, "{:?}", h.state().events);
    let added = h.state().board.cards.last().unwrap().id;
    assert_eq!(h.state().selection.ids(), [added]);

    // Delete removes the selection.
    h.key_press(egui::Key::Delete);
    h.run();
    assert_eq!(h.state().board.cards.len(), 2, "{:?} {:?}", h.state().events, h.state().selection);
}

#[test]
fn canvas_handles_draw_links() {
    let mut h = harness(state(2, true));
    h.run();
    let (a, b) = (h.state().board.cards[0].clone(), h.state().board.cards[1].clone());
    let o = h.state().origin;
    let handle = o + vec2(a.x + a.w, a.y + a.h / 2.0);
    let target = o + vec2(b.x + b.w / 2.0, b.y + b.h / 2.0);
    h.hover_at(handle);
    h.drag_at(handle);
    h.run();
    for k in 1..=10 {
        h.hover_at(handle + (target - handle) * (k as f32 / 10.0));
        h.run();
    }
    h.drop_at(target);
    h.run();
    assert_eq!(h.state().board.links.len(), 1, "{:?}", h.state().events);
    assert_eq!((h.state().board.links[0].from, h.state().board.links[0].to), (a.id, b.id));
    // The cards did not move.
    assert_eq!(h.state().board.cards[0].x, a.x);
}
