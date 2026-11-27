// SPDX-License-Identifier: MIT OR Apache-2.0
//! The system's file dialogs: choose a file to open, a place to save, a folder.
//!
//! An app asks with [`Shell::ask_file`](crate::Shell::ask_file), giving the dialog ([`FileAsk`])
//! and the command to run with the answer. Nothing waits: the dialog opens after the frame is
//! drawn, the window keeps drawing while it is up, and when the user has chosen, the command runs
//! with the choice in its parameters (`"path"`, and `"paths"` for every file chosen). Closing
//! the dialog without choosing runs nothing, unless the app named a command for that too
//! ([`Shell::ask_file_or`](crate::Shell::ask_file_or)). When no dialog can be shown, the status
//! bar says why and that command runs as well, with the reason in its parameters
//! (`"unavailable"`, read with [`unavailable`]). One dialog shows at a time; later ones wait
//! their turn.
//!
//! ```no_run
//! # use jcj_shell::{Cmd, Shell, commands::ids, files::FileAsk};
//! # fn open(shell: &mut Shell) {
//! // "Open…": the answer runs `file.openPath` with `{"path": "<the file chosen>"}`.
//! shell.ask_file(FileAsk::open().title("Open").filter("Text", &["txt", "md"]), Cmd::new(ids::OPEN_PATH));
//! # }
//! ```
//!
//! **The window's part.** A dialog belongs to a window, and the shell draws inside one without
//! owning it. So the app's window keeps a [`FileDialogs`] beside its shell and calls
//! [`FileDialogs::show`] once at the end of every frame, after
//! [`Shell::ui`](crate::Shell::ui), with the window itself:
//!
//! ```ignore
//! struct Window {
//!     shell: Shell,
//!     files: jcj_shell::files::FileDialogs,
//!     app: MyApp,
//! }
//!
//! impl eframe::App for Window {
//!     fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
//!         self.shell.ui(ui, &mut self.app);
//!         self.files.show(&mut self.shell, ui.ctx(), Some(&*frame));
//!     }
//! }
//! ```
//!
//! Without that call a dialog asked for never opens. The dialogs live with the window, not in
//! the shell, because the system's dialog on screen belongs to the thread that showed it, and a
//! shell stays a value that can be moved between threads.
//!
//! **Tests and automation** answer in the system's place with [`FileDialogs::answer_with`] and
//! call [`FileDialogs::tick`], which needs no window; the answers run as commands like real
//! ones.
//!
//! The dialogs are the upstream layer's, which shows them without stopping the window's event
//! loop on any platform (a dialog that blocks it can crash the app on macOS).

use std::path::PathBuf;

use jcj_upstream::dialogs as up;
use raw_window_handle::{DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle};
use serde_json::{Value, json};

use jcj_i18n::tr_with;

use crate::Shell;
use crate::commands::Cmd;

/// Most file dialogs a shell holds for its window to show; one asked beyond that is dropped.
pub const MAX_WAITING: usize = 32;

/// Which dialog to show.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FileKind {
    /// Choose one file that exists.
    #[default]
    Open,
    /// Choose one or more files that exist.
    OpenMany,
    /// Choose where to write a file.
    Save,
    /// Choose a folder.
    Folder,
}

/// A file dialog to show.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileAsk {
    /// Which dialog.
    pub kind: FileKind,
    /// The dialog's title (its message line on macOS); empty for the system's own.
    pub title: String,
    /// The file types offered: a name as the dialog lists it, and its extensions without the dot.
    /// Extensions match in any letter case.
    pub filters: Vec<(String, Vec<String>)>,
    /// The folder the dialog starts in.
    pub folder: Option<PathBuf>,
    /// The file name a Save dialog suggests.
    pub file_name: Option<String>,
}

impl FileAsk {
    /// Choose one file that exists.
    pub fn open() -> Self {
        FileAsk { kind: FileKind::Open, ..FileAsk::default() }
    }

    /// Choose one or more files that exist.
    pub fn open_many() -> Self {
        FileAsk { kind: FileKind::OpenMany, ..FileAsk::default() }
    }

    /// Choose where to write a file.
    pub fn save() -> Self {
        FileAsk { kind: FileKind::Save, ..FileAsk::default() }
    }

    /// Choose a folder.
    pub fn folder() -> Self {
        FileAsk { kind: FileKind::Folder, ..FileAsk::default() }
    }

    /// With this title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Offering this file type too: `name` as the dialog lists it, `extensions` without the dot.
    pub fn filter(mut self, name: impl Into<String>, extensions: &[&str]) -> Self {
        self.filters.push((name.into(), extensions.iter().map(|e| e.trim_start_matches('.').to_owned()).collect()));
        self
    }

    /// Starting in this folder.
    pub fn in_folder(mut self, folder: impl Into<PathBuf>) -> Self {
        self.folder = Some(folder.into());
        self
    }

    /// Suggesting this file name (a Save dialog).
    pub fn file_name(mut self, name: impl Into<String>) -> Self {
        self.file_name = Some(name.into());
        self
    }

    fn to_upstream(&self) -> up::FileDialog {
        let mut d = up::FileDialog::new(match self.kind {
            FileKind::Open => up::Kind::Open { multiple: false },
            FileKind::OpenMany => up::Kind::Open { multiple: true },
            FileKind::Save => up::Kind::Save,
            FileKind::Folder => up::Kind::Folder,
        });
        if !self.title.trim().is_empty() {
            d = d.title(self.title.trim());
        }
        for (name, extensions) in &self.filters {
            let extensions: Vec<&str> = extensions.iter().map(String::as_str).collect();
            d = d.filter(name.as_str(), &extensions);
        }
        if let Some(folder) = &self.folder {
            d = d.directory(folder.clone());
        }
        if let Some(name) = &self.file_name {
            d = d.file_name(name.as_str());
        }
        d
    }

    fn from_upstream(d: &up::FileDialog) -> Self {
        FileAsk {
            kind: match d.kind {
                up::Kind::Open { multiple: false } => FileKind::Open,
                up::Kind::Open { multiple: true } => FileKind::OpenMany,
                up::Kind::Save => FileKind::Save,
                up::Kind::Folder => FileKind::Folder,
            },
            title: d.title.clone().unwrap_or_default(),
            filters: d.filters.iter().map(|f| (f.name.clone(), f.extensions.clone())).collect(),
            folder: d.directory.clone(),
            file_name: d.file_name.clone(),
        }
    }
}

/// How a file dialog ended: what a stand-in for the system's dialogs answers
/// ([`FileDialogs::answer_with`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileAnswer {
    /// Closed without choosing.
    Cancelled,
    /// The files, the folder or the place to save chosen.
    Chosen(Vec<PathBuf>),
    /// No dialog could be shown; the text says why.
    Unavailable(String),
}

/// A stand-in for the system's dialogs.
pub type AnswerWith = Box<dyn FnMut(&FileAsk) -> FileAnswer>;

/// A dialog the shell was asked for, and what its answer runs.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Asked {
    pub(crate) ask: FileAsk,
    pub(crate) then: Then,
}

/// What an answer runs.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Then {
    pub(crate) chosen: Cmd,
    pub(crate) cancelled: Option<Cmd>,
}

/// What an answered dialog asks the shell to do.
#[derive(Clone, Debug, PartialEq)]
enum Outcome {
    /// Run this command.
    Run(Cmd),
    /// Say this in the status bar: no dialog could be shown. Then run the command for a dialog
    /// closed without choosing, if there is one, with the reason in its parameters.
    Unavailable(String, Option<Cmd>),
}

/// The parameter that holds why no dialog could be shown, in the command run for a dialog
/// closed without choosing ([`Shell::ask_file_or`](crate::Shell::ask_file_or)).
pub const UNAVAILABLE: &str = "unavailable";

/// Why no file dialog could be shown, when `cmd` was run for that and not because the user
/// closed the dialog without choosing; `None` when it was closed. An app that keeps a step
/// waiting on the answer gives it up either way; one that has another way to ask (a typed path,
/// a default folder) can offer it only when this is `Some`.
pub fn unavailable(cmd: &Cmd) -> Option<&str> {
    cmd.params.get(UNAVAILABLE).and_then(Value::as_str)
}

/// `cancelled` with `why` in its parameters ([`UNAVAILABLE`]). Parameters it already has are
/// kept; ones that are not an object are replaced.
fn with_unavailable(mut cancelled: Cmd, why: &str) -> Cmd {
    if !cancelled.params.is_object() {
        cancelled.params = json!({});
    }
    if let Some(params) = cancelled.params.as_object_mut() {
        params.insert(UNAVAILABLE.into(), Value::String(why.to_owned()));
    }
    cancelled
}

/// `then` with the paths chosen in its parameters: `path` (the first) and `paths` (all), as
/// text. Parameters it already has are kept; ones that are not an object are replaced.
pub fn with_paths(mut then: Cmd, paths: &[PathBuf]) -> Cmd {
    let text: Vec<String> = paths.iter().map(|p| p.to_string_lossy().into_owned()).collect();
    if !then.params.is_object() {
        then.params = json!({});
    }
    if let Some(params) = then.params.as_object_mut() {
        if let Some(first) = text.first() {
            params.insert("path".into(), Value::String(first.clone()));
        }
        params.insert("paths".into(), json!(text));
    }
    then
}

/// No window: the parent of a dialog that a stand-in answers.
struct NoWindow;

impl HasWindowHandle for NoWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Err(HandleError::Unavailable)
    }
}

impl HasDisplayHandle for NoWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Err(HandleError::Unavailable)
    }
}

/// The file dialogs of one window: the one on screen, those waiting, the answers not yet run.
/// The app's window keeps one beside its [`Shell`] and calls [`FileDialogs::show`] every frame.
#[derive(Default)]
pub struct FileDialogs {
    dialogs: Option<up::Dialogs<Then>>,
    /// A stand-in answers instead of the system.
    stand_in: bool,
    /// A stand-in set before the first frame.
    early_stand_in: Option<AnswerWith>,
}

impl std::fmt::Debug for FileDialogs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileDialogs").field("waiting", &self.waiting()).field("open", &self.is_open()).field("stand_in", &self.stand_in).finish()
    }
}

impl FileDialogs {
    /// No dialog yet.
    pub fn new() -> Self {
        FileDialogs::default()
    }

    /// How many dialogs wait behind the one on screen.
    pub fn waiting(&self) -> usize {
        self.dialogs.as_ref().map_or(0, up::Dialogs::queued)
    }

    /// Whether a dialog is on screen.
    pub fn is_open(&self) -> bool {
        self.dialogs.as_ref().is_some_and(up::Dialogs::is_open)
    }

    /// Answer every dialog with `stand_in` instead of the system's dialogs (tests and
    /// automation); `None` shows the system's again. The answers still come back on a later
    /// call of [`FileDialogs::show`] or [`FileDialogs::tick`], like real ones.
    pub fn answer_with(&mut self, stand_in: Option<AnswerWith>) {
        self.stand_in = stand_in.is_some();
        match &mut self.dialogs {
            Some(d) => d.set_override(stand_in.map(upstream_stand_in)),
            None => self.early_stand_in = stand_in,
        }
    }

    /// Once a frame, after [`Shell::ui`], on the main thread, with the window as `parent`: take
    /// the dialogs `shell` was asked for, show the next one, and queue on `shell` the commands
    /// of those that were answered. Without a parent, a Save dialog cannot be shown on macOS (it
    /// is a sheet on its window) and the shell's status bar says so.
    pub fn show<W: HasWindowHandle + HasDisplayHandle>(&mut self, shell: &mut Shell, ctx: &egui::Context, parent: Option<&W>) {
        let asked = shell.take_file_asks();
        if self.dialogs.is_none() && asked.is_empty() && self.early_stand_in.is_none() {
            return;
        }
        let early_stand_in = self.early_stand_in.take();
        let dialogs = self.dialogs.get_or_insert_with(|| {
            // An answer comes from another thread: it wakes the window to run its command.
            let wake = ctx.clone();
            let mut d = up::Dialogs::new(std::sync::Arc::new(move || wake.request_repaint()));
            d.set_override(early_stand_in.map(upstream_stand_in));
            d
        });
        for Asked { ask, then } in asked {
            dialogs.ask(ask.to_upstream(), then);
        }
        dialogs.show_pending(parent);
        let answers = dialogs.take_answers();
        if !answers.is_empty() {
            ctx.request_repaint();
        }
        for outcome in answers.into_iter().filter_map(|(then, answer)| outcome(then, answer)) {
            match outcome {
                Outcome::Run(cmd) => shell.run(cmd),
                Outcome::Unavailable(why, cancelled) => {
                    shell.message = tr_with("The file dialog could not be shown: {why}", &[("why", &why)]);
                    if let Some(cmd) = cancelled {
                        shell.run(cmd);
                    }
                }
            }
        }
    }

    /// [`FileDialogs::show`] with no window: for tests, where a stand-in answers
    /// ([`FileDialogs::answer_with`]).
    pub fn tick(&mut self, shell: &mut Shell, ctx: &egui::Context) {
        self.show(shell, ctx, None::<&NoWindow>);
    }
}

fn upstream_stand_in(mut answer_with: AnswerWith) -> up::Override {
    Box::new(move |request: &up::Request| match request {
        up::Request::File(d) => match answer_with(&FileAsk::from_upstream(d)) {
            FileAnswer::Cancelled => up::Answer::Cancelled,
            FileAnswer::Chosen(paths) => up::Answer::Paths(paths),
            FileAnswer::Unavailable(why) => up::Answer::Unavailable(why),
        },
        up::Request::Message(_) => up::Answer::Unavailable("the shell asks for no message boxes".into()),
    })
}

fn outcome(then: Then, answer: up::Answer) -> Option<Outcome> {
    match answer {
        up::Answer::Paths(paths) if !paths.is_empty() => Some(Outcome::Run(with_paths(then.chosen, &paths))),
        up::Answer::Paths(_) | up::Answer::Cancelled | up::Answer::Message(_) => then.cancelled.map(Outcome::Run),
        up::Answer::Unavailable(why) => {
            let cancelled = then.cancelled.map(|c| with_unavailable(c, &why));
            Some(Outcome::Unavailable(why, cancelled))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ask_says_which_dialog_and_what_it_offers() {
        let ask = FileAsk::save().title(" Save As ").filter("Text", &[".txt", "MD"]).in_folder("drafts").file_name("notes.txt");
        assert_eq!(ask.kind, FileKind::Save);
        assert_eq!(ask.filters, [("Text".to_owned(), vec!["txt".to_owned(), "MD".to_owned()])]);
        let up = ask.to_upstream();
        assert_eq!(up.kind, up::Kind::Save);
        assert_eq!(up.title.as_deref(), Some("Save As"));
        assert_eq!((up.directory.clone(), up.file_name.clone()), (Some(PathBuf::from("drafts")), Some("notes.txt".to_owned())));
        // What a stand-in sees is what was asked.
        let seen = FileAsk::from_upstream(&up);
        assert_eq!(seen, FileAsk { title: "Save As".into(), ..ask });
        for (ask, kind) in [(FileAsk::open(), FileKind::Open), (FileAsk::open_many(), FileKind::OpenMany), (FileAsk::folder(), FileKind::Folder)] {
            assert_eq!(FileAsk::from_upstream(&ask.to_upstream()).kind, kind);
            assert_eq!(ask.to_upstream().title, None, "no title of ours: the system's own");
        }
    }

    #[test]
    fn the_choice_goes_into_the_commands_parameters() {
        let paths = [PathBuf::from("a.txt"), PathBuf::from("b.txt")];
        let cmd = with_paths(Cmd::with("file.import", json!({"as": "notes"})), &paths);
        assert_eq!(cmd.params, json!({"as": "notes", "path": "a.txt", "paths": ["a.txt", "b.txt"]}));
        assert_eq!(with_paths(Cmd::with("x", json!(7)), &paths[..1]).params, json!({"path": "a.txt", "paths": ["a.txt"]}));
    }

    #[test]
    fn answers_run_the_command_asked_for() {
        let then = || Then { chosen: Cmd::new("file.openPath"), cancelled: None };
        let chosen = outcome(then(), up::Answer::Paths(vec![PathBuf::from("a.txt")]));
        assert_eq!(chosen, Some(Outcome::Run(Cmd::with("file.openPath", json!({"path": "a.txt", "paths": ["a.txt"]})))));
        assert_eq!(outcome(then(), up::Answer::Cancelled), None, "closing the dialog runs nothing");
        assert_eq!(outcome(then(), up::Answer::Paths(Vec::new())), None);
        let or = Then { cancelled: Some(Cmd::new("file.closeCancelled")), ..then() };
        assert_eq!(outcome(or, up::Answer::Cancelled), Some(Outcome::Run(Cmd::new("file.closeCancelled"))));
        assert_eq!(outcome(then(), up::Answer::Unavailable("no portal".into())), Some(Outcome::Unavailable("no portal".into(), None)));
    }

    #[test]
    fn a_dialog_that_cannot_be_shown_gives_up_the_waiting_step_with_the_reason() {
        let then = Then { chosen: Cmd::new("file.saveAs"), cancelled: Some(Cmd::with("file.closeCancelled", json!({"window": 2}))) };
        let given_up = Cmd::with("file.closeCancelled", json!({"window": 2, "unavailable": "no portal"}));
        assert_eq!(
            outcome(then.clone(), up::Answer::Unavailable("no portal".into())),
            Some(Outcome::Unavailable("no portal".into(), Some(given_up.clone())))
        );
        assert_eq!(unavailable(&given_up), Some("no portal"));
        // Closed by the user: the same command, and no reason.
        let Some(Outcome::Run(closed)) = outcome(then, up::Answer::Cancelled) else { panic!("the cancelled command runs") };
        assert_eq!(unavailable(&closed), None);
        assert_eq!(with_unavailable(Cmd::with("x", json!(7)), "none").params, json!({"unavailable": "none"}));
    }
}
