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
use shared::package::Package;

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
    // The product's name, for generated files and for the release packages. Change to the app's name.
    product: "This app",
    // The programs the app ships, as the packages that build them, the desktop program first
    // (for THIRD-PARTY-LICENSES.txt and the release packages). Change to the app's.
    shipped: &["app"],
    package: Some(PACKAGE),
    tasks: TASKS,
    ..App::DEFAULT
};

/// What a release of the app carries besides its programs (`cargo xtask package`; the core's
/// `docs/app-releases.md`). Set the fields the app has; the rest come from `Package::DEFAULT`
/// (`shared::package::Package` describes each):
///
/// - `file_types`: the kinds of file the app opens, as
///   `&[FileType { name: "Example document", extensions: &["exd"], mime: "application/x-example", ..FileType::DEFAULT }]`
///   (`shared::package::FileType`);
/// - `license_files`: the license texts of what the programs embed (fonts, data);
/// - `macos_icons`, `windows_icon`: icons drawn apart for those systems.
const PACKAGE: Package = Package {
    // The release id: the stem of every release file and the name of the desktop program, which
    // is also the id the app gives its update source. Change to the app's.
    id: "app",
    // The app's id on a desktop: the one the window is given (`with_app_id`). Change to the app's.
    app_id: "org.example.app",
    summary: "A foundation app",
    // A placeholder. Replace it with the app's icon, in the sizes it has.
    icons: &[(256, "assets/icon-256.png")],
    ..Package::DEFAULT
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
