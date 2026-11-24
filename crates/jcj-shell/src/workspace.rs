// SPDX-License-Identifier: MIT OR Apache-2.0
//! The workspace's panels: which panel is open on each side of the document (left, right,
//! bottom), how wide or tall each side is, which panels the user moved to another side, and
//! which are pinned. The app supplies its panels ([`PanelSpec`]); the layout is saved with the
//! preferences and restored at the next start.
//!
//! One panel shows on a side at a time; its rail icon (left and right) or its title bar opens
//! and closes it. A pinned panel (the default) takes its room beside the document. An unpinned
//! one floats over the document and closes when the user goes back to the document.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// A side of the document a panel docks to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    /// Left of the document.
    #[default]
    Left,
    /// Right of the document.
    Right,
    /// Under the document.
    Bottom,
}

impl Side {
    /// Every side.
    pub const ALL: [Side; 3] = [Side::Left, Side::Right, Side::Bottom];

    /// The side's name in commands and preferences.
    pub fn key(self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Right => "right",
            Side::Bottom => "bottom",
        }
    }

    /// The side named `k`.
    pub fn from_key(k: &str) -> Option<Side> {
        Side::ALL.into_iter().find(|s| s.key() == k)
    }

    /// The side as menus name it.
    pub fn label(self) -> &'static str {
        match self {
            Side::Left => "Left",
            Side::Right => "Right",
            Side::Bottom => "Bottom",
        }
    }

    /// The range a side's size is kept within, points.
    pub fn size_range(self) -> (f32, f32) {
        match self {
            Side::Left | Side::Right => (200.0, 480.0),
            Side::Bottom => (120.0, 600.0),
        }
    }

    /// The size a side starts at, points.
    pub fn default_size(self) -> f32 {
        match self {
            Side::Left => 260.0,
            Side::Right => 290.0,
            Side::Bottom => 220.0,
        }
    }
}

/// A panel the app offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanelSpec {
    /// Its id (kept in the preferences; never change it once released).
    pub id: String,
    /// Its title.
    pub title: String,
    /// Its rail icon ([`crate::icons`]).
    pub icon: String,
    /// The side it docks to until the user moves it.
    pub side: Side,
}

impl PanelSpec {
    /// A panel docked to `side`.
    pub fn new(id: impl Into<String>, title: impl Into<String>, icon: impl Into<String>, side: Side) -> Self {
        PanelSpec { id: id.into(), title: title.into(), icon: icon.into(), side }
    }
}

/// A value for each side.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PerSide<T> {
    /// Left.
    pub left: T,
    /// Right.
    pub right: T,
    /// Bottom.
    pub bottom: T,
}

impl<T> PerSide<T> {
    /// The value for `side`.
    pub fn get(&self, side: Side) -> &T {
        match side {
            Side::Left => &self.left,
            Side::Right => &self.right,
            Side::Bottom => &self.bottom,
        }
    }

    /// The value for `side`, to change.
    pub fn get_mut(&mut self, side: Side) -> &mut T {
        match side {
            Side::Left => &mut self.left,
            Side::Right => &mut self.right,
            Side::Bottom => &mut self.bottom,
        }
    }
}

/// The panels' layout, as the preferences keep it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    /// The panel open on each side, if any.
    pub open: PerSide<Option<String>>,
    /// The panel last open on each side (what showing the side again opens).
    pub last: PerSide<Option<String>>,
    /// Each side's width (left, right) or height (bottom), points.
    pub size: PerSide<f32>,
    /// Panels the user moved from the side the app gave them.
    pub moved: BTreeMap<String, Side>,
    /// Panels the user unpinned (they float over the document).
    pub unpinned: BTreeSet<String>,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            open: PerSide::default(),
            last: PerSide::default(),
            size: PerSide { left: Side::Left.default_size(), right: Side::Right.default_size(), bottom: Side::Bottom.default_size() },
            moved: BTreeMap::new(),
            unpinned: BTreeSet::new(),
        }
    }
}

fn spec<'a>(specs: &'a [PanelSpec], id: &str) -> Option<&'a PanelSpec> {
    specs.iter().find(|p| p.id == id)
}

impl Layout {
    /// A layout with `open` panels showing (the app's first-run layout).
    pub fn with_open(specs: &[PanelSpec], open: &[&str]) -> Self {
        let mut l = Layout::default();
        for id in open {
            l.show(specs, id);
        }
        l
    }

    /// The side panel `id` docks to now.
    pub fn side_of(&self, specs: &[PanelSpec], id: &str) -> Option<Side> {
        let s = spec(specs, id)?;
        Some(self.moved.get(id).copied().unwrap_or(s.side))
    }

    /// The panels on `side`, in the app's order.
    pub fn panels_on<'a>(&self, specs: &'a [PanelSpec], side: Side) -> Vec<&'a PanelSpec> {
        specs.iter().filter(|p| self.side_of(specs, &p.id) == Some(side)).collect()
    }

    /// The panel open on `side`.
    pub fn open_on(&self, side: Side) -> Option<&str> {
        self.open.get(side).as_deref()
    }

    /// Whether panel `id` is open.
    pub fn is_open(&self, id: &str) -> bool {
        Side::ALL.iter().any(|s| self.open_on(*s) == Some(id))
    }

    /// Whether panel `id` is pinned (takes its room beside the document).
    pub fn is_pinned(&self, id: &str) -> bool {
        !self.unpinned.contains(id)
    }

    /// Open panel `id` on its side (closing the one there).
    pub fn show(&mut self, specs: &[PanelSpec], id: &str) -> bool {
        let Some(side) = self.side_of(specs, id) else { return false };
        *self.open.get_mut(side) = Some(id.to_string());
        *self.last.get_mut(side) = Some(id.to_string());
        true
    }

    /// Close panel `id` if it is open.
    pub fn hide(&mut self, id: &str) {
        for side in Side::ALL {
            if self.open_on(side) == Some(id) {
                *self.open.get_mut(side) = None;
            }
        }
    }

    /// Open panel `id`, or close it if it is open (a click on its rail icon).
    pub fn toggle(&mut self, specs: &[PanelSpec], id: &str) -> bool {
        if self.is_open(id) {
            self.hide(id);
            true
        } else {
            self.show(specs, id)
        }
    }

    /// Close `side`, or open it again with the panel last open there (else its first panel).
    pub fn toggle_side(&mut self, specs: &[PanelSpec], side: Side) -> bool {
        if self.open_on(side).is_some() {
            *self.open.get_mut(side) = None;
            return true;
        }
        let here = self.panels_on(specs, side);
        let last = self.last.get(side).clone().filter(|id| here.iter().any(|p| &p.id == id));
        match last.or_else(|| here.first().map(|p| p.id.clone())) {
            Some(id) => self.show(specs, &id),
            None => false,
        }
    }

    /// Dock panel `id` to `side` (open there if it was open).
    pub fn move_to(&mut self, specs: &[PanelSpec], id: &str, side: Side) -> bool {
        let Some(s) = spec(specs, id) else { return false };
        let was_open = self.is_open(id);
        self.hide(id);
        for other in Side::ALL {
            if self.last.get(other).as_deref() == Some(id) {
                *self.last.get_mut(other) = None;
            }
        }
        if s.side == side {
            self.moved.remove(id);
        } else {
            self.moved.insert(id.to_string(), side);
        }
        if was_open {
            self.show(specs, id);
        }
        true
    }

    /// Pin or unpin panel `id`.
    pub fn set_pinned(&mut self, id: &str, pinned: bool) {
        if pinned {
            self.unpinned.remove(id);
        } else {
            self.unpinned.insert(id.to_string());
        }
    }

    /// Set `side`'s size, kept within [`Side::size_range`].
    pub fn set_size(&mut self, side: Side, size: f32) {
        let (lo, hi) = side.size_range();
        *self.size.get_mut(side) = if size.is_finite() { size.clamp(lo, hi) } else { side.default_size() };
    }

    /// The user went back to the document: unpinned panels close.
    pub fn close_unpinned(&mut self) {
        for side in Side::ALL {
            if let Some(id) = self.open_on(side)
                && !self.is_pinned(id)
            {
                *self.open.get_mut(side) = None;
            }
        }
    }

    /// The layout brought in line with the app's panels, for a layout read from a file: panels
    /// the app no longer has are forgotten, a panel open on a side it is not on is closed, and
    /// sizes are kept within range.
    pub fn repaired(mut self, specs: &[PanelSpec]) -> Layout {
        self.moved.retain(|id, _| spec(specs, id).is_some());
        self.unpinned.retain(|id| spec(specs, id).is_some());
        for side in Side::ALL {
            for slot in [self.open.get(side).clone(), self.last.get(side).clone()] {
                if let Some(id) = slot
                    && self.side_of(specs, &id) != Some(side)
                {
                    if self.open.get(side).as_deref() == Some(id.as_str()) {
                        *self.open.get_mut(side) = None;
                    }
                    if self.last.get(side).as_deref() == Some(id.as_str()) {
                        *self.last.get_mut(side) = None;
                    }
                }
            }
            // A size left out of the file reads as zero: take the default.
            let size = *self.size.get(side);
            self.set_size(side, if size > 0.0 { size } else { side.default_size() });
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn specs() -> Vec<PanelSpec> {
        vec![
            PanelSpec::new("outline", "Outline", "outline", Side::Left),
            PanelSpec::new("find", "Find", "find", Side::Left),
            PanelSpec::new("inspector", "Inspector", "properties", Side::Right),
            PanelSpec::new("log", "Log", "draft", Side::Bottom),
        ]
    }

    #[test]
    fn one_panel_per_side_and_toggles() {
        let s = specs();
        let mut l = Layout::default();
        assert!(l.toggle(&s, "outline"));
        assert_eq!(l.open_on(Side::Left), Some("outline"));
        l.toggle(&s, "find");
        assert_eq!(l.open_on(Side::Left), Some("find"), "one panel per side");
        assert!(!l.is_open("outline"));
        l.toggle(&s, "find");
        assert_eq!(l.open_on(Side::Left), None);
        // Showing the side again opens the panel last open there.
        l.toggle_side(&s, Side::Left);
        assert_eq!(l.open_on(Side::Left), Some("find"));
        // A side never opened opens its first panel.
        l.toggle_side(&s, Side::Bottom);
        assert_eq!(l.open_on(Side::Bottom), Some("log"));
        assert!(!l.toggle(&s, "missing"));
    }

    #[test]
    fn panels_move_between_sides() {
        let s = specs();
        let mut l = Layout::with_open(&s, &["outline"]);
        assert!(l.move_to(&s, "outline", Side::Right));
        assert_eq!(l.open_on(Side::Left), None);
        assert_eq!(l.open_on(Side::Right), Some("outline"), "still open, on its new side");
        assert_eq!(l.panels_on(&s, Side::Right).iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["outline", "inspector"]);
        l.move_to(&s, "outline", Side::Left);
        assert!(l.moved.is_empty(), "back home: nothing to remember");
    }

    #[test]
    fn unpinned_panels_close_when_the_document_is_used() {
        let s = specs();
        let mut l = Layout::with_open(&s, &["outline", "inspector"]);
        l.set_pinned("inspector", false);
        assert!(!l.is_pinned("inspector"));
        l.close_unpinned();
        assert_eq!((l.open_on(Side::Left), l.open_on(Side::Right)), (Some("outline"), None));
        l.set_pinned("inspector", true);
        assert!(l.unpinned.is_empty());
    }

    #[test]
    fn layouts_save_and_restore() {
        let s = specs();
        let mut l = Layout::with_open(&s, &["find", "log"]);
        l.move_to(&s, "log", Side::Right);
        l.set_pinned("find", false);
        l.set_size(Side::Left, 333.0);
        l.set_size(Side::Bottom, 5.0);
        assert_eq!(l.size.bottom, 120.0, "kept within range");
        let text = serde_json::to_string(&l).unwrap();
        let back: Layout = serde_json::from_str(&text).unwrap();
        assert_eq!(back.repaired(&s), l);
        // A layout from a version with other panels, and damaged sizes.
        let old = r#"{"open": {"left": "gone", "right": "log"}, "moved": {"gone": "bottom"}, "unpinned": ["gone"], "size": {"left": 9999}}"#;
        let r: Layout = serde_json::from_str::<Layout>(old).unwrap().repaired(&s);
        assert_eq!(r.open_on(Side::Left), None);
        assert_eq!(r.open_on(Side::Right), None, "log is a bottom panel");
        assert!(r.moved.is_empty() && r.unpinned.is_empty());
        assert_eq!(r.size.left, 480.0);
        assert_eq!(r.size.right, 290.0, "missing: the default");
        assert_eq!(serde_json::from_str::<Layout>("{}").unwrap(), Layout::default());
    }
}
