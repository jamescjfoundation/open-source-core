// SPDX-License-Identifier: MIT OR Apache-2.0
//! `core-testbed [file]` opens the window; `core-testbed selftest` runs the headless self-test;
//! `core-testbed verify <a> <b>` checks two files word for word; `core-testbed mcp` serves a
//! document and the board to an agent over standard input and output.
//!
//! Its command line follows the core's convention (decision 0017,
//! `docs/command-lines-and-agent-tools.md`), with one difference a desktop program has: with no
//! argument it opens its window.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::process::ExitCode;

use core_testbed::window::Testbed;
use jcj_agents::cli::{Exit, McpArgs, Standard, standard, version_line};
use jcj_shell::egui;
use jcj_shell::{Shell, theme::Tokens};

struct Window {
    shell: Shell,
    /// The window's part of the shell's file dialogs.
    files: jcj_shell::files::FileDialogs,
    app: Testbed,
}

impl eframe::App for Window {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.shell.ui(ui, &mut self.app);
        // Show the next file dialog the shell was asked for, over this window, once the frame
        // is drawn; an answer runs its command in a later frame.
        self.files.show(&mut self.shell, ui.ctx(), Some(&*frame));
    }
}

const PROGRAM: &str = "core-testbed";

fn usage() -> String {
    format!(
        "usage:\n  {PROGRAM} [<project file>]\n  {PROGRAM} selftest\n  {PROGRAM} verify <source> <result>\n  {PROGRAM} {}\n  {PROGRAM} --help | --version",
        McpArgs::USAGE
    )
}

/// The document in `path`: a testbed project's main document, or a file in a format the core
/// reads.
fn load(path: &str) -> Result<jcj_doc::Document, String> {
    let file = std::path::Path::new(path);
    if file.extension().is_some_and(|e| e == core_testbed::window::EXTENSION) {
        let project = jcj_project::files::open(file, &core_testbed::selftest::APP).map_err(|e| format!("{path}: {e}"))?;
        return project.find_document("Main").and_then(|id| project.document(id)).cloned().ok_or_else(|| format!("{path}: no main document"));
    }
    let bytes = std::fs::read(file).map_err(|e| format!("{path}: {e}"))?;
    let format = jcj_interchange::Format::detect(Some(path), &bytes).ok_or_else(|| format!("{path}: not a format the testbed reads"))?;
    jcj_interchange::import(format, &bytes).map_err(|e| format!("{path}: {e}"))
}

/// `verify <source> <result>`: the two files' words, compared in order.
fn verify(args: &[String]) -> Result<Exit, (Exit, String)> {
    let [source, result] = args else { return Err((Exit::Usage, "verify takes a source and a result".into())) };
    let failed = |e: String| (Exit::Failed, e);
    let (source, result) = (load(source).map_err(failed)?, load(result).map_err(failed)?);
    let check = jcj_doc::verify::Check::of(&jcj_doc::verify::document_words(&source), &jcj_doc::verify::document_words(&result));
    println!("{check}");
    // The check ran either way: a difference is its answer, not a failure.
    Ok(if check.passed() { Exit::Done } else { Exit::Differs })
}

/// `mcp [<file>] [--read-root <folder>] [--write-root <folder>]`.
fn mcp(args: &[String]) -> Result<Exit, (Exit, String)> {
    let args = McpArgs::parse(args).map_err(|e| (Exit::Usage, e))?;
    // The file is the one the person named: it is opened before the agent's folders hold.
    let doc = match &args.file {
        Some(path) => load(path).map_err(|e| (Exit::Failed, e))?,
        None => core_testbed::selftest::sample_document().unwrap_or_default(),
    };
    let roots = args.confine(PROGRAM).map_err(|e| (Exit::Usage, e))?;
    core_testbed::agents::server(doc, roots).serve_stdio().map_err(|e| (Exit::Failed, format!("mcp: {e}")))?;
    Ok(Exit::Done)
}

fn selftest() -> Result<Exit, (Exit, String)> {
    for s in core_testbed::selftest::run().map_err(|e| (Exit::Failed, format!("self-test FAILED: {e}")))? {
        println!("ok  {:<12} {}", s.name, s.detail);
    }
    Ok(Exit::Done)
}

fn window(file: Option<&str>) -> Result<Exit, (Exit, String)> {
    // The interface language, when asked for (English otherwise).
    if let Ok(code) = std::env::var("CORE_TESTBED_LANG") {
        core_testbed::languages::set(&code);
    }
    let path = file.map_or_else(|| std::path::PathBuf::from("untitled.jcjtest"), std::path::PathBuf::from);
    let window = Window {
        shell: Shell::new("Core testbed", env!("CARGO_PKG_VERSION"), Tokens::default())
            .with_options(jcj_shell::ShellOptions { start_window: true, ..jcj_shell::ShellOptions::default() }),
        files: jcj_shell::files::FileDialogs::new(),
        app: Testbed::new(path),
    };
    let options = eframe::NativeOptions::default();
    eframe::run_native("Core testbed", options, Box::new(|_cc| Ok(Box::new(window))))
        .map_err(|e| (Exit::Failed, format!("could not open a window: {e}")))?;
    Ok(Exit::Done)
}

/// Where an app's releases come from. The testbed publishes none and never checks for one; it
/// names a source so that the updater's entry point at start is used here as an app uses it.
const RELEASES: jcj_shell::updates::desktop::ReleaseSource = jcj_shell::updates::desktop::ReleaseSource {
    id: PROGRAM,
    name: "Core testbed",
    tagline: "Exercises every crate of the core",
    repo: "jamescjfoundation/open-source-core",
    developer: "the open-source-core contributors",
    version: env!("CARGO_PKG_VERSION"),
    installs_in_place: false,
};

fn main() -> ExitCode {
    // First, before the program reads its own arguments: a copy of it that the updater started
    // to finish an update does that work and nothing else (Windows; `None` everywhere else).
    let started_with: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if let Some(done) = RELEASES.finish_update(&started_with) {
        return if done.is_ok() { ExitCode::SUCCESS } else { ExitCode::FAILURE };
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or_default();
    let outcome = match (standard(&args), args.first().map(String::as_str)) {
        (Some(Standard::Help), _) => {
            println!("{}", usage());
            Ok(Exit::Done)
        }
        (Some(Standard::Version), _) => {
            println!("{}", version_line(PROGRAM, env!("CARGO_PKG_VERSION")));
            Ok(Exit::Done)
        }
        // A desktop program: with nothing asked, its window.
        (Some(Standard::NoCommand), _) => window(None),
        (None, Some("selftest")) => selftest(),
        (None, Some("verify")) => verify(rest),
        (None, Some("mcp")) => mcp(rest),
        (None, Some(option)) if option.starts_with('-') => Err((Exit::Usage, format!("unknown option {option}"))),
        (None, file) => window(file),
    };
    match outcome {
        Ok(exit) => exit.into(),
        Err((exit, why)) => {
            // Results go to standard output; what went wrong, to standard error.
            eprintln!("{PROGRAM}: {why}");
            if exit == Exit::Usage {
                eprintln!("{}", usage());
            }
            exit.into()
        }
    }
}
