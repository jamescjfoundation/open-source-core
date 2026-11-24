// SPDX-License-Identifier: MIT OR Apache-2.0
//! `core-testbed [file]` opens the window; `core-testbed selftest` runs the headless self-test;
//! `core-testbed mcp` serves the sample document to an agent over standard input and output.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::process::ExitCode;

use core_testbed::window::Testbed;
use jcj_shell::egui;
use jcj_shell::{Shell, theme::Tokens};

struct Window {
    shell: Shell,
    app: Testbed,
}

impl eframe::App for Window {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.shell.ui(ui, &mut self.app);
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("selftest") => match core_testbed::selftest::run() {
            Ok(steps) => {
                for s in steps {
                    println!("ok  {:<12} {}", s.name, s.detail);
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("self-test FAILED: {e}");
                ExitCode::FAILURE
            }
        },
        Some("mcp") => {
            let doc = core_testbed::selftest::sample_document().unwrap_or_default();
            let info = jcj_agents::AgentInfo::new("core-testbed", "Core testbed", env!("CARGO_PKG_VERSION"), "The core's test app.");
            let mut server = jcj_agents::AgentServer::headless(jcj_engine::session(doc), jcj_agents::AgentConfig::new(info));
            match server.serve_stdio() {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("mcp: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("--help" | "-h" | "help") => {
            println!("usage: core-testbed [project file] | selftest | mcp");
            ExitCode::SUCCESS
        }
        file => {
            let path = file.map_or_else(|| std::path::PathBuf::from("untitled.jcjtest"), std::path::PathBuf::from);
            let window = Window { shell: Shell::new("Core testbed", env!("CARGO_PKG_VERSION"), Tokens::default()), app: Testbed::new(path) };
            let options = eframe::NativeOptions::default();
            match eframe::run_native("Core testbed", options, Box::new(|_cc| Ok(Box::new(window)))) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("could not open a window: {e}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}
