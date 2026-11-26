// SPDX-License-Identifier: MIT OR Apache-2.0
//! An app's start-up on the desktop, the same in every app.
//!
//! Every app opens its window the same way, and each step is easy to leave out when it is
//! written by hand: the window's title, app id and icon; the settings folder; the preferences
//! read at start, written as they change and once more at the end; the update check, the close
//! that starts an installed update, its helper started last, and on Windows the helper's own
//! start, which finishes the update and opens no window; the file dialogs; the question
//! before a window with unsaved work closes; files named on the command line, whatever bytes
//! their names hold. This module does all of them.
//!
//! An app gives its data, a [`Desktop`], and what only it can do, a [`DesktopApp`]:
//!
//! ```
//! use std::path::PathBuf;
//! use jcj_shell::desktop::{Desktop, DesktopApp, Releases, Start};
//! use jcj_shell::prefs::{NoPrefs, Prefs};
//! use jcj_shell::{App, Cmd, CommandInfo, Commands, Shell, egui};
//!
//! const DESKTOP: Desktop = Desktop {
//!     releases: Some(Releases { repo: "example-org/sample-app", tagline: "A sample", developer: "the sample contributors" }),
//!     ..Desktop::new("Sample App", "sample-app", "org.example.sample", env!("CARGO_PKG_VERSION"))
//! };
//!
//! #[derive(Default)]
//! struct Sample {
//!     opened: Vec<PathBuf>,
//! }
//! # impl Commands for Sample {
//! #     fn command(&self, _id: &str) -> Option<CommandInfo> {
//! #         None
//! #     }
//! # }
//! # impl App for Sample {
//! #     fn document_name(&self) -> String {
//! #         "Untitled".into()
//! #     }
//! #     fn run(&mut self, _cmd: &Cmd, _shell: &mut Shell) -> Result<(), String> {
//! #         Ok(())
//! #     }
//! #     fn document_ui(&mut self, _ui: &mut egui::Ui, _shell: &mut Shell) {}
//! # }
//!
//! impl DesktopApp for Sample {
//!     type Prefs = Prefs<NoPrefs>;
//!
//!     fn apply_prefs(&mut self, _prefs: &NoPrefs, _shell: &mut Shell) {}
//!
//!     fn prefs(&self, _last: &NoPrefs) -> NoPrefs {
//!         NoPrefs {}
//!     }
//!
//!     fn open(&mut self, files: &[PathBuf], _shell: &mut Shell) {
//!         self.opened = files.to_vec();
//!     }
//! }
//!
//! // The whole of an app's `main`, with the `window` feature:
//! #[cfg(feature = "window")]
//! fn main_of_the_app() -> std::process::ExitCode {
//!     DESKTOP.main(|_start: &mut Start<'_>| Sample::default())
//! }
//!
//! // Without a window (a test, an app that hosts the shell itself), the same steps:
//! let ctx = egui::Context::default();
//! let window = jcj_shell::desktop::Window::start(&DESKTOP, &ctx, None, vec![PathBuf::from("a.txt")], |_start| Sample::default());
//! assert_eq!(window.app.opened, [PathBuf::from("a.txt")]);
//! ```
//!
//! [`Window`] is the window's state and the steps of a frame; it needs no window toolkit, so a
//! test drives it. With the `window` feature the crate depends on eframe, [`Window`] is an
//! `eframe::App`, and [`Desktop::run`] and [`Desktop::main`] open the window. The feature is
//! off unless an app asks for it, so an app that writes its own start-up builds as before.
//!
//! The order of a start: the settings folder is made; the app builds itself ([`Start`]); the
//! update check is attached; the preferences are read and put into effect, the shell's then the
//! app's ([`DesktopApp::apply_prefs`]); the app opens what it starts with
//! ([`DesktopApp::open`]). The order of an end: the app's own last work
//! ([`DesktopApp::exit`]), the preferences, then the helper of an installed update.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Instant;

use egui::{Context, Ui, ViewportBuilder, ViewportCommand};
use jcj_theme::Brand;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::app::App;
use crate::files::FileDialogs;
use crate::prefs::{Contents, PrefsFile};
use crate::shell::Shell;
use crate::updates::desktop::ReleaseSource;

/// Where an app's releases are published, for the update check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Releases {
    /// The GitHub `owner/repo` releases are read and downloaded from, and only from.
    pub repo: &'static str,
    /// What the app is for, in a few words (installers' menu entries).
    pub tagline: &'static str,
    /// Who makes it, as the update prompt says.
    pub developer: &'static str,
}

/// An app's data for its start-up. Build one with [`Desktop::new`] and name the fields that
/// differ, so a field added later takes its default:
/// `Desktop { brand: BRAND, ..Desktop::new(name, id, app_id, version) }`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Desktop {
    /// The app's name: the window's title, and its settings folder on macOS and Windows.
    pub name: &'static str,
    /// Its machine name, lowercase with hyphens: the settings folder elsewhere, the stem of
    /// its release files, and the name its messages on standard error start with.
    pub id: &'static str,
    /// The window's app id, in reverse-domain form: what a Linux desktop matches to the app's
    /// launcher entry, so the window shows the app's icon and groups under it.
    pub app_id: &'static str,
    /// The version running (`env!("CARGO_PKG_VERSION")`).
    pub version: &'static str,
    /// The brand files: design tokens and the window's icon.
    pub brand: Brand,
    /// Where releases come from; `None` for an app that does not check for updates.
    pub releases: Option<Releases>,
    /// An environment variable that names another settings folder, for a run kept apart from
    /// the user's own (a test, a second profile). Unset or empty: the platform's folder.
    pub profile_env: Option<&'static str>,
    /// The window's size at first, in points; `None` for the toolkit's.
    pub window_size: Option<[f32; 2]>,
}

impl Desktop {
    /// An app with the neutral look, no icon, no update check and the platform's settings folder.
    pub const fn new(name: &'static str, id: &'static str, app_id: &'static str, version: &'static str) -> Desktop {
        Desktop { name, id, app_id, version, brand: Brand::NEUTRAL, releases: None, profile_env: None, window_size: None }
    }

    /// The app's settings folder: the one [`Desktop::profile_env`] names when it is set, else
    /// the platform's ([`crate::prefs::settings_dir`]). `None` when the platform gives none.
    pub fn settings_dir(&self) -> Option<PathBuf> {
        self.settings_dir_from(self.profile_env.and_then(std::env::var_os))
    }

    /// [`Desktop::settings_dir`], given the profile variable's value.
    pub fn settings_dir_from(&self, profile: Option<OsString>) -> Option<PathBuf> {
        match profile.filter(|dir| !dir.is_empty()) {
            Some(dir) => Some(PathBuf::from(dir)),
            None => crate::prefs::settings_dir(self.id, self.name),
        }
    }

    /// The window as the toolkit should make it: title, app id, icon and size.
    pub fn viewport(&self) -> ViewportBuilder {
        let mut viewport = ViewportBuilder::default().with_title(self.name).with_app_id(self.app_id);
        if let Some(size) = self.window_size {
            viewport = viewport.with_inner_size(size);
        }
        if let Some(icon) = self.brand.icon() {
            viewport = viewport.with_icon(icon);
        }
        viewport
    }

    /// The app's releases as the update check needs them. An update is installed in place on
    /// Linux and Windows; on macOS the check offers the download, because the updater at the
    /// core's pin cannot replace a macOS app.
    pub fn release_source(&self) -> Option<ReleaseSource> {
        self.releases.map(|r| ReleaseSource {
            id: self.id,
            name: self.name,
            tagline: r.tagline,
            repo: r.repo,
            developer: r.developer,
            version: self.version,
            installs_in_place: !cfg!(target_os = "macos"),
        })
    }

    /// Finish an update when this process was started to do that (on Windows the updater
    /// starts a copy of the app as its helper). An app with a command line of its own calls
    /// this first thing in `main`, before it reads its arguments, and returns the code it is
    /// given; [`Desktop::main`] does it itself. `None`: an ordinary start.
    pub fn finish_update(&self) -> Option<std::process::ExitCode> {
        let args: Vec<OsString> = std::env::args_os().skip(1).collect();
        Some(match self.finish_update_with(&args)? {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{}: the update could not be finished: {e}", self.id);
                std::process::ExitCode::FAILURE
            }
        })
    }

    /// [`Desktop::finish_update`], given the arguments (without the program's name).
    pub fn finish_update_with(&self, args: &[OsString]) -> Option<Result<(), String>> {
        if !crate::updates::desktop::is_finish_update(args) {
            return None;
        }
        match self.release_source() {
            Some(source) => source.finish_update(args),
            None => Some(Err("this app does not install updates".into())),
        }
    }

    /// A shell with the app's name, version and brand tokens. [`Window::start`] makes it, hands
    /// it to the app in [`Start`], and attaches the update check afterwards.
    pub fn shell(&self) -> Shell {
        Shell::new(self.name, self.version, self.brand.tokens())
    }

    /// Open the app's window and run it until it closes: every step of this module, with the
    /// app built by `build` once the window exists. `files` are the files to open
    /// ([`arguments`] for those named on the command line). A process started as an update's
    /// helper opens no window here either: the helper's result is returned.
    #[cfg(feature = "window")]
    pub fn run<A, B>(&self, files: Vec<PathBuf>, build: B) -> Result<(), String>
    where
        A: DesktopApp + 'static,
        B: FnOnce(&mut Start<'_>) -> A + 'static,
    {
        // Started as an update's helper: no window. An app whose own command line would
        // refuse the helper's arguments calls `finish_update` before it reads them.
        if let Some(done) = self.finish_update_with(&std::env::args_os().skip(1).collect::<Vec<_>>()) {
            return done;
        }
        let desktop = *self;
        let options = eframe::NativeOptions { viewport: self.viewport(), ..eframe::NativeOptions::default() };
        eframe::run_native(
            self.name,
            options,
            Box::new(move |cc| Ok(Box::new(Window::start(&desktop, &cc.egui_ctx, desktop.settings_dir(), files, build)))),
        )
        .map_err(|e| e.to_string())
    }

    /// The whole `main` of a desktop app: the update helper's work when the process was started
    /// for it ([`Desktop::finish_update`]), else [`Desktop::run`] with the files named on the
    /// command line. A window that cannot be opened is said on standard error, with exit code 1.
    ///
    /// An app with a command line of its own (`--version`, subcommands) calls
    /// [`Desktop::finish_update`], reads its command line, then calls [`Desktop::run`].
    #[cfg(feature = "window")]
    pub fn main<A, B>(&self, build: B) -> std::process::ExitCode
    where
        A: DesktopApp + 'static,
        B: FnOnce(&mut Start<'_>) -> A + 'static,
    {
        if let Some(code) = self.finish_update() {
            return code;
        }
        match self.run(arguments(), build) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("{}: could not open a window: {e}", self.id);
                std::process::ExitCode::FAILURE
            }
        }
    }
}

/// The files named on the command line: every argument that is not an option. The names are
/// kept as the system gave them, so a name that is not Unicode still opens.
pub fn arguments() -> Vec<PathBuf> {
    files_named(std::env::args_os().skip(1))
}

/// The files among `args`: all but those starting with `-`.
pub fn files_named(args: impl IntoIterator<Item = OsString>) -> Vec<PathBuf> {
    args.into_iter().filter(|a| !a.is_empty() && a.as_encoded_bytes().first() != Some(&b'-')).map(PathBuf::from).collect()
}

/// What an app has to build itself with, once its window exists.
#[derive(Debug)]
pub struct Start<'a> {
    /// The window's egui context (to wake the window from another thread, to load textures).
    pub ctx: &'a Context,
    /// The shell, with the app's name, version and brand. The app sets its options here, or
    /// puts its own shell in its place; the update check is attached after the app is built.
    pub shell: Shell,
    /// The app's settings folder, already made: for what the app keeps beside its preferences
    /// (a personal dictionary, recovery copies, history). `None` when there is none.
    pub settings: Option<&'a Path>,
    /// The files the app was started with ([`DesktopApp::open`] gets them again, once the
    /// preferences are in effect).
    pub files: &'a [PathBuf],
}

impl Start<'_> {
    /// Change the shell with its builder methods: `start.shell_with(|s| s.with_options(o))`.
    pub fn shell_with(&mut self, change: impl FnOnce(Shell) -> Shell) {
        let shell = std::mem::replace(&mut self.shell, Shell::new("", "", jcj_theme::Tokens::default()));
        self.shell = change(shell);
    }
}

/// The app's own preferences of a [`DesktopApp`]: the app's part of its preferences file.
pub type AppPrefs<A> = <<A as DesktopApp>::Prefs as Contents>::App;

/// What only the app can do at start-up, while it runs and at the end. Only the preferences
/// must be written; every other step has a default that does nothing.
pub trait DesktopApp: App + Sized {
    /// The preferences file's contents: [`crate::prefs::Prefs`] with the app's own struct
    /// ([`crate::prefs::NoPrefs`] when it has none), or the app's own type
    /// ([`Contents`]).
    type Prefs: Contents;

    /// Put the app's preferences into effect, as read at start-up (the defaults when there is
    /// no file or no settings folder). The shell's are in effect already, so what depends on
    /// both is done here: the interface language, a folder named in a preference.
    fn apply_prefs(&mut self, prefs: &AppPrefs<Self>, shell: &mut Shell);

    /// The app's preferences as they are now. `last` is what was read or last written: a
    /// preference the window does not show is taken from there. Called every frame; the file
    /// is written when the answer changes.
    fn prefs(&self, last: &AppPrefs<Self>) -> AppPrefs<Self>;

    /// Open what the app starts with, once the preferences are in effect. `files` are those it
    /// was started with; with none the choice is the app's (the documents open at the last
    /// close, its start page, an empty document).
    fn open(&mut self, files: &[PathBuf], shell: &mut Shell) {
        let _ = (files, shell);
    }

    /// Show a problem of the start-up's own (preferences that could not be read or saved).
    /// The default is the shell's status bar; an app with its own place for messages puts it
    /// there.
    fn report(&mut self, problem: String, shell: &mut Shell) {
        shell.message = problem;
    }

    /// Whether the window may close now. An app with unsaved work asks its question here
    /// ([`Shell::ask_unsaved`]) and answers no; the window stays open.
    fn may_close(&mut self, shell: &mut Shell) -> bool {
        let _ = shell;
        true
    }

    /// Whether the app's question was answered so that the window should close now.
    fn close_confirmed(&self) -> bool {
        false
    }

    /// The app's own work once a frame, before the window is drawn (a file it keeps current).
    fn tick(&mut self, ctx: &Context, shell: &mut Shell) {
        let _ = (ctx, shell);
    }

    /// The window's title, when it is not the app's name (the open document's name, a mark for
    /// unsaved changes). The window is told when it changes.
    fn window_title(&self) -> Option<String> {
        None
    }

    /// The app's last work, when the window has closed: a closing save or backup, its own
    /// files. The preferences are written after it, and an installed update starts last.
    fn exit(&mut self, shell: &mut Shell) {
        let _ = shell;
    }
}

/// An app's window: its shell, the app, its file dialogs and its preferences file, and the
/// steps of a frame. With the `window` feature it is an `eframe::App`.
///
/// Dropping a window ends it ([`Window::exit`]) if that was not done, so an end reached another
/// way still writes the preferences and starts an installed update.
pub struct Window<A: DesktopApp> {
    /// The shell.
    pub shell: Shell,
    /// The app.
    pub app: A,
    /// The window's part of the shell's file dialogs.
    pub files: FileDialogs,
    /// The preferences file and its contents as last shown, when there is a settings folder.
    prefs: Option<(PrefsFile<A::Prefs>, A::Prefs)>,
    /// An installed update asked for a restart, and closing was asked for once.
    restart_asked: bool,
    /// The title the window was last told.
    title: Option<String>,
    exited: bool,
}

impl<A: DesktopApp> std::fmt::Debug for Window<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Window")
            .field("name", &self.shell.name)
            .field("preferences", &self.prefs.as_ref().map(|(file, _)| &file.path))
            .field("exited", &self.exited)
            .finish_non_exhaustive()
    }
}

impl<A: DesktopApp> Window<A> {
    /// Start an app: every step up to the first frame (the order is in the module's text).
    /// `settings` is the app's settings folder ([`Desktop::settings_dir`]); without one the app
    /// runs with its defaults and nothing is kept.
    pub fn start(desktop: &Desktop, ctx: &Context, settings: Option<PathBuf>, files: Vec<PathBuf>, build: impl FnOnce(&mut Start<'_>) -> A) -> Self {
        if let Some(dir) = &settings
            && let Err(e) = std::fs::create_dir_all(dir)
        {
            // Not fatal: the app runs, and a preference that cannot be saved is reported then.
            log::warn!("the settings folder could not be made: {e}");
        }
        let mut start = Start { ctx, shell: desktop.shell(), settings: settings.as_deref(), files: &files };
        let mut app = build(&mut start);
        let mut shell = start.shell;
        if let Some(source) = desktop.release_source() {
            shell = shell.with_updates(source);
        }
        let (file, read, problem) = match &settings {
            Some(dir) => {
                let (file, read, problem) = PrefsFile::<A::Prefs>::open(dir);
                (Some(file), read, problem)
            }
            None => (None, A::Prefs::default(), None),
        };
        shell.apply_prefs(read.shell(), &app.panels());
        app.apply_prefs(read.app(), &mut shell);
        if let Some(problem) = problem {
            app.report(problem, &mut shell);
        }
        app.open(&files, &mut shell);
        Window { shell, app, files: FileDialogs::new(), prefs: file.map(|f| (f, read)), restart_asked: false, title: None, exited: false }
    }

    /// The preferences file, when there is a settings folder.
    pub fn prefs_path(&self) -> Option<&Path> {
        self.prefs.as_ref().map(|(file, _)| file.path.as_path())
    }

    /// Once a frame, before the window is drawn: answer a request to close, keep the
    /// preferences file current, let the app do its own work, and tell the window its title.
    pub fn logic(&mut self, ctx: &Context) {
        let asked = ctx.input(|i| i.viewport().close_requested());
        if let Some(command) = self.closing(asked) {
            ctx.send_viewport_cmd(command);
        }
        self.save_prefs(Some(ctx), false);
        self.app.tick(ctx, &mut self.shell);
        if let Some(title) = self.app.window_title()
            && self.title.as_ref() != Some(&title)
        {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title = Some(title);
        }
    }

    /// What to tell the window about closing this frame. `asked`: the user or the system asked
    /// the window to close.
    fn closing(&mut self, asked: bool) -> Option<ViewportCommand> {
        // Closing with unsaved work asks first.
        if asked && !self.app.may_close(&mut self.shell) {
            return Some(ViewportCommand::CancelClose);
        }
        // An installed update waits for a restart: close as the close button would, asking
        // about unsaved work first. Once: Cancel keeps the window open.
        let restart = self.shell.close_requested() && !self.restart_asked;
        if restart {
            self.restart_asked = true;
        }
        if (restart && self.app.may_close(&mut self.shell)) || self.app.close_confirmed() {
            return Some(ViewportCommand::Close);
        }
        None
    }

    /// Draw a frame: the shell around the app, then the next file dialog the shell was asked
    /// for, over the window `parent`. An answer runs its command in a later frame.
    pub fn ui<W: HasWindowHandle + HasDisplayHandle>(&mut self, ui: &mut Ui, parent: Option<&W>) {
        self.shell.ui(ui, &mut self.app);
        self.files.show(&mut self.shell, ui.ctx(), parent);
    }

    /// [`Window::ui`] with no window behind it: for tests and offscreen pictures, where a
    /// stand-in answers the file dialogs ([`FileDialogs::answer_with`]).
    pub fn ui_offscreen(&mut self, ui: &mut Ui) {
        self.shell.ui(ui, &mut self.app);
        self.files.tick(&mut self.shell, ui.ctx());
    }

    /// The end: the app's own last work, the preferences written whatever the time since the
    /// last write, then the helper of an installed update. A second call does nothing.
    pub fn exit(&mut self) {
        if std::mem::replace(&mut self.exited, true) {
            return;
        }
        self.app.exit(&mut self.shell);
        self.save_prefs(None, true);
        // Last, once everything is saved: the helper starts the installed version after this exit.
        self.shell.finish();
    }

    /// Keep the preferences file up to date with what the window shows.
    fn save_prefs(&mut self, ctx: Option<&Context>, force: bool) {
        let Some((file, shown)) = &mut self.prefs else { return };
        *shown = A::Prefs::of(self.shell.prefs(), self.app.prefs(shown.app()));
        match file.save(shown, Instant::now(), force) {
            // A change is waiting for the next write: come back for it without an input.
            Ok(Some(wait)) => {
                if let Some(ctx) = ctx {
                    ctx.request_repaint_after(wait);
                }
            }
            Ok(None) => {}
            Err(e) => {
                let problem = jcj_i18n::tr_with("Preferences could not be saved: {why}", &[("why", e.as_str())]);
                self.app.report(problem, &mut self.shell);
            }
        }
    }
}

impl<A: DesktopApp> Drop for Window<A> {
    fn drop(&mut self) {
        self.exit();
    }
}

#[cfg(feature = "window")]
impl<A: DesktopApp> eframe::App for Window<A> {
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        Window::logic(self, ctx);
    }

    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        Window::ui(self, ui, Some(&*frame));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.exit();
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::commands::{Cmd, CommandInfo, Commands};
    use crate::prefs::{NoPrefs, Prefs};

    const DESKTOP: Desktop = Desktop {
        releases: Some(Releases { repo: "example-org/sample-app", tagline: "A test app", developer: "the sample contributors" }),
        profile_env: Some("JCJ_SHELL_TEST_PROFILE_NEVER_SET"),
        window_size: Some([900.0, 700.0]),
        ..Desktop::new("Sample App", "sample-app", "org.example.sample", "1.2.3")
    };

    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    struct SamplePrefs {
        zoom: u32,
        /// A preference the window does not show: kept as the file has it.
        folder: String,
    }

    /// What happened, in order, seen from outside the window (which owns the app).
    type Log = Rc<RefCell<Vec<String>>>;

    #[derive(Default)]
    struct Sample {
        log: Log,
        zoom: u32,
        dirty: bool,
        confirmed: bool,
        title: Option<String>,
        problems: Vec<String>,
    }

    impl Sample {
        fn note(&self, what: impl Into<String>) {
            self.log.borrow_mut().push(what.into());
        }
    }

    impl Commands for Sample {
        fn command(&self, _id: &str) -> Option<CommandInfo> {
            None
        }
    }

    impl App for Sample {
        fn document_name(&self) -> String {
            "Untitled".into()
        }

        fn run(&mut self, _cmd: &Cmd, _shell: &mut Shell) -> Result<(), String> {
            Ok(())
        }

        fn document_ui(&mut self, ui: &mut Ui, _shell: &mut Shell) {
            ui.label("The document");
        }
    }

    impl DesktopApp for Sample {
        type Prefs = Prefs<SamplePrefs>;

        fn apply_prefs(&mut self, prefs: &SamplePrefs, shell: &mut Shell) {
            self.note(format!("prefs zoom {} theme {:?}", prefs.zoom, shell.mode));
            self.zoom = prefs.zoom;
        }

        fn prefs(&self, last: &SamplePrefs) -> SamplePrefs {
            SamplePrefs { zoom: self.zoom, folder: last.folder.clone() }
        }

        fn open(&mut self, files: &[PathBuf], _shell: &mut Shell) {
            self.note(format!("open {}", files.len()));
        }

        fn report(&mut self, problem: String, _shell: &mut Shell) {
            self.problems.push(problem);
        }

        fn may_close(&mut self, _shell: &mut Shell) -> bool {
            !self.dirty || self.confirmed
        }

        fn close_confirmed(&self) -> bool {
            self.confirmed
        }

        fn tick(&mut self, _ctx: &Context, _shell: &mut Shell) {
            self.note("tick");
        }

        fn window_title(&self) -> Option<String> {
            self.title.clone()
        }

        fn exit(&mut self, _shell: &mut Shell) {
            self.note("exit");
        }
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("jcj-shell-desktop-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn start(settings: Option<PathBuf>, files: Vec<PathBuf>, log: &Log) -> Window<Sample> {
        let log = log.clone();
        Window::start(&DESKTOP, &Context::default(), settings, files, move |start| {
            start.shell_with(|s| s.with_options(crate::ShellOptions { status_bar: false, ..crate::ShellOptions::default() }));
            let app = Sample { log, ..Sample::default() };
            app.note(format!("build settings {} files {}", start.settings.is_some(), start.files.len()));
            app
        })
    }

    /// The commands the start-up sends a window, among all a frame sent (the theme sends its
    /// own).
    fn ours(sent: Option<&egui::ViewportOutput>) -> Vec<ViewportCommand> {
        let ours = |c: &&ViewportCommand| matches!(c, ViewportCommand::Close | ViewportCommand::CancelClose | ViewportCommand::Title(_));
        sent.map(|v| v.commands.iter().filter(ours).cloned().collect()).unwrap_or_default()
    }

    /// One frame of `window`, as the toolkit runs it; the commands the start-up sent the window.
    fn frame(ctx: &Context, window: &mut Window<Sample>, close_asked: bool) -> Vec<ViewportCommand> {
        let mut info = egui::ViewportInfo::default();
        if close_asked {
            info.events.push(egui::ViewportEvent::Close);
        }
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0))),
            viewports: std::iter::once((egui::ViewportId::ROOT, info)).collect(),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            window.logic(ui.ctx());
            window.ui_offscreen(ui);
        });
        // No picture is made of the frame: its textures are not needed.
        out.textures_delta.clear();
        ours(out.viewport_output.get(&egui::ViewportId::ROOT))
    }

    #[test]
    fn the_window_is_made_from_the_app_s_data() {
        let viewport = DESKTOP.viewport();
        assert_eq!(viewport.title.as_deref(), Some("Sample App"));
        assert_eq!(viewport.app_id.as_deref(), Some("org.example.sample"));
        assert_eq!(viewport.inner_size, Some(egui::vec2(900.0, 700.0)));
        assert!(viewport.icon.is_none(), "the neutral brand has no icon");
        let plain = Desktop::new("Plain", "plain", "org.example.plain", "0.1.0");
        assert_eq!((plain.viewport().inner_size, plain.release_source(), plain.profile_env), (None, None, None));
        let shell = DESKTOP.shell();
        assert_eq!((shell.name.as_str(), shell.version.as_str()), ("Sample App", "1.2.3"));
        assert_eq!(shell.tokens(), &jcj_theme::Tokens::default());
    }

    #[test]
    fn releases_are_the_app_s_and_macos_is_sent_to_the_release_page() {
        let source = DESKTOP.release_source().unwrap();
        assert_eq!((source.id, source.name, source.version), ("sample-app", "Sample App", "1.2.3"));
        assert_eq!((source.repo, source.tagline, source.developer), ("example-org/sample-app", "A test app", "the sample contributors"));
        assert_eq!(source.installs_in_place, !cfg!(target_os = "macos"));
        assert_eq!(source.releases_url(), "https://github.com/example-org/sample-app/releases");
    }

    #[test]
    fn a_start_as_the_update_helper_is_told_from_an_ordinary_one() {
        let os = |args: &[&str]| args.iter().map(OsString::from).collect::<Vec<_>>();
        assert_eq!(DESKTOP.finish_update_with(&os(&["a.txt"])), None);
        assert_eq!(DESKTOP.finish_update_with(&[]), None);
        let helper = os(&[crate::updates::desktop::FINISH_FLAG]);
        assert!(DESKTOP.finish_update_with(&helper).is_some_and(|done| done.is_err()), "not a whole update: an error, no window");
        // An app that does not install updates never acts on the helper's arguments.
        let plain = Desktop::new("Plain", "plain", "org.example.plain", "0.1.0");
        assert_eq!(plain.finish_update_with(&helper), Some(Err("this app does not install updates".into())));
        // The test's own arguments are not the helper's.
        assert!(DESKTOP.finish_update().is_none());
    }

    #[test]
    fn a_profile_folder_is_used_whole() {
        assert_eq!(DESKTOP.settings_dir_from(Some("profile-folder".into())), Some(PathBuf::from("profile-folder")));
        // Otherwise the platform's folder for the app, as the shell names it.
        assert_eq!(DESKTOP.settings_dir_from(None), crate::prefs::settings_dir("sample-app", "Sample App"));
        assert_eq!(DESKTOP.settings_dir_from(Some("".into())), DESKTOP.settings_dir_from(None));
        assert_eq!(DESKTOP.settings_dir(), DESKTOP.settings_dir_from(None), "the variable is not set");
    }

    #[test]
    fn files_are_the_arguments_that_are_not_options() {
        let args = ["a.txt", "--flag", "", "-x", "b c.txt"].map(OsString::from);
        assert_eq!(files_named(args), [PathBuf::from("a.txt"), PathBuf::from("b c.txt")]);
        assert!(files_named(Vec::new()).is_empty());
    }

    /// A name that is not Unicode stays a file to open, byte for byte.
    #[cfg(unix)]
    #[test]
    fn a_name_that_is_not_unicode_is_kept() {
        use std::os::unix::ffi::OsStringExt;
        let odd = OsString::from_vec(vec![b'a', 0xFF, b'.', b't', b'x', b't']);
        assert!(odd.to_str().is_none());
        assert_eq!(files_named([odd.clone()]), [PathBuf::from(odd)]);
    }

    #[test]
    fn a_start_runs_every_step_in_order() {
        let dir = temp_dir("order");
        let log = Log::default();
        let window = start(Some(dir.clone()), vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")], &log);
        assert_eq!(*log.borrow(), ["build settings true files 2", "prefs zoom 0 theme Light", "open 2"]);
        assert!(dir.is_dir(), "the settings folder is made");
        assert_eq!(window.prefs_path(), Some(dir.join("preferences.json").as_path()));
        assert!(!window.shell.options.status_bar, "the shell the app changed is the one used");
        assert!(window.app.problems.is_empty());
        drop(window);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn preferences_are_read_written_as_they_change_and_written_at_the_end() {
        let dir = temp_dir("prefs");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("preferences.json"), r#"{"shell": {"theme": "dark"}, "app": {"zoom": 3, "folder": "kept"}}"#).unwrap();
        let log = Log::default();
        let ctx = Context::default();
        let mut window = start(Some(dir.clone()), Vec::new(), &log);
        assert_eq!((window.app.zoom, window.shell.mode), (3, jcj_theme::Mode::Dark));
        assert_eq!(log.borrow()[1], "prefs zoom 3 theme Dark", "the shell's preferences are in effect before the app's");

        // A change is written by a frame, with the preference the window does not show kept.
        window.app.zoom = 5;
        window.shell.mode = jcj_theme::Mode::ExtraDark;
        frame(&ctx, &mut window, false);
        let read = |dir: &Path| serde_json::from_str::<Prefs<SamplePrefs>>(&std::fs::read_to_string(dir.join("preferences.json")).unwrap()).unwrap();
        assert_eq!(read(&dir).app, SamplePrefs { zoom: 5, folder: "kept".into() });
        assert_eq!(read(&dir).shell.theme, jcj_theme::Mode::ExtraDark);

        // A second change within the write interval waits, and the end writes it.
        window.app.zoom = 7;
        frame(&ctx, &mut window, false);
        assert_eq!(read(&dir).app.zoom, 5);
        window.exit();
        assert_eq!(read(&dir).app.zoom, 7);
        assert_eq!(log.borrow().iter().filter(|l| *l == "exit").count(), 1);
        // Ending twice, or dropping an ended window, does nothing more.
        window.exit();
        drop(window);
        assert_eq!(log.borrow().iter().filter(|l| *l == "exit").count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_window_that_is_dropped_still_ends() {
        let dir = temp_dir("drop");
        let log = Log::default();
        let mut window = start(Some(dir.clone()), Vec::new(), &log);
        window.app.zoom = 9;
        drop(window);
        assert_eq!(log.borrow().last().map(String::as_str), Some("exit"));
        let text = std::fs::read_to_string(dir.join("preferences.json")).unwrap();
        assert_eq!(serde_json::from_str::<Prefs<SamplePrefs>>(&text).unwrap().app.zoom, 9);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn damaged_preferences_are_reported_to_the_app_and_the_defaults_used() {
        let dir = temp_dir("damaged");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("preferences.json"), "{ not json").unwrap();
        let window = start(Some(dir.clone()), Vec::new(), &Log::default());
        assert_eq!(window.app.problems.len(), 1);
        assert!(window.app.problems[0].contains("defaults"), "{}", window.app.problems[0]);
        assert_eq!(window.app.zoom, 0);
        assert!(dir.join("preferences.json.damaged").exists());
        drop(window);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn preferences_that_cannot_be_saved_are_reported() {
        let dir = temp_dir("unwritable");
        std::fs::create_dir_all(&dir).unwrap();
        // A file where the settings folder should be: the folder cannot be made, nothing can be
        // written, and the app still starts.
        std::fs::write(dir.join("not-a-folder"), "in the way").unwrap();
        let mut window = start(Some(dir.join("not-a-folder").join("settings")), Vec::new(), &Log::default());
        window.app.problems.clear();
        window.app.zoom = 2;
        frame(&Context::default(), &mut window, false);
        assert!(window.app.problems.iter().any(|p| p.starts_with("Preferences could not be saved")), "{:?}", window.app.problems);
        drop(window);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn without_a_settings_folder_the_app_runs_on_its_defaults() {
        let log = Log::default();
        let mut window = start(None, vec![PathBuf::from("a.txt")], &log);
        assert_eq!(*log.borrow(), ["build settings false files 1", "prefs zoom 0 theme Light", "open 1"]);
        assert_eq!(window.prefs_path(), None);
        window.app.zoom = 4;
        frame(&Context::default(), &mut window, false);
        window.exit();
        assert!(window.app.problems.is_empty());
        assert_eq!(log.borrow().last().map(String::as_str), Some("exit"));
    }

    #[test]
    fn the_default_report_is_the_shell_s_status_bar() {
        struct Plain;
        impl Commands for Plain {
            fn command(&self, _id: &str) -> Option<CommandInfo> {
                None
            }
        }
        impl App for Plain {
            fn document_name(&self) -> String {
                "Untitled".into()
            }
            fn run(&mut self, _cmd: &Cmd, _shell: &mut Shell) -> Result<(), String> {
                Ok(())
            }
            fn document_ui(&mut self, _ui: &mut Ui, _shell: &mut Shell) {}
        }
        impl DesktopApp for Plain {
            type Prefs = Prefs<NoPrefs>;
            fn apply_prefs(&mut self, _prefs: &NoPrefs, _shell: &mut Shell) {}
            fn prefs(&self, _last: &NoPrefs) -> NoPrefs {
                NoPrefs {}
            }
        }
        let dir = temp_dir("plain");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("preferences.json"), "[").unwrap();
        let mut window = Window::start(&DESKTOP, &Context::default(), Some(dir.clone()), Vec::new(), |_| Plain);
        assert!(window.shell.message.contains("defaults"), "{}", window.shell.message);
        // Every other step has a default: the window may close, has the app's name, and ends.
        assert_eq!(window.closing(true), None);
        assert!(frame_of(&mut window).is_empty());
        window.exit();
        let _ = std::fs::remove_dir_all(&dir);

        fn frame_of(window: &mut Window<Plain>) -> Vec<ViewportCommand> {
            let ctx = Context::default();
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                window.logic(ui.ctx());
                window.ui_offscreen(ui);
            });
            out.textures_delta.clear();
            ours(out.viewport_output.get(&egui::ViewportId::ROOT))
        }
    }

    #[test]
    fn a_window_with_unsaved_work_stays_open_until_the_app_says_close() {
        let ctx = Context::default();
        let mut window = start(None, Vec::new(), &Log::default());
        assert!(frame(&ctx, &mut window, true).is_empty(), "nothing unsaved: the window closes");
        window.app.dirty = true;
        assert_eq!(frame(&ctx, &mut window, true), [ViewportCommand::CancelClose]);
        assert!(frame(&ctx, &mut window, false).is_empty());
        // The app's question was answered: the window is told to close, and is let.
        window.app.confirmed = true;
        assert_eq!(frame(&ctx, &mut window, false), [ViewportCommand::Close]);
        assert_eq!(frame(&ctx, &mut window, true), [ViewportCommand::Close]);
    }

    #[test]
    fn an_installed_update_closes_the_window_once_and_asks_about_unsaved_work() {
        let mut window = start(None, Vec::new(), &Log::default());
        assert_eq!(window.closing(false), None);
        window.shell.request_close();
        window.app.dirty = true;
        assert_eq!(window.closing(false), None, "unsaved work: the app's question first");
        assert_eq!(window.closing(false), None, "asked once: cancelling keeps the window open");
        window.app.confirmed = true;
        assert_eq!(window.closing(false), Some(ViewportCommand::Close));

        let mut clean = start(None, Vec::new(), &Log::default());
        clean.shell.request_close();
        assert_eq!(clean.closing(false), Some(ViewportCommand::Close));
        assert_eq!(clean.closing(false), None);
    }

    #[test]
    fn the_window_is_told_its_title_when_it_changes() {
        let ctx = Context::default();
        let log = Log::default();
        let mut window = start(None, Vec::new(), &log);
        assert!(frame(&ctx, &mut window, false).is_empty(), "no title of the app's: the name stays");
        window.app.title = Some("Draft - Sample App".into());
        assert_eq!(frame(&ctx, &mut window, false), [ViewportCommand::Title("Draft - Sample App".into())]);
        assert!(frame(&ctx, &mut window, false).is_empty(), "told once");
        window.app.title = Some("Draft* - Sample App".into());
        assert_eq!(frame(&ctx, &mut window, false), [ViewportCommand::Title("Draft* - Sample App".into())]);
        assert_eq!(log.borrow().iter().filter(|l| *l == "tick").count(), 4, "the app's own work runs every frame");
    }

    #[test]
    fn a_file_dialog_the_shell_was_asked_for_is_answered_through_the_window() {
        let ctx = Context::default();
        let mut window = start(None, Vec::new(), &Log::default());
        window.files.answer_with(Some(Box::new(|_| crate::files::FileAnswer::Chosen(vec![PathBuf::from("chosen.txt")]))));
        assert!(window.shell.ask_file(crate::files::FileAsk::open(), Cmd::new("file.openPath")));
        for _ in 0..4 {
            frame(&ctx, &mut window, false);
        }
        assert_eq!(window.shell.file_asks_waiting(), 0);
        assert!(!window.files.is_open());
    }
}
