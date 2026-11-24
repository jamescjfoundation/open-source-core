// SPDX-License-Identifier: MIT OR Apache-2.0
//! The card window: a card's title and body, Tab moving between them, simple list buttons, and
//! Save.

use egui::{Align2, Context};

use crate::model::{Card, CardId};
use crate::text::{next_number, with_list_line};

/// A card being edited in the card window.
#[derive(Clone, Debug, PartialEq)]
pub struct CardEdit {
    /// The card.
    pub id: CardId,
    /// Its title as typed so far.
    pub title: String,
    /// Its body as typed so far.
    pub body: String,
    /// Which field has the keyboard: the body, else the title.
    pub on_body: bool,
    focus_set: bool,
}

impl CardEdit {
    /// Start editing `card`, in its title.
    pub fn of(card: &Card) -> Self {
        CardEdit { id: card.id, title: card.title.clone(), body: card.body.clone(), on_body: false, focus_set: false }
    }

    /// Tab: the other field.
    pub fn toggle(&mut self) {
        self.on_body = !self.on_body;
        self.focus_set = false;
    }
}

/// Show the card window while `edit` holds a card. Returns the edit when the user saves it (the
/// window then closes); closing the window drops the edit.
pub fn card_window(ctx: &Context, edit: &mut Option<CardEdit>) -> Option<CardEdit> {
    let mut e = edit.take()?;
    let mut open = true;
    let mut save = false;
    egui::Window::new("Card").open(&mut open).collapsible(false).resizable(false).anchor(Align2::RIGHT_TOP, [-16.0, 60.0]).show(ctx, |ui| {
        // Tab moves between the title and the body (taken before the fields see it).
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab)) {
            e.toggle();
        }
        ui.label("Title");
        let t = ui.add(egui::TextEdit::singleline(&mut e.title).desired_width(300.0));
        ui.label("Body");
        let b = ui.add(egui::TextEdit::multiline(&mut e.body).desired_rows(6).desired_width(300.0).lock_focus(true));
        if !e.focus_set {
            if e.on_body {
                b.request_focus()
            } else {
                t.request_focus()
            }
            e.focus_set = true;
        }
        ui.horizontal(|ui| {
            // Simple lists: each button starts a new list line in the body.
            if ui.small_button("• List").clicked() {
                e.body = with_list_line(&e.body, None);
            }
            if ui.small_button("1. List").clicked() {
                let n = next_number(&e.body);
                e.body = with_list_line(&e.body, Some(n));
            }
        });
        ui.horizontal(|ui| {
            save = ui.button("Save").clicked();
            ui.weak("Tab: title / body");
        });
    });
    if save {
        return Some(e);
    }
    if open {
        *edit = Some(e);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_moves_between_title_and_body() {
        let mut e = CardEdit::of(&Card { id: CardId(3), title: "T".into(), ..Card::default() });
        assert!(!e.on_body);
        e.toggle();
        assert!(e.on_body);
        e.toggle();
        assert!(!e.on_body);
    }
}
