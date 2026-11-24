// SPDX-License-Identifier: MIT OR Apache-2.0
//! Recovery of unsaved work after a crash (desktop only).
//!
//! A desktop app turns it on once per session with [`enable`], giving its profile folder and a
//! function that wakes its event loop. The session then keeps a copy of each open document with
//! unsaved changes in that folder, written on a worker thread outside typing, and removes it when
//! the document is saved or closed. On the next start [`documents`] lists the copies left behind.
//! Headless sessions (agents, the command line, tests) never write recovery copies unless enabled.
//!
//! Each frame the app calls [`tick`]; [`wait`] says how long it may sleep before the next tick.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::Session;

/// A copy of a document left by an earlier run.
pub use jcj_upstream::engine::recovery::RecoveryInfo;

#[cfg(not(target_arch = "wasm32"))]
/// Keep recovery copies of `session`'s documents in `folder`; `wake` is called from the worker
/// when the app should tick.
pub fn enable(session: &mut Session, folder: PathBuf, wake: Arc<dyn Fn() + Send + Sync>) -> Result<(), String> {
    session.enable_recovery(folder, wake)
}

/// The copies left by earlier runs, newest first (not those of documents open now).
pub fn documents(session: &Session) -> Vec<RecoveryInfo> {
    session.recovery_documents()
}

/// Take in the worker's results and queue the next write. Call once per frame.
pub fn tick(session: &mut Session) {
    session.recovery_tick();
}

/// How long the app may wait before the next [`tick`]; `None`: until woken.
pub fn wait(session: &Session) -> Option<Duration> {
    session.recovery_wait()
}

/// Whether recovery has read the folder and is ready, and whether it is busy writing.
pub fn status(session: &Session) -> (bool, bool) {
    (session.recovery_ready(), session.recovery_busy())
}

/// The last error recovery met, if any (a folder it cannot write).
pub fn error(session: &Session) -> Option<&str> {
    session.recovery_error()
}

/// Write pending copies and stop (the app is quitting normally).
pub fn finish(session: &mut Session) {
    session.finish_recovery();
}

/// Remove the recovery copy of the active document (it was saved, or the user discarded it).
pub fn discard_current(session: &mut Session) {
    session.discard_recovery_copy();
}
