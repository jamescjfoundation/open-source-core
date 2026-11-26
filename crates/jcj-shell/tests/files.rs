// SPDX-License-Identifier: MIT OR Apache-2.0
//! The shell's file dialogs (core issue #28), driven without a window: a stand-in answers in the
//! system's place, and each answer runs the command the app asked with.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use egui::vec2;
use egui_kittest::Harness;
use jcj_shell::commands::ids;
use jcj_shell::files::{FileAnswer, FileAsk, FileKind};
use jcj_shell::{App, Cmd, CommandInfo, Commands, Shell};
use serde_json::json;

/// An app that keeps the commands it was asked to run.
#[derive(Default)]
struct Recorder {
    ran: Vec<Cmd>,
}

impl Commands for Recorder {
    fn command(&self, _: &str) -> Option<CommandInfo> {
        None
    }
}

impl App for Recorder {
    fn document_name(&self) -> String {
        "Untitled".into()
    }

    fn run(&mut self, cmd: &Cmd, shell: &mut Shell) -> Result<(), String> {
        // "Save As…" as an app writes it: ask where, and come back with the answer.
        if cmd.id == ids::SAVE_AS && cmd.params.get("path").is_none() {
            shell.ask_file(FileAsk::save().title("Save As").filter("Text", &["txt"]).file_name("notes.txt"), Cmd::new(ids::SAVE_AS));
            return Ok(());
        }
        self.ran.push(cmd.clone());
        Ok(())
    }

    fn document_ui(&mut self, ui: &mut egui::Ui, _shell: &mut Shell) {
        ui.label("The document");
    }
}

type State = (Shell, Recorder);

fn harness() -> Harness<'static, State> {
    let shell = Shell::new("Sample", "1.2.3", jcj_shell::theme::Tokens::default());
    Harness::builder().with_size(vec2(900.0, 600.0)).with_step_dt(0.05).build_ui_state(
        |ui, s: &mut State| {
            let (shell, app) = s;
            shell.ui(ui, app);
        },
        (shell, Recorder::default()),
    )
}

/// Answer every dialog by `answer`, keeping what was asked.
fn stand_in(shell: &mut Shell, answer: impl Fn(&FileAsk) -> FileAnswer + 'static) -> Rc<RefCell<Vec<FileAsk>>> {
    let asked = Rc::new(RefCell::new(Vec::new()));
    let log = asked.clone();
    shell.answer_file_dialogs_with(Some(Box::new(move |ask| {
        log.borrow_mut().push(ask.clone());
        answer(ask)
    })));
    asked
}

#[test]
fn the_answer_runs_the_command_with_the_path_chosen() {
    let mut h = harness();
    let asked = stand_in(&mut h.state_mut().0, |ask| match ask.kind {
        FileKind::Save => FileAnswer::Chosen(vec![PathBuf::from("drafts/notes.txt")]),
        FileKind::OpenMany => FileAnswer::Chosen(vec![PathBuf::from("a.md"), PathBuf::from("b.md")]),
        FileKind::Folder => FileAnswer::Chosen(vec![PathBuf::from("exports")]),
        FileKind::Open => FileAnswer::Cancelled,
    });
    // Asked before the first frame, and from inside a command: both are answered.
    let shell = &mut h.state_mut().0;
    assert!(shell.ask_file(FileAsk::open_many().filter("Markdown", &["md"]), Cmd::with("file.import", json!({"as": "notes"}))));
    assert!(shell.ask_file(FileAsk::folder().title("Export to"), Cmd::new("file.exportTo")));
    assert!(shell.ask_file(FileAsk::open(), Cmd::new(ids::OPEN_PATH)), "closed without choosing: runs nothing");
    assert!(shell.ask_file_or(FileAsk::open(), Cmd::new(ids::OPEN_PATH), Cmd::new("file.openCancelled")));
    assert!(shell.file_dialog_pending());
    shell.run(Cmd::new(ids::SAVE_AS));
    h.run();
    h.run();
    let (shell, app) = h.state();
    assert_eq!(
        app.ran,
        [
            Cmd::with("file.import", json!({"as": "notes", "path": "a.md", "paths": ["a.md", "b.md"]})),
            Cmd::with("file.exportTo", json!({"path": "exports", "paths": ["exports"]})),
            Cmd::new("file.openCancelled"),
            Cmd::with(ids::SAVE_AS, json!({"path": "drafts/notes.txt", "paths": ["drafts/notes.txt"]})),
        ],
        "in the order asked"
    );
    assert!(!shell.file_dialog_pending());
    assert!(shell.message.is_empty(), "{}", shell.message);
    let asked = asked.borrow();
    assert_eq!(asked.len(), 5);
    assert_eq!(asked[4], FileAsk::save().title("Save As").filter("Text", &["txt"]).file_name("notes.txt"), "the stand-in sees what was asked");
}

#[test]
fn a_dialog_that_cannot_be_shown_is_said_in_the_status_bar() {
    let mut h = harness();
    stand_in(&mut h.state_mut().0, |_| FileAnswer::Unavailable("no dialog service".into()));
    h.run();
    h.state_mut().0.ask_file(FileAsk::save(), Cmd::new(ids::SAVE_AS));
    h.run();
    let (shell, app) = h.state();
    assert!(app.ran.is_empty());
    assert_eq!(shell.message, "The file dialog could not be shown: no dialog service");
}

#[test]
fn without_a_window_or_a_stand_in_a_dialog_waits() {
    let mut h = harness();
    h.state_mut().0.ask_file(FileAsk::open(), Cmd::new(ids::OPEN_PATH));
    h.run();
    let (shell, app) = h.state();
    assert!(shell.file_dialog_pending(), "it shows when the window calls show_file_dialogs");
    assert!(app.ran.is_empty());
    // Only so many wait; the rest are refused.
    let shell = &mut h.state_mut().0;
    let taken = (0..2 * jcj_shell::files::MAX_WAITING).filter(|_| shell.ask_file(FileAsk::open(), Cmd::new(ids::OPEN_PATH))).count();
    assert_eq!(taken, jcj_shell::files::MAX_WAITING - 1);
}
