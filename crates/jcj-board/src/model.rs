// SPDX-License-Identifier: MIT OR Apache-2.0
//! The board's data: cards, the links between them, the grid, and which cards are selected.
//!
//! One set of cards serves both views: the corkboard shows them in their order (dragging a card
//! reorders the list), the canvas at their positions (dragging moves it). Nothing here draws, so
//! an app can keep a board in its document, change it through its own commands (for undo), and
//! test every change without a window.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// Limits that keep hostile or runaway data in check.
pub const MAX_CARDS: usize = 5_000;
/// Most links a board keeps.
pub const MAX_LINKS: usize = 20_000;
/// Longest card title kept, characters.
pub const MAX_TITLE: usize = 300;
/// Longest card body kept, characters.
pub const MAX_BODY: usize = 10_000;
/// Longest card label kept, characters.
pub const MAX_LABEL: usize = 60;
/// The canvas's size, points: cards stay inside it.
pub const CANVAS: f32 = 20_000.0;
/// A new card's size on the canvas, points.
pub const CARD_W: f32 = 180.0;
/// A new card's height on the canvas, points.
pub const CARD_H: f32 = 110.0;
/// Smallest and largest card width, points.
pub const CARD_W_RANGE: (f32, f32) = (60.0, 1_000.0);
/// Smallest and largest card height, points.
pub const CARD_H_RANGE: (f32, f32) = (40.0, 1_000.0);

/// The card colors offered: pale index-card tints. Any `#RRGGBB` is kept.
pub const COLORS: [&str; 8] = ["#FFF2A8", "#FFD1A8", "#F7B5C4", "#D9C2F0", "#B8D4F5", "#B8E6D0", "#D8E8A8", "#E6E6E6"];

/// A card's id, unique on its board.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CardId(pub u32);

/// One card.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Card {
    /// Its id.
    pub id: CardId,
    /// One line.
    pub title: String,
    /// Any number of lines.
    pub body: String,
    /// `#RRGGBB`.
    pub color: String,
    /// A short label shown on the card (a status, a tag); may be empty.
    pub label: String,
    /// Its place on the canvas, points from the canvas's top left.
    pub x: f32,
    /// Its place on the canvas, points from the canvas's top.
    pub y: f32,
    /// Its width on the canvas.
    pub w: f32,
    /// Its height on the canvas.
    pub h: f32,
}

impl Default for Card {
    fn default() -> Self {
        Card {
            id: CardId(0),
            title: String::new(),
            body: String::new(),
            color: COLORS[0].into(),
            label: String::new(),
            x: 0.0,
            y: 0.0,
            w: CARD_W,
            h: CARD_H,
        }
    }
}

/// How a link is drawn: a plain line, an arrow forward (to `to`), back, or both ways.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkStyle {
    /// No arrowheads.
    Line,
    /// An arrowhead at `to`.
    #[default]
    Forward,
    /// An arrowhead at `from`.
    Back,
    /// Arrowheads at both ends.
    Both,
}

impl LinkStyle {
    /// Every style, in the order menus offer them.
    pub const ALL: [LinkStyle; 4] = [LinkStyle::Line, LinkStyle::Forward, LinkStyle::Back, LinkStyle::Both];

    /// Whether the link has an arrowhead at `to`.
    pub fn head_at_to(self) -> bool {
        matches!(self, LinkStyle::Forward | LinkStyle::Both)
    }

    /// Whether the link has an arrowhead at `from`.
    pub fn head_at_from(self) -> bool {
        matches!(self, LinkStyle::Back | LinkStyle::Both)
    }
}

/// A line from one card to another.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Link {
    /// Where it starts.
    pub from: CardId,
    /// Where it ends.
    pub to: CardId,
    /// How it is drawn.
    pub style: LinkStyle,
    /// `#RRGGBB`, or empty for the theme's line color.
    pub color: String,
}

/// How [`Board::arrange`] lines cards up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arrange {
    /// Each card to the nearest grid point.
    Grid,
    /// One under another, in reading order.
    Column,
    /// Side by side, left to right.
    Row,
    /// Stepped down and to the right.
    Cascade,
}

impl Arrange {
    /// Every way, with its menu label.
    pub const ALL: [(Arrange, &'static str); 4] =
        [(Arrange::Grid, "On the Grid"), (Arrange::Column, "In a Column"), (Arrange::Row, "In a Row"), (Arrange::Cascade, "Cascade")];
}

/// Why a change to a board was refused.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BoardError {
    /// No card has this id.
    #[error("there is no card {0}")]
    NoCard(u32),
    /// A card with this id is already on the board.
    #[error("card {0} is already on the board")]
    Duplicate(u32),
    /// The board holds as many cards (or links) as it may.
    #[error("a board holds at most {0} {1}")]
    Full(usize, &'static str),
    /// A link must join two different cards.
    #[error("a link joins two different cards")]
    SelfLink,
    /// A color that is not `#RRGGBB`.
    #[error("`{0}` is not a color: write #RRGGBB")]
    Color(String),
}

/// Cards, links and the grid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Board {
    /// The cards, in order: the corkboard shows them in this order.
    pub cards: Vec<Card>,
    /// Lines between cards.
    pub links: Vec<Link>,
    /// Grid spacing, points.
    pub grid: f32,
    /// Whether cards placed on the canvas snap to the grid.
    pub snap: bool,
}

impl Default for Board {
    fn default() -> Self {
        Board { cards: Vec::new(), links: Vec::new(), grid: 20.0, snap: true }
    }
}

/// Text with control characters (other than new lines and tabs) taken out, cut to `max`
/// characters.
fn clean_text(s: &str, max: usize) -> String {
    s.replace("\r\n", "\n").chars().filter(|c| *c == '\n' || *c == '\t' || !c.is_control()).take(max).collect()
}

/// One line of text: [`clean_text`] with new lines as spaces.
fn clean_line(s: &str, max: usize) -> String {
    clean_text(s, max).replace(['\n', '\t'], " ")
}

/// Whether `c` is `#RRGGBB`.
pub fn is_color(c: &str) -> bool {
    let h = c.trim();
    h.len() == 7 && h.starts_with('#') && h[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

fn finite_in(v: f32, lo: f32, hi: f32, default: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { default }
}

/// The index a card dragged from index `from` and dropped on the card at `onto` goes before,
/// among `count` cards (`None`: to the end). Dragged later, it goes after the card dropped on;
/// dragged earlier, before it.
pub fn drop_target(from: usize, onto: usize, count: usize) -> Option<usize> {
    if from < onto { (onto + 1 < count).then_some(onto + 1) } else { Some(onto) }
}

impl Board {
    /// The card with `id`.
    pub fn card(&self, id: CardId) -> Option<&Card> {
        self.cards.iter().find(|c| c.id == id)
    }

    /// The card with `id`, to change.
    pub fn card_mut(&mut self, id: CardId) -> Option<&mut Card> {
        self.cards.iter_mut().find(|c| c.id == id)
    }

    /// Where the card with `id` is in the order.
    pub fn index_of(&self, id: CardId) -> Option<usize> {
        self.cards.iter().position(|c| c.id == id)
    }

    /// An id no card has yet.
    pub fn next_id(&self) -> CardId {
        CardId(self.cards.iter().map(|c| c.id.0).max().map_or(1, |m| m.saturating_add(1)))
    }

    /// `v` on the grid, when snapping is on.
    pub fn snapped(&self, v: f32) -> f32 {
        if self.snap && self.grid > 0.0 { (v / self.grid).round() * self.grid } else { v }
    }

    /// A new card with `title` and `body` at the end of the order, placed at the next free spot
    /// on the canvas (six to a row) in the next color. Returns its id.
    pub fn add_card(&mut self, title: &str, body: &str) -> Result<CardId, BoardError> {
        let n = self.cards.len() as f32;
        let x = self.snapped(40.0 + (n % 6.0) * (CARD_W + 20.0));
        let y = self.snapped(40.0 + (n / 6.0).floor() * (CARD_H + 20.0));
        let color = COLORS.get(self.cards.len() % COLORS.len()).copied().unwrap_or(COLORS[0]);
        let card = Card {
            id: self.next_id(),
            title: clean_line(title, MAX_TITLE),
            body: clean_text(body, MAX_BODY),
            color: color.into(),
            x,
            y,
            ..Card::default()
        };
        self.insert(card)
    }

    /// A new card centred on canvas point (`x`, `y`): what a double-click on the canvas adds.
    pub fn add_card_at(&mut self, x: f32, y: f32) -> Result<CardId, BoardError> {
        let id = self.add_card("", "")?;
        self.set_position(id, x - CARD_W / 2.0, y - CARD_H / 2.0)?;
        Ok(id)
    }

    /// Put `card` at the end of the order. Its text is cleaned and its place kept on the canvas.
    pub fn insert(&mut self, mut card: Card) -> Result<CardId, BoardError> {
        if self.cards.len() >= MAX_CARDS {
            return Err(BoardError::Full(MAX_CARDS, "cards"));
        }
        if self.card(card.id).is_some() {
            return Err(BoardError::Duplicate(card.id.0));
        }
        repair_card(&mut card);
        let id = card.id;
        self.cards.push(card);
        Ok(id)
    }

    /// Take the card with `id` off the board, with its links.
    pub fn remove(&mut self, id: CardId) -> Option<Card> {
        let i = self.index_of(id)?;
        self.links.retain(|l| l.from != id && l.to != id);
        Some(self.cards.remove(i))
    }

    /// Move the card at index `from` to go before the card now at index `before` (`None`: to the
    /// end). Returns whether the order changed.
    pub fn reorder(&mut self, from: usize, before: Option<usize>) -> bool {
        if from >= self.cards.len() {
            return false;
        }
        let to = match before {
            Some(b) if b > self.cards.len() => return false,
            Some(b) if b > from => b - 1,
            Some(b) => b,
            None => self.cards.len() - 1,
        };
        if to == from {
            return false;
        }
        let card = self.cards.remove(from);
        self.cards.insert(to.min(self.cards.len()), card);
        true
    }

    /// Place the card with `id` at (`x`, `y`) on the canvas, on the grid when snapping is on.
    pub fn set_position(&mut self, id: CardId, x: f32, y: f32) -> Result<(), BoardError> {
        let (x, y) = (self.snapped(finite_in(x, 0.0, CANVAS, 0.0)), self.snapped(finite_in(y, 0.0, CANVAS, 0.0)));
        let c = self.card_mut(id).ok_or(BoardError::NoCard(id.0))?;
        c.x = x.clamp(0.0, CANVAS);
        c.y = y.clamp(0.0, CANVAS);
        Ok(())
    }

    /// Move the card with `id` by (`dx`, `dy`) points.
    pub fn move_by(&mut self, id: CardId, dx: f32, dy: f32) -> Result<(), BoardError> {
        let c = self.card(id).ok_or(BoardError::NoCard(id.0))?;
        let (x, y) = (c.x + dx, c.y + dy);
        self.set_position(id, x, y)
    }

    /// Give the card with `id` a new size, within [`CARD_W_RANGE`] and [`CARD_H_RANGE`].
    pub fn resize(&mut self, id: CardId, w: f32, h: f32) -> Result<(), BoardError> {
        let (w, h) = (self.snapped(w), self.snapped(h));
        let c = self.card_mut(id).ok_or(BoardError::NoCard(id.0))?;
        c.w = finite_in(w, CARD_W_RANGE.0, CARD_W_RANGE.1, CARD_W);
        c.h = finite_in(h, CARD_H_RANGE.0, CARD_H_RANGE.1, CARD_H);
        Ok(())
    }

    /// Set the card's title and body (cleaned and cut to their limits).
    pub fn set_text(&mut self, id: CardId, title: &str, body: &str) -> Result<(), BoardError> {
        let c = self.card_mut(id).ok_or(BoardError::NoCard(id.0))?;
        c.title = clean_line(title, MAX_TITLE);
        c.body = clean_text(body, MAX_BODY);
        Ok(())
    }

    /// Set the card's color (`#RRGGBB`).
    pub fn set_color(&mut self, id: CardId, color: &str) -> Result<(), BoardError> {
        if !is_color(color) {
            return Err(BoardError::Color(color.to_string()));
        }
        let c = self.card_mut(id).ok_or(BoardError::NoCard(id.0))?;
        c.color = color.trim().to_uppercase();
        Ok(())
    }

    /// Set the card's label.
    pub fn set_label(&mut self, id: CardId, label: &str) -> Result<(), BoardError> {
        let c = self.card_mut(id).ok_or(BoardError::NoCard(id.0))?;
        c.label = clean_line(label, MAX_LABEL);
        Ok(())
    }

    /// Link two cards (once: a link back the other way is a different link). Linking two cards
    /// already linked changes the link's style.
    pub fn link(&mut self, from: CardId, to: CardId, style: LinkStyle) -> Result<(), BoardError> {
        if from == to {
            return Err(BoardError::SelfLink);
        }
        for id in [from, to] {
            self.card(id).ok_or(BoardError::NoCard(id.0))?;
        }
        if let Some(l) = self.links.iter_mut().find(|l| l.from == from && l.to == to) {
            l.style = style;
            return Ok(());
        }
        if self.links.len() >= MAX_LINKS {
            return Err(BoardError::Full(MAX_LINKS, "links"));
        }
        self.links.push(Link { from, to, style, color: String::new() });
        Ok(())
    }

    /// Remove the link from `from` to `to`. Returns whether there was one.
    pub fn unlink(&mut self, from: CardId, to: CardId) -> bool {
        let before = self.links.len();
        self.links.retain(|l| !(l.from == from && l.to == to));
        self.links.len() != before
    }

    /// The links to or from the card with `id`.
    pub fn links_of(&self, id: CardId) -> Vec<&Link> {
        self.links.iter().filter(|l| l.from == id || l.to == id).collect()
    }

    /// Line the cards `ids` up, starting from where the top-left one of them is. Returns how
    /// many cards moved into place.
    pub fn arrange(&mut self, ids: &[CardId], how: Arrange) -> usize {
        let mut picked: Vec<Card> = self.cards.iter().filter(|c| ids.contains(&c.id)).cloned().collect();
        if picked.is_empty() {
            return 0;
        }
        // Reading order: top to bottom, then left to right (rows), or left to right (row).
        if how == Arrange::Row {
            picked.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
        } else {
            picked.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
        }
        let x0 = picked.iter().map(|c| c.x).fold(f32::INFINITY, f32::min);
        let y0 = picked.iter().map(|c| c.y).fold(f32::INFINITY, f32::min);
        let grid = self.grid.max(1.0);
        let gap = grid.max(10.0);
        let (mut x, mut y) = (x0, y0);
        for p in &picked {
            let (nx, ny) = match how {
                // On the grid whether snapping is on or not.
                Arrange::Grid => ((p.x / grid).round() * grid, (p.y / grid).round() * grid),
                Arrange::Column => {
                    let at = (x0, y);
                    y += p.h + gap;
                    at
                }
                Arrange::Row => {
                    let at = (x, y0);
                    x += p.w + gap;
                    at
                }
                Arrange::Cascade => {
                    let at = (x, y);
                    x += 30.0;
                    y += 30.0;
                    at
                }
            };
            if let Some(c) = self.card_mut(p.id) {
                c.x = nx.clamp(0.0, CANVAS);
                c.y = ny.clamp(0.0, CANVAS);
            }
        }
        picked.len()
    }

    /// The board brought within the limits, for data read from a file: cards past the limit,
    /// repeated ids, and links to missing cards are dropped; text is cleaned; sizes and places
    /// are kept on the canvas; bad colors take the default.
    pub fn repaired(mut self) -> Board {
        self.cards.truncate(MAX_CARDS);
        let mut seen = HashSet::new();
        self.cards.retain(|c| seen.insert(c.id));
        for c in &mut self.cards {
            repair_card(c);
        }
        let ids: HashSet<CardId> = self.cards.iter().map(|c| c.id).collect();
        self.links.truncate(MAX_LINKS);
        self.links.retain(|l| l.from != l.to && ids.contains(&l.from) && ids.contains(&l.to));
        let mut seen_links = HashSet::new();
        self.links.retain(|l| seen_links.insert((l.from, l.to)));
        for l in &mut self.links {
            if !is_color(&l.color) {
                l.color.clear();
            }
        }
        self.grid = finite_in(self.grid, 5.0, 200.0, 20.0);
        self
    }

    /// Apply what a board view reported. Selection changes go to `selection`; the rest change
    /// the board. Returns whether the board itself changed. Apps that run changes as undoable
    /// commands handle the events themselves instead.
    pub fn apply(&mut self, event: &BoardEvent, selection: &mut Selection) -> bool {
        match event {
            BoardEvent::Select(id) => {
                selection.select(*id);
                false
            }
            BoardEvent::Toggle(id) => {
                selection.toggle(*id);
                false
            }
            BoardEvent::Open(_) => false,
            BoardEvent::Moved { id, x, y } => self.set_position(*id, *x, *y).is_ok(),
            BoardEvent::Reorder { from, before } => self.reorder(*from, *before),
            BoardEvent::AddAt { x, y } => match self.add_card_at(*x, *y) {
                Ok(id) => {
                    selection.select(Some(id));
                    true
                }
                Err(_) => false,
            },
            BoardEvent::Link { from, to } => self.link(*from, *to, LinkStyle::Forward).is_ok(),
            BoardEvent::Delete(ids) => {
                let mut changed = false;
                for id in ids {
                    changed |= self.remove(*id).is_some();
                }
                selection.retain(|id| self.card(id).is_some());
                changed
            }
        }
    }
}

fn repair_card(c: &mut Card) {
    c.title = clean_line(&c.title, MAX_TITLE);
    c.body = clean_text(&c.body, MAX_BODY);
    c.label = clean_line(&c.label, MAX_LABEL);
    if !is_color(&c.color) {
        c.color = COLORS[0].into();
    }
    c.x = finite_in(c.x, 0.0, CANVAS, 0.0);
    c.y = finite_in(c.y, 0.0, CANVAS, 0.0);
    c.w = finite_in(c.w, CARD_W_RANGE.0, CARD_W_RANGE.1, CARD_W);
    c.h = finite_in(c.h, CARD_H_RANGE.0, CARD_H_RANGE.1, CARD_H);
}

/// What a board view reports: the user's gestures, for the app to apply (with
/// [`Board::apply`], or as its own undoable commands).
#[derive(Clone, Debug, PartialEq)]
pub enum BoardEvent {
    /// Select this card alone, or nothing.
    Select(Option<CardId>),
    /// Add this card to the selection, or take it out (Shift-click).
    Toggle(CardId),
    /// Open this card to edit it (double-click).
    Open(CardId),
    /// A card was dragged to (`x`, `y`) on the canvas.
    Moved {
        /// The card.
        id: CardId,
        /// Its new left edge.
        x: f32,
        /// Its new top edge.
        y: f32,
    },
    /// A card was dragged to a new place in the order ([`Board::reorder`]).
    Reorder {
        /// Its index before the move.
        from: usize,
        /// The index it goes before (`None`: the end).
        before: Option<usize>,
    },
    /// Add a card centred here on the canvas (a double-click on the empty canvas).
    AddAt {
        /// Canvas x.
        x: f32,
        /// Canvas y.
        y: f32,
    },
    /// A link was drawn from one card to another.
    Link {
        /// Where it starts.
        from: CardId,
        /// Where it ends.
        to: CardId,
    },
    /// Delete these cards (Delete or Backspace).
    Delete(Vec<CardId>),
}

/// Which cards are selected: a main one, and others added with Shift.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Selection {
    primary: Option<CardId>,
    also: Vec<CardId>,
}

impl Selection {
    /// The main selected card.
    pub fn primary(&self) -> Option<CardId> {
        self.primary
    }

    /// Every selected card, the main one first.
    pub fn ids(&self) -> Vec<CardId> {
        let mut v: Vec<CardId> = self.primary.into_iter().collect();
        v.extend(self.also.iter().copied().filter(|x| Some(*x) != self.primary));
        v
    }

    /// Whether `id` is selected.
    pub fn contains(&self, id: CardId) -> bool {
        self.primary == Some(id) || self.also.contains(&id)
    }

    /// Whether nothing is selected.
    pub fn is_empty(&self) -> bool {
        self.primary.is_none() && self.also.is_empty()
    }

    /// Select `id` alone (`None`: nothing).
    pub fn select(&mut self, id: Option<CardId>) {
        self.primary = id;
        self.also.clear();
    }

    /// Add `id` to the selection, or take it out. Taking out the main card makes the last one
    /// added the main one.
    pub fn toggle(&mut self, id: CardId) {
        if self.primary == Some(id) {
            self.primary = self.also.pop();
        } else if let Some(i) = self.also.iter().position(|x| *x == id) {
            self.also.remove(i);
        } else if self.primary.is_none() {
            self.primary = Some(id);
        } else {
            self.also.push(id);
        }
    }

    /// Keep only the cards `keep` says yes to.
    pub fn retain(&mut self, mut keep: impl FnMut(CardId) -> bool) {
        self.also.retain(|id| keep(*id));
        if self.primary.is_some_and(|id| !keep(id)) {
            self.primary = self.also.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn board(n: usize) -> Board {
        let mut b = Board::default();
        for i in 0..n {
            b.add_card(&format!("Card {i}"), "").unwrap();
        }
        b
    }

    fn titles(b: &Board) -> Vec<&str> {
        b.cards.iter().map(|c| c.title.as_str()).collect()
    }

    #[test]
    fn drops_land_where_the_user_expects() {
        // Dragged later: after the card dropped on; earlier: before it.
        assert_eq!(drop_target(0, 2, 4), Some(3));
        assert_eq!(drop_target(0, 3, 4), None, "after the last: to the end");
        assert_eq!(drop_target(3, 1, 4), Some(1));
    }

    #[test]
    fn reorder_moves_a_card_before_another() {
        let mut b = board(4);
        assert!(b.reorder(0, drop_target(0, 2, 4)));
        assert_eq!(titles(&b), ["Card 1", "Card 2", "Card 0", "Card 3"]);
        assert!(b.reorder(3, drop_target(3, 0, 4)));
        assert_eq!(titles(&b), ["Card 3", "Card 1", "Card 2", "Card 0"]);
        assert!(b.reorder(1, None));
        assert_eq!(titles(&b), ["Card 3", "Card 2", "Card 0", "Card 1"]);
        assert!(!b.reorder(3, None), "already last");
        assert!(!b.reorder(2, Some(3)), "already before 3");
        assert!(!b.reorder(9, None));
        assert!(!b.reorder(0, Some(9)));
    }

    #[test]
    fn cards_get_ids_places_and_colors() {
        let mut b = board(7);
        let ids: Vec<u32> = b.cards.iter().map(|c| c.id.0).collect();
        assert_eq!(ids, [1, 2, 3, 4, 5, 6, 7]);
        // Six to a row, on the grid.
        assert_eq!((b.cards[1].x, b.cards[1].y), (240.0, 40.0));
        assert_eq!((b.cards[6].x, b.cards[6].y), (40.0, 180.0));
        assert_ne!(b.cards[0].color, b.cards[1].color);
        b.remove(CardId(7));
        assert_eq!(b.next_id(), CardId(7));
        assert_eq!(b.insert(Card { id: CardId(2), ..Card::default() }), Err(BoardError::Duplicate(2)));
    }

    #[test]
    fn moving_snaps_and_stays_on_the_canvas() {
        let mut b = board(1);
        let id = CardId(1);
        b.set_position(id, 33.0, 47.0).unwrap();
        assert_eq!((b.cards[0].x, b.cards[0].y), (40.0, 40.0));
        b.snap = false;
        b.move_by(id, 3.5, -100.0).unwrap();
        assert_eq!((b.cards[0].x, b.cards[0].y), (43.5, 0.0));
        b.set_position(id, f32::NAN, 1e9).unwrap();
        assert_eq!((b.cards[0].x, b.cards[0].y), (0.0, CANVAS));
        assert_eq!(b.set_position(CardId(9), 0.0, 0.0), Err(BoardError::NoCard(9)));
        b.resize(id, 5.0, 5_000.0).unwrap();
        assert_eq!((b.cards[0].w, b.cards[0].h), (CARD_W_RANGE.0, CARD_H_RANGE.1));
    }

    #[test]
    fn links_join_two_cards_once_and_go_with_them() {
        let mut b = board(3);
        let (a, c, d) = (CardId(1), CardId(2), CardId(3));
        b.link(a, c, LinkStyle::Forward).unwrap();
        b.link(a, c, LinkStyle::Both).unwrap();
        assert_eq!(b.links.len(), 1);
        assert_eq!(b.links[0].style, LinkStyle::Both);
        b.link(c, a, LinkStyle::Line).unwrap();
        b.link(c, d, LinkStyle::Line).unwrap();
        assert_eq!(b.link(a, a, LinkStyle::Line), Err(BoardError::SelfLink));
        assert_eq!(b.link(a, CardId(9), LinkStyle::Line), Err(BoardError::NoCard(9)));
        assert_eq!(b.links_of(a).len(), 2);
        assert!(b.unlink(c, a));
        assert!(!b.unlink(c, a));
        b.remove(c);
        assert!(b.links.is_empty(), "a card's links go with it");
    }

    #[test]
    fn arrange_lines_cards_up_from_the_top_left() {
        let mut b = board(3);
        b.snap = false;
        for (i, (x, y)) in [(300.0, 50.0), (100.0, 400.0), (100.0, 50.0)].into_iter().enumerate() {
            b.set_position(b.cards[i].id, x, y).unwrap();
        }
        let ids: Vec<CardId> = b.cards.iter().map(|c| c.id).collect();
        assert_eq!(b.arrange(&ids, Arrange::Row), 3);
        let at = |b: &Board, i: u32| b.card(CardId(i)).map(|c| (c.x, c.y)).unwrap();
        // Left to right: card 2 (x 100, lower), card 3 (x 100), card 1 (x 300).
        assert_eq!(at(&b, 3), (100.0, 50.0));
        assert_eq!(at(&b, 2), (100.0 + CARD_W + 20.0, 50.0));
        b.arrange(&ids, Arrange::Column);
        assert_eq!(at(&b, 3).0, 100.0);
        assert!(b.cards.iter().all(|c| c.x == 100.0));
        b.set_position(CardId(1), 107.0, 13.0).unwrap();
        b.arrange(&[CardId(1)], Arrange::Grid);
        assert_eq!(at(&b, 1), (100.0, 20.0));
        assert_eq!(b.arrange(&[CardId(42)], Arrange::Grid), 0);
    }

    #[test]
    fn selection_adds_and_removes_with_shift() {
        let (a, b, c) = (CardId(1), CardId(2), CardId(3));
        let mut s = Selection::default();
        assert!(s.is_empty());
        s.toggle(a);
        assert_eq!(s.primary(), Some(a));
        s.toggle(b);
        s.toggle(c);
        assert_eq!(s.ids(), [a, b, c]);
        s.toggle(b);
        assert_eq!(s.ids(), [a, c]);
        s.toggle(a);
        assert_eq!(s.primary(), Some(c), "the last added becomes the main one");
        s.select(Some(b));
        assert_eq!(s.ids(), [b]);
        assert!(s.contains(b) && !s.contains(c));
        s.retain(|id| id != b);
        assert!(s.is_empty());
    }

    #[test]
    fn events_apply_to_the_board_and_selection() {
        let mut b = board(3);
        let mut s = Selection::default();
        assert!(!b.apply(&BoardEvent::Select(Some(CardId(2))), &mut s));
        assert!(!b.apply(&BoardEvent::Toggle(CardId(3)), &mut s));
        assert_eq!(s.ids(), [CardId(2), CardId(3)]);
        assert!(b.apply(&BoardEvent::Reorder { from: 0, before: None }, &mut s));
        assert_eq!(titles(&b), ["Card 1", "Card 2", "Card 0"]);
        assert!(b.apply(&BoardEvent::Moved { id: CardId(1), x: 400.0, y: 400.0 }, &mut s));
        assert!(b.apply(&BoardEvent::Link { from: CardId(1), to: CardId(2) }, &mut s));
        assert!(b.apply(&BoardEvent::AddAt { x: 500.0, y: 500.0 }, &mut s));
        assert_eq!(s.ids(), [CardId(4)]);
        let added = b.card(CardId(4)).unwrap();
        assert_eq!((added.x, added.y), (420.0, 440.0), "centred on the point, snapped");
        s.select(Some(CardId(1)));
        assert!(b.apply(&BoardEvent::Delete(vec![CardId(1)]), &mut s));
        assert!(s.is_empty() && b.links.is_empty());
        assert!(!b.apply(&BoardEvent::Open(CardId(2)), &mut s));
    }

    #[test]
    fn repair_brings_hostile_data_within_limits() {
        let json = r##"{"cards": [
            {"id": 1, "title": "A\nB\u0007", "color": "red", "x": -5, "w": 99999},
            {"id": 1, "title": "dup"},
            {"id": 2, "label": "Draft", "color": "#b8d4f5"}
        ], "links": [{"from": 1, "to": 2}, {"from": 1, "to": 2}, {"from": 1, "to": 9}, {"from": 2, "to": 2}], "grid": 0}"##;
        let b: Board = serde_json::from_str(json).unwrap();
        let b = b.repaired();
        assert_eq!(b.cards.len(), 2);
        assert_eq!(b.cards[0].title, "A B");
        assert_eq!(b.cards[0].color, COLORS[0]);
        assert_eq!((b.cards[0].x, b.cards[0].w), (0.0, CARD_W_RANGE.1));
        assert_eq!(b.cards[1].label, "Draft");
        assert_eq!(b.links.len(), 1);
        assert_eq!(b.grid, 5.0);
        // And a board survives a round trip.
        let again: Board = serde_json::from_str(&serde_json::to_string(&b).unwrap()).unwrap();
        assert_eq!(again, b);
    }

    #[test]
    fn text_and_colors_are_checked() {
        let mut b = board(1);
        let id = CardId(1);
        b.set_text(id, "Title\nwith a break", "Body\r\nlines").unwrap();
        assert_eq!(b.cards[0].title, "Title with a break");
        assert_eq!(b.cards[0].body, "Body\nlines");
        assert_eq!(b.set_color(id, "blue"), Err(BoardError::Color("blue".into())));
        b.set_color(id, "#b8e6d0").unwrap();
        assert_eq!(b.cards[0].color, "#B8E6D0");
        b.set_label(id, "Done").unwrap();
        assert_eq!(b.cards[0].label, "Done");
    }
}
