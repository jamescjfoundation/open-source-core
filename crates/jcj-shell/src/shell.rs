// SPDX-License-Identifier: MIT OR Apache-2.0
//! The shell: the window around an app's document. The app builds a [`Shell`] with its name,
//! version and design tokens, and calls [`Shell::ui`] every frame with itself ([`App`]).

use std::time::Instant;

use egui::{Event, Rect, Ui};
use jcj_i18n::{tl, tr_with};
use jcj_theme::{FontFace, Mode, Theme, Tokens};
use serde_json::Value;

use crate::app::App;
use crate::autosave::{AutoSave, Tick};
use crate::backstage::Page;
use crate::commands::{Cmd, shell_ids};
use crate::dialogs::{Dialogs, Palette, RecoveryCopy, UnsavedPrompt};
use crate::keys::{KeyMap, parse_shortcut, shortcut};
use crate::prefs::{ShellPrefs, add_recent};
use crate::ribbon::Chrome;
use crate::updates::Updates;
use crate::workspace::{Layout, PanelSpec, Side};

/// How the shell behaves where apps may differ. Every option defaults to the behavior apps had
/// before it existed; an option that changes what users see is turned on by the app that wants
/// it (the plan's "Step 3").
#[derive(Clone, Debug, PartialEq)]
pub struct ShellOptions {
    /// Run commands bound to keys with Mod, Ctrl or Alt, or function keys, before the document
    /// sees them (on). Plain keys always go to the document.
    pub shortcuts: bool,
    /// The quick-access row above the ribbon (on).
    pub quick_row: bool,
    /// The ribbon (on).
    pub ribbon: bool,
    /// Ribbon groups shrink and fold to fit the window (on).
    pub ribbon_fit: bool,
    /// The status bar (on).
    pub status_bar: bool,
    /// The File button's label ("File").
    pub file_label: String,
    /// The shell's File pages, in order (all of [`Page::BUILT_IN`]).
    pub file_pages: Vec<Page>,
    /// The first-run tour: a title and a line for each step (none: no tour).
    pub tour: Vec<(String, String)>,
    /// Command parameters that follow the window's state rather than tell controls apart, so a
    /// control keeps its key tip letters as they change.
    pub keytip_ignored_params: Vec<String>,
    /// Show the start window (templates, Open, recent files) in place of the document while the
    /// app has none open (off).
    pub start_window: bool,
}

impl Default for ShellOptions {
    fn default() -> Self {
        ShellOptions {
            shortcuts: true,
            quick_row: true,
            ribbon: true,
            ribbon_fit: true,
            status_bar: true,
            file_label: tl("File").into(),
            file_pages: Page::BUILT_IN.to_vec(),
            tour: Vec::new(),
            keytip_ignored_params: ["on", "id", "ids"].map(String::from).to_vec(),
            start_window: false,
        }
    }
}

/// The window around an app's document: ribbon, panels, File page, dialogs, status bar,
/// preferences, keys, auto-save and updates.
#[derive(Debug)]
pub struct Shell {
    /// The app's name.
    pub name: String,
    /// The app's version.
    pub version: String,
    tokens: Tokens,
    fonts: Vec<FontFace>,
    fonts_installed: bool,
    applied: Option<Theme>,
    /// The theme in use.
    pub mode: Mode,
    /// The chrome: the ribbon's state, key tips, the File page, the tour.
    pub chrome: Chrome,
    /// The panels' layout.
    pub layout: Layout,
    /// Auto-save.
    pub autosave: AutoSave,
    /// Updates as the window shows them.
    pub updates: Updates,
    #[cfg(not(target_arch = "wasm32"))]
    updater: Option<crate::updates::desktop::Updater>,
    /// The user's own keys.
    pub keys: KeyMap,
    /// Recent documents, newest first.
    pub recent: Vec<String>,
    /// Open the documents that were open when the app last closed.
    pub reopen_last: bool,
    /// The documents open when the app last closed (the app keeps it current).
    pub last_open: Vec<String>,
    /// The user's name.
    pub user_name: String,
    /// The first-run tour has been seen (in an earlier run).
    pub tour_seen: bool,
    /// The dialogs.
    pub dialogs: Dialogs,
    /// A message for the status bar (a command's error); empty for none.
    pub message: String,
    /// The options.
    pub options: ShellOptions,
    /// Whether the platform is macOS (keys show as ⌘; its own text keys apply).
    pub mac: bool,
    queue: Vec<Cmd>,
    /// The file dialogs asked for, until the window's [`crate::files::FileDialogs`] takes them.
    file_asks: Vec<crate::files::Asked>,
    close_requested: bool,
    /// Rails and floating panels drawn this frame (a press elsewhere closes floating panels).
    pub(crate) rails: Vec<Rect>,
    pub(crate) floating: Vec<Rect>,
}

impl Shell {
    /// A shell for the app `name` at `version`, themed with `tokens`.
    pub fn new(name: impl Into<String>, version: impl Into<String>, tokens: Tokens) -> Self {
        Shell {
            name: name.into(),
            version: version.into(),
            tokens,
            fonts: Vec::new(),
            fonts_installed: false,
            applied: None,
            mode: Mode::Light,
            chrome: Chrome::default(),
            layout: Layout::default(),
            autosave: AutoSave::default(),
            updates: Updates { check_at_start: true, ..Updates::default() },
            #[cfg(not(target_arch = "wasm32"))]
            updater: None,
            keys: KeyMap::default(),
            recent: Vec::new(),
            reopen_last: false,
            last_open: Vec::new(),
            user_name: String::new(),
            tour_seen: false,
            dialogs: Dialogs::default(),
            message: String::new(),
            options: ShellOptions::default(),
            mac: cfg!(target_os = "macos"),
            queue: Vec::new(),
            file_asks: Vec::new(),
            close_requested: false,
            rails: Vec::new(),
            floating: Vec::new(),
        }
    }

    /// With the interface fonts the tokens name (installed on the first frame).
    pub fn with_fonts(mut self, faces: Vec<FontFace>) -> Self {
        self.fonts = faces;
        self.fonts_installed = false;
        self
    }

    /// With these options.
    pub fn with_options(mut self, options: ShellOptions) -> Self {
        self.options = options;
        self
    }

    /// With updates from `source`'s GitHub releases (desktop builds).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_updates(mut self, source: crate::updates::desktop::ReleaseSource) -> Self {
        let updater = crate::updates::desktop::Updater::new(source);
        updater.read_outcome(&mut self.updates);
        self.updater = Some(updater);
        self
    }

    /// The design tokens.
    pub fn tokens(&self) -> &Tokens {
        &self.tokens
    }

    /// Use other design tokens (applied on the next frame).
    pub fn set_tokens(&mut self, tokens: Tokens) {
        self.tokens = tokens;
        self.applied = None;
    }

    /// The theme in use.
    pub fn theme(&self) -> Theme {
        self.tokens.theme(self.mode)
    }

    /// Queue `cmd` to run once this frame is drawn: the shell's own commands by the shell, the
    /// app's through [`App::run`].
    pub fn run(&mut self, cmd: Cmd) {
        self.queue.push(cmd);
    }

    /// The commands queued and not run yet.
    pub fn queued(&self) -> &[Cmd] {
        &self.queue
    }

    /// Put `path` first among the recent documents.
    pub fn add_recent(&mut self, path: &str) {
        add_recent(&mut self.recent, path);
    }

    /// Ask "save changes first?" about document `name` before `doing` ("closing", "opening
    /// another document"). The answer comes back as [`crate::commands::ids::UNSAVED_ANSWER`] with
    /// `tag`.
    pub fn ask_unsaved(&mut self, name: impl Into<String>, doing: impl Into<String>, tag: impl Into<String>) {
        self.dialogs.unsaved = Some(UnsavedPrompt { name: name.into(), doing: doing.into(), tag: tag.into() });
    }

    /// Offer the copies crash recovery kept. `can_restore`: the open document is untouched, so a
    /// copy can be restored into it.
    pub fn offer_recovery(&mut self, copies: Vec<RecoveryCopy>, can_restore: bool) {
        self.dialogs.show_recovery = !copies.is_empty();
        self.dialogs.recovery = copies;
        self.dialogs.can_restore = can_restore;
    }

    /// Open the File page at `page`.
    pub fn open_file_page(&mut self, page: Page) {
        self.chrome.file_page = Some(page);
    }

    /// Whether the window should close (to start an installed update).
    pub fn close_requested(&self) -> bool {
        self.close_requested
    }

    /// Call when the app is about to exit: starts an installed update's helper.
    pub fn finish(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(u) = self.updater.as_mut() {
            u.finish();
        }
    }

    /// The shell's preferences as they are now.
    pub fn prefs(&self) -> ShellPrefs {
        ShellPrefs {
            theme: self.mode,
            autosave: crate::prefs::AutoSavePrefs {
                enabled: self.autosave.enabled,
                minutes: (self.autosave.every.as_secs() / 60).clamp(1, u64::from(crate::prefs::MAX_AUTOSAVE_MINUTES)) as u32,
                idle_seconds: self.autosave.idle.as_secs().clamp(1, u64::from(crate::prefs::MAX_IDLE_SECONDS)) as u32,
            },
            recent: self.recent.clone(),
            reopen_last: self.reopen_last,
            last_open: self.last_open.clone(),
            user_name: self.user_name.clone(),
            keys: self.keys.user().clone(),
            layout: self.layout.clone(),
            ribbon_collapsed: self.chrome.collapsed,
            tour_seen: self.tour_seen || self.chrome.tour_done,
            check_updates: self.updates.check_at_start,
        }
    }

    /// Put preferences into effect, bounded; `panels` are the app's (a layout naming panels the
    /// app no longer has is repaired).
    pub fn apply_prefs(&mut self, prefs: &ShellPrefs, panels: &[PanelSpec]) {
        let p = prefs.clone().bounded();
        self.mode = p.theme;
        self.autosave.enabled = p.autosave.enabled;
        self.autosave.every = p.autosave_every();
        self.autosave.idle = p.autosave_idle();
        self.recent = p.recent.clone();
        self.reopen_last = p.reopen_last;
        self.last_open = p.last_open.clone();
        self.user_name = p.user_name.clone();
        self.keys.set_user(&p.keys);
        self.layout = p.layout.clone().repaired(panels);
        self.chrome.collapsed = p.ribbon_collapsed;
        self.tour_seen = p.tour_seen;
        self.updates.check_at_start = p.check_updates;
        if !p.tour_seen && !self.options.tour.is_empty() {
            self.chrome.tour = Some(0);
        }
    }

    /// Draw one frame: the chrome, the panels, the document (or the File page), the dialogs;
    /// then run the commands queued, and auto-save if a save is due.
    pub fn ui<A: App>(&mut self, ui: &mut Ui, app: &mut A) {
        let ctx = ui.ctx().clone();
        self.apply_theme(&ctx);
        self.mac = ctx.os() == egui::os::OperatingSystem::Mac;
        self.chrome.drawn.clear();
        self.chrome.areas.clear();
        self.rails.clear();
        let floating = std::mem::take(&mut self.floating);
        let tabs = app.ribbon_tabs();
        crate::ribbon::handle_keytips(self, ui, &tabs);
        if self.options.shortcuts {
            self.dispatch_keys(ui, &*app);
        }
        crate::ribbon::show(self, ui, app, &tabs);
        if self.options.status_bar {
            crate::chrome::status_bar(self, ui, app);
        }
        if self.chrome.file_page.is_some() {
            egui::CentralPanel::default().show(ui, |ui| crate::backstage::show(self, ui, app));
        } else {
            crate::sidebar::show(self, ui, app);
            let start = self.options.start_window && !app.has_document();
            let doc = egui::CentralPanel::default()
                .show(ui, |ui| if start { crate::start::start_window(ui, self, app) } else { app.document_ui(ui, self) });
            self.close_floating_on_press(&ctx, doc.response.rect, &floating);
        }
        crate::dialogs::show(self, &ctx, app);
        crate::updates::window(self, &ctx);
        if self.chrome.file_page.is_none() {
            crate::ribbon::tour(self, &ctx);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(u) = self.updater.as_mut()
            && u.tick(&mut self.updates, &ctx)
        {
            self.close_requested = true;
        }
        if !self.queue.is_empty() {
            // What the commands change shows in the next frame.
            ctx.request_repaint();
        }
        self.run_queue(app);
        self.tick_autosave(&ctx, app, Instant::now());
    }

    fn apply_theme(&mut self, ctx: &egui::Context) {
        if !self.fonts_installed {
            jcj_theme::install_fonts(ctx, &self.tokens.fonts, &self.fonts);
            self.fonts_installed = true;
        }
        let theme = self.theme();
        if self.applied != Some(theme) {
            jcj_theme::apply(ctx, &theme);
            self.applied = Some(theme);
        }
    }

    /// Run the commands bound to the keys pressed this frame (keys with Mod, Ctrl or Alt, and
    /// function keys), taking those key presses from the document.
    fn dispatch_keys(&mut self, ui: &mut Ui, cmds: &dyn crate::commands::Commands) {
        let mac = self.mac;
        let pressed: Vec<(usize, String)> = ui.input(|i| {
            i.events
                .iter()
                .enumerate()
                .filter_map(|(n, e)| match e {
                    Event::Key { key, pressed: true, modifiers, .. } => shortcut(*key, *modifiers, mac).map(|s| (n, s)),
                    _ => None,
                })
                .filter(|(_, s)| parse_shortcut(s).is_some_and(|p| p.is_command_key()))
                .collect()
        });
        if pressed.is_empty() {
            return;
        }
        let mut map = KeyMap::from_commands(crate::dialogs::all_commands(cmds).iter());
        map.set_user(self.keys.user());
        let mut taken = Vec::new();
        for (n, s) in pressed {
            if let Some(id) = map.bound_to(&s) {
                self.run(Cmd::new(id));
                taken.push(n);
            }
        }
        if !taken.is_empty() {
            ui.input_mut(|i| {
                let mut n = 0;
                i.events.retain(|_| {
                    let keep = !taken.contains(&n);
                    n += 1;
                    keep
                });
            });
        }
    }

    /// A press in the document (not on a rail or a floating panel) closes floating panels.
    fn close_floating_on_press(&mut self, ctx: &egui::Context, doc: Rect, floating: &[Rect]) {
        if floating.is_empty() {
            return;
        }
        let press = ctx.input(|i| if i.pointer.any_pressed() { i.pointer.interact_pos() } else { None });
        if let Some(p) = press
            && doc.contains(p)
            && !floating.iter().chain(&self.rails).any(|r| r.contains(p))
        {
            self.layout.close_unpinned();
        }
    }

    /// Show a file dialog of the system's, and run `then` with the choice in its parameters:
    /// `"path"`, the file, folder or place to save chosen, and `"paths"`, every file chosen
    /// (text; a path that is not Unicode loses the characters that are not). Closing the dialog
    /// without choosing runs nothing. The dialog opens after the frame, when the app's window
    /// calls [`crate::files::FileDialogs::show`]; see [`crate::files`]. `false` when too many
    /// dialogs wait already ([`crate::files::MAX_WAITING`]) and this one was dropped.
    pub fn ask_file(&mut self, ask: crate::files::FileAsk, then: Cmd) -> bool {
        self.push_file_ask(ask, then, None)
    }

    /// [`Shell::ask_file`], with a command to run when the dialog is closed without choosing
    /// (a step that waited on the answer can then be given up).
    pub fn ask_file_or(&mut self, ask: crate::files::FileAsk, then: Cmd, cancelled: Cmd) -> bool {
        self.push_file_ask(ask, then, Some(cancelled))
    }

    /// How many file dialogs were asked for and not yet taken by the window to show.
    pub fn file_asks_waiting(&self) -> usize {
        self.file_asks.len()
    }

    fn push_file_ask(&mut self, ask: crate::files::FileAsk, chosen: Cmd, cancelled: Option<Cmd>) -> bool {
        if self.file_asks.len() >= crate::files::MAX_WAITING {
            return false;
        }
        self.file_asks.push(crate::files::Asked { ask, then: crate::files::Then { chosen, cancelled } });
        true
    }

    /// The file dialogs asked for, for the window to show.
    pub(crate) fn take_file_asks(&mut self) -> Vec<crate::files::Asked> {
        std::mem::take(&mut self.file_asks)
    }

    /// Run the queued commands (and any they queue, a few rounds deep).
    pub fn run_queue<A: App>(&mut self, app: &mut A) {
        for _ in 0..4 {
            let queue = std::mem::take(&mut self.queue);
            if queue.is_empty() {
                return;
            }
            for cmd in queue {
                if cmd.is_shell() {
                    self.run_shell(&cmd, app);
                } else {
                    match app.run(&cmd, self) {
                        Ok(()) => {}
                        Err(e) => self.message = e,
                    }
                }
            }
        }
    }

    fn run_shell<A: App>(&mut self, cmd: &Cmd, app: &A) {
        let param = |k: &str| cmd.params.get(k).and_then(Value::as_str).map(str::to_string);
        match cmd.id.as_str() {
            shell_ids::FILE_PAGE => self.chrome.file_page = Some(Page::from_key(param("page").as_deref().unwrap_or("new"))),
            shell_ids::CLOSE_FILE_PAGE => self.chrome.file_page = None,
            shell_ids::PANEL => {
                if let Some(id) = param("panel") {
                    self.layout.toggle(&app.panels(), &id);
                }
            }
            shell_ids::SIDE => {
                if let Some(side) = param("side").as_deref().and_then(Side::from_key) {
                    self.layout.toggle_side(&app.panels(), side);
                }
            }
            shell_ids::RIBBON => {
                self.chrome.collapsed = !self.chrome.collapsed;
                self.chrome.peek = false;
            }
            shell_ids::KEYTIPS => self.chrome.toggle_keytips(),
            shell_ids::PALETTE => {
                self.dialogs.palette = if self.dialogs.palette.is_some() { None } else { Some(Palette::default()) };
            }
            shell_ids::PREFERENCES => self.chrome.file_page = Some(Page::Preferences),
            shell_ids::KEYS => self.chrome.file_page = Some(Page::Keys),
            shell_ids::ABOUT => self.chrome.file_page = Some(Page::About),
            shell_ids::CHECK_FOR_UPDATES => self.updates.check_now(),
            shell_ids::THEME => {
                self.mode = match param("mode").as_deref() {
                    Some("light") => Mode::Light,
                    Some("dark") => Mode::Dark,
                    Some("extra_dark") => Mode::ExtraDark,
                    _ if self.mode == Mode::Light => Mode::Dark,
                    _ => Mode::Light,
                }
            }
            shell_ids::TOUR => {
                if !self.options.tour.is_empty() {
                    self.chrome.tour = Some(0);
                }
            }
            other => self.message = tr_with("There is no command {command}.", &[("command", other)]),
        }
    }

    fn tick_autosave<A: App>(&mut self, ctx: &egui::Context, app: &mut A, now: Instant) {
        self.autosave.observe(app.revision(), now);
        match self.autosave.tick(now, app.is_dirty(), app.has_file(), || app.autosave()) {
            Tick::Waiting(at) => ctx.request_repaint_after(at.saturating_duration_since(now)),
            Tick::Failed(why) => self.message = tr_with("Auto-save failed: {why}", &[("why", &why)]),
            Tick::Saved | Tick::Idle => {}
        }
    }
}

// At the end of the file: the catalog check reads a source file up to its first test-only line.
#[cfg(test)]
impl Shell {
    /// As if an installed update had asked for a restart (tests of what a window does then).
    pub(crate) fn request_close(&mut self) {
        self.close_requested = true;
    }
}
