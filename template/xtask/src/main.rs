// SPDX-License-Identifier: MIT OR Apache-2.0
//! `cargo xtask <task>` for a foundation app built on open-source-core.
//!
//! Started from the core's `template/`. This file is the app's own, and it holds only what is the
//! app's: its data (`APP`) and its own tasks (`TASKS`). The gate, every shared task and the list
//! of them are in the modules under `shared`, which the core owns and `cargo xtask sync` keeps
//! identical in every app; `cargo xtask help` lists them. A module the app adds beside this
//! file is the app's too.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod shared;

use std::process::ExitCode;

use shared::gate::{self, App, Task};

/// What the shared tasks need to know about this app. Set the fields the app has; the rest come
/// from `App::DEFAULT` (`xtask/src/gate.rs` describes each):
///
/// - `providers`: the repositories the app builds on besides the core, as
///   `&[Provider { name: "other", git: "https://github.com/<owner>/<repository>" }]`
///   (`shared::link::Provider`);
/// - `no_indexing`: packages whose roots must also deny `clippy::indexing_slicing`;
/// - `legal_files`: legal files beyond the ones every app carries;
/// - `screenshots`: the package whose `screenshots` example draws `docs/images/`.
const APP: App = App {
    // The product's name, for generated files. Change to the app's name.
    product: "This app",
    // The programs the app ships (for THIRD-PARTY-LICENSES.txt). Change to the app's binaries.
    shipped: &["app"],
    tasks: TASKS,
    ..App::DEFAULT
};

/// The app's own tasks: a name, a line for `cargo xtask help`, and a function that takes the
/// workspace's folder and the arguments after the name. `Task::check` puts a task in the gate,
/// after the shared checks and before clippy and the tests; `Task::other` is run by name only.
///
/// ```ignore
/// const TASKS: &[Task] = &[
///     Task::check("names", "no other product's name in the interface text", names::check),
///     Task::other("bundle", "build a release bundle for this computer in dist/", |root, _| bundle::bundle(root)),
/// ];
/// ```
const TASKS: &[Task] = &[];

fn main() -> ExitCode {
    gate::main(&APP)
}
