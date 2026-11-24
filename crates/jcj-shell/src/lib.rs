// SPDX-License-Identifier: MIT OR Apache-2.0
//! The app shell: the window around an app's document.
//!
//! An app builds a [`Shell`] with its name, version and design tokens (`jcj-theme`), implements
//! [`App`] (its document view, its ribbon tabs and panels, its commands), and calls
//! [`Shell::ui`] every frame. The shell draws and runs the rest, the same in every app:
//!
//! - a quick-access row and a ribbon whose tabs and groups the app supplies, with groups that
//!   shrink and fold to fit the window ([`ribbon_fit`]) and key tips (F10);
//! - side panels docked left, right or at the bottom, pinned or floating, shown and hidden from
//!   rails, moved between sides, with the layout saved ([`workspace`]);
//! - the File page (New, Open, Recent, Save, Save As, Export, Print, Preferences, Keys, About,
//!   and the app's own pages) ([`backstage`]);
//! - the command palette, "save changes first?" and crash recovery ([`dialogs`]);
//! - preferences kept as JSON in the app's settings folder, the shell's and the app's own
//!   ([`prefs`]); the user's own keys ([`keys`]);
//! - auto-save's schedule, with the app's save ([`autosave`]); updates from the app's GitHub
//!   releases ([`updates`]);
//! - generic icons ([`icons`]) and an offscreen renderer for pictures and visual checks
//!   ([`snapshot`]).
//!
//! Every control runs a command by id. The shell's own commands start with `shell.`; the rest go
//! to [`App::run`] once the frame is drawn.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod app;
pub mod autosave;
pub mod backstage;
mod chrome;
pub mod commands;
pub mod dialogs;
pub mod icons;
pub mod keys;
pub mod prefs;
pub mod ribbon;
pub mod ribbon_fit;
mod shell;
mod sidebar;
pub mod snapshot;
pub mod split;
pub mod start;
pub mod updates;
pub mod workspace;
pub mod writing;

pub use app::{About, App};
pub use backstage::{Page, PageSpec};
pub use commands::{Cmd, CommandInfo, Commands};
pub use ribbon::TabSpec;
pub use ribbon::widgets::{self, Control};
pub use shell::{Shell, ShellOptions};
pub use sidebar::RAIL_W;
pub use start::TemplateSpec;
pub use workspace::{Layout, PanelSpec, Side};

/// egui, at the version the core pins, so apps draw with the same one the shell uses.
pub use egui;
/// The theme crate, for apps that build a shell.
pub use jcj_theme as theme;
