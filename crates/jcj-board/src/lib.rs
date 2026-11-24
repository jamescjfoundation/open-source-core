// SPDX-License-Identifier: MIT OR Apache-2.0
//! Cards, a corkboard and a canvas.
//!
//! A [`Board`] holds cards (a title, a body, a color and a label) in an order and at places on a
//! canvas, and the links between them. The data model knows nothing of drawing and is tested on
//! its own; the views draw a board and report what the user did as [`BoardEvent`]s, which the app
//! applies with [`Board::apply`] or runs as its own undoable commands:
//!
//! - [`Corkboard`]: the cards in their order, in rows; drag a card to reorder.
//! - [`Canvas`]: the cards at their places on a dot grid, with lines between them; drag to move,
//!   drag from a card's handle to another card to link them, double-click the canvas to add one.
//! - [`card_window`]: a card's title and body to edit, Tab moving between them.
//!
//! What the cards stand for (chapters, ideas, steps of a plan) is the app's business.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod canvas;
mod card;
mod corkboard;
mod editor;
mod model;
pub mod text;

pub use canvas::{Canvas, CanvasState, edge_point};
pub use card::{card_color, paint_card};
pub use corkboard::{CORK_CARD, Corkboard};
pub use editor::{CardEdit, card_window};
pub use model::{
    Arrange, Board, BoardError, BoardEvent, CANVAS, CARD_H, CARD_H_RANGE, CARD_W, CARD_W_RANGE, COLORS, Card, CardId, Link, LinkStyle, MAX_BODY,
    MAX_CARDS, MAX_LABEL, MAX_LINKS, MAX_TITLE, Selection, drop_target, is_color,
};
