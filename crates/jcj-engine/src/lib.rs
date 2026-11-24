// SPDX-License-Identifier: MIT OR Apache-2.0
//! Editing sessions, undo and redo, the command registry and recovery.
//!
//! Apps name engine types through this crate, never through WordCraft (decision 0005). The
//! engine is WordCraft's `wordcraft-engine` at the pinned release:
//!
//! - a [`Session`] holds one or more open documents, the selection, undo and redo history and
//!   the app-wide state (clipboard, preferences);
//! - every action is a command: a [`CommandSpec`] with an id (`app.action`), a label, where it
//!   lives in the interface, its parameters and a `run` function. [`Session::run`] is the single
//!   entry point the interface, the command line and agents use. A command that `mutates` is one
//!   undo step, and a command that fails leaves the document as it was;
//! - an app registers its own commands beside the engine's with [`registry_with`] and starts
//!   sessions on that registry with [`session_with`].
//!
//! **We use the full upstream engine.** WordCraft's engine carries about 400 word-processor
//! commands (tables, mail merge, equations, review...). A slimmer engine (session, undo, registry
//! and recovery without that catalog) is a patch we have proposed but do not carry yet
//! (SUITE_ARCHITECTURE.md, "An engine without the Word command catalog"). Until then every
//! registry holds the engine's commands too; apps show only their own in their interface, and an
//! app command with the id of an engine command replaces it.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod commands;
pub mod native;
pub mod recovery;

use std::collections::HashSet;
use std::sync::Arc;

use jcj_doc::Document;
use jcj_upstream::engine as up;

// The engine types apps hold. Each name is part of the core's interface.
pub use up::{CmdError, CmdResult, CommandSpec, Registry, Selection, Session};

/// Helpers for writing commands: results and positions as JSON, the time, inserting text.
pub mod cmd {
    pub use jcj_upstream::engine::cmd::{civil_from_days, now_unix, parse_pos, pos_json, sel_result};

    /// Insert `text` at the caret, replacing the selection, as typing does: with the formatting
    /// waiting at the caret, a tracked change while changes are tracked, a line break (`\n`) as
    /// a new paragraph.
    pub fn type_text(s: &mut crate::Session, text: &str) -> Result<(), crate::CmdError> {
        jcj_upstream::engine::cmd::type_text(s, text)
    }

    /// [`type_text`] for text that was not typed (pasted, replaced): no typing rules such as
    /// ending a list on an empty paragraph.
    pub fn paste_text(s: &mut crate::Session, text: &str) -> Result<(), crate::CmdError> {
        jcj_upstream::engine::cmd::paste_text(s, text)
    }
}

/// Reading command parameters (`params::str(v, "name")`, `params::bool`, ...).
pub mod params {
    pub use jcj_upstream::engine::p::*;
}

/// Confining the files commands read and write to chosen folders (agents, the command line).
/// `current` says which folders are in force (a command that hands a file to another program,
/// such as printing, refuses while they are); `unconfined` runs code with none.
pub mod file_access {
    pub use jcj_upstream::engine::file_access::{AutomationRoots, check_read, check_write, confine, confine_process, current, unconfined};
}

/// The engine's own commands (the full upstream catalog), in registry order.
pub fn engine_specs() -> Vec<CommandSpec> {
    up::cmd::registry().all().to_vec()
}

/// The ids that more than one of `specs` has, each once, in order.
pub fn duplicate_ids(specs: &[CommandSpec]) -> Vec<&'static str> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for s in specs {
        if !seen.insert(s.id) && !out.contains(&s.id) {
            out.push(s.id);
        }
    }
    out
}

/// A registry of the engine's commands followed by the app's. An app command with the id of an
/// engine command replaces it. Two app commands with the same id are a mistake: the later wins
/// here; [`try_registry_with`] reports them instead.
pub fn registry_with(app: Vec<CommandSpec>) -> Registry {
    let mut all: Vec<CommandSpec> = engine_specs().into_iter().filter(|s| !app.iter().any(|m| m.id == s.id)).collect();
    all.extend(app);
    Registry::new(all)
}

/// [`registry_with`], or the app command ids registered more than once.
pub fn try_registry_with(app: Vec<CommandSpec>) -> Result<Registry, Vec<&'static str>> {
    let dupes = duplicate_ids(&app);
    if dupes.is_empty() { Ok(registry_with(app)) } else { Err(dupes) }
}

/// A session on `doc` with `registry` (share one registry between sessions with the `Arc`).
pub fn session_with(doc: Document, registry: Arc<Registry>) -> Session {
    let mut s = Session::new(doc);
    s.registry = registry;
    s
}

/// A session on `doc` with the engine's commands only.
pub fn session(doc: Document) -> Session {
    Session::new(doc)
}

/// The commands of `registry` whose ids start with `prefix` (an app's namespace, `"notes."`).
pub fn commands_in<'a>(registry: &'a Registry, prefix: &str) -> Vec<&'a CommandSpec> {
    registry.all().iter().filter(|s| s.id.starts_with(prefix)).collect()
}

#[cfg(test)]
mod tests;
