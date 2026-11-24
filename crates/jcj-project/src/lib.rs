// SPDX-License-Identifier: MIT OR Apache-2.0
//! The project container: one document or many, with assets, snapshots, the app's own data,
//! atomic saving, rotating backups and a recovery file.
//!
//! A [`Project`] holds named documents ([`jcj_doc::Document`], stored as WordCraft's native JSON
//! so every app's extension data survives), media by key ([assets](Project::add_asset)), named
//! and dated copies of documents ([snapshots](Project::take_snapshot)), and data the app keeps
//! for the whole project ([`Project::app_data`]). On disk it is one zip file; `FORMAT.md` beside
//! this crate is the specification. The file extension is the app's choice.
//!
//! ```
//! use jcj_project::{App, Project};
//!
//! const APP: App = App::new("org.example.notes", 1);
//!
//! let mut project = Project::new(&APP);
//! let id = project.add_document("Chapter One", jcj_doc::new_document()).unwrap();
//! let bytes = project.to_bytes().unwrap();
//! let back = Project::from_bytes(&bytes, &APP).unwrap();
//! assert_eq!(back.name(id), Some("Chapter One"));
//! ```
//!
//! Each app names itself with an [`App`]: an id and the version of the app's own data. A
//! project made by an older version of the app is passed to the app's migration hook when it is
//! opened ([`App::with_migration`]); one made by a newer version, or by another app, is refused
//! with a clear error.
//!
//! Reading never trusts the file: its size, the number of zip entries, each entry's size, the
//! total read and the number of documents, assets and snapshots are bounded ([`Limits`]), and
//! damage is an [`Error`], never a panic.
//!
//! On disk ([`files`]): saves go through craft-atomic, so a failed or interrupted save leaves the
//! old file whole; [`files::backup`] keeps rotating, dated copies; [`files::write_recovery`] keeps
//! a recovery copy next to the file between saves; [`AutoSave`] says when to write one.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod autosave;
mod container;
pub mod files;
mod project;

pub use autosave::AutoSave;
pub use container::{CONTAINER_VERSION, Limits, MIMETYPE, Summary};
pub use project::{App, DocId, MAX_KEY_LEN, MAX_NAME_LEN, MigrateFn, Project, Snapshot, SnapshotId, valid_key};

use std::path::PathBuf;

/// Why a project could not be read, changed or written.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The bytes are not a project file (not a zip, or no project `mimetype` entry).
    #[error("this is not a project file")]
    NotAProject,
    /// Something is over a [`Limits`] bound: the file, an entry, or a count.
    #[error("the project is too large: {0}")]
    TooLarge(String),
    /// The container was written by a newer version of the core.
    #[error("the project was saved in a newer file format (container version {0}); this version reads up to {CONTAINER_VERSION}")]
    NewerContainer(u32),
    /// The project belongs to another app.
    #[error("the project was made by another app ({found})")]
    OtherApp { found: String },
    /// The project was saved by a newer version of the app.
    #[error("the project was saved by a newer version of the app (data version {found}); this version reads up to {supported}")]
    NewerApp { found: u32, supported: u32 },
    /// The file is a project, but a part of it is missing or unreadable.
    #[error("the project is damaged: {0}")]
    Damaged(String),
    /// The app's migration hook refused the project.
    #[error("the project could not be brought up to date: {0}")]
    Migration(String),
    /// A request the project cannot carry out: an unknown document, a bad name or key.
    #[error("{0}")]
    Invalid(String),
    /// The automation file gateway refused a path.
    #[error("{0}")]
    Refused(String),
    /// Reading or writing a file failed.
    #[error("{}: {source}", path.display())]
    Io { path: PathBuf, source: std::io::Error },
}

/// The result of project operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;
