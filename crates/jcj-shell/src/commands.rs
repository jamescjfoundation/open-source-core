// SPDX-License-Identifier: MIT OR Apache-2.0
//! Commands as the shell sees them. Every control in the shell runs a command by id with JSON
//! parameters, the way apps' command registries (and agents) run them. The app describes its
//! commands through [`Commands`]; the shell's own commands (ids starting `shell.`) it handles
//! itself.

use jcj_i18n::tl;
use serde_json::{Value, json};

/// A command to run: its id and parameters.
#[derive(Clone, Debug, PartialEq)]
pub struct Cmd {
    /// The command's id, such as `file.save`.
    pub id: String,
    /// Its parameters (`{}` for none).
    pub params: Value,
}

impl Cmd {
    /// Command `id` with no parameters.
    pub fn new(id: impl Into<String>) -> Self {
        Cmd { id: id.into(), params: json!({}) }
    }

    /// Command `id` with `params`.
    pub fn with(id: impl Into<String>, params: Value) -> Self {
        Cmd { id: id.into(), params }
    }

    /// Whether this is one of the shell's own commands.
    pub fn is_shell(&self) -> bool {
        self.id.starts_with("shell.")
    }
}

/// What the shell shows of a command: in tooltips, menus, the command palette and the Keys page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandInfo {
    /// Its id.
    pub id: String,
    /// Its name, as menus show it ("Save As…").
    pub label: String,
    /// Its keys: portable shortcuts ([`crate::keys`]), alternatives separated by ` / `.
    pub shortcut: String,
    /// Where it lives ("File", "Edit › Find"); the palette searches it too.
    pub location: String,
    /// An icon name from [`crate::icons`], or empty.
    pub icon: String,
    /// Whether it needs parameters (the palette and the Keys page list only those that don't).
    pub takes_params: bool,
}

impl CommandInfo {
    /// A command with an id, a label and keys.
    pub fn new(id: impl Into<String>, label: impl Into<String>, shortcut: impl Into<String>) -> Self {
        CommandInfo { id: id.into(), label: label.into(), shortcut: shortcut.into(), ..CommandInfo::default() }
    }

    /// The first of its shortcuts, or empty.
    pub fn first_shortcut(&self) -> &str {
        self.shortcut.split(" / ").next().unwrap_or("").trim()
    }
}

/// The app's commands, as the shell asks about them while drawing.
pub trait Commands {
    /// What to show for command `id`, if the app has it.
    fn command(&self, id: &str) -> Option<CommandInfo>;

    /// Why command `id` cannot run now, if it cannot (its controls are greyed out and say so).
    fn why_disabled(&self, id: &str) -> Option<String> {
        let _ = id;
        None
    }

    /// Every command, for the command palette and the Keys page.
    fn all_commands(&self) -> Vec<CommandInfo> {
        Vec::new()
    }
}

/// No commands: for drawing shell parts without an app.
impl Commands for () {
    fn command(&self, _: &str) -> Option<CommandInfo> {
        None
    }
}

/// Command ids the shell's own controls run that the app provides. An app that has no such
/// command leaves the control greyed out.
pub mod ids {
    /// A new document.
    pub const NEW: &str = "file.new";
    /// Open a document (the app shows its file dialog).
    pub const OPEN: &str = "file.open";
    /// Start a new document from one of the app's templates: `{"template": id}`.
    pub const NEW_FROM_TEMPLATE: &str = "file.newFromTemplate";
    /// Open the document at `{"path": "..."}` (recent files).
    pub const OPEN_PATH: &str = "file.openPath";
    /// Save.
    pub const SAVE: &str = "file.save";
    /// Save in a new file.
    pub const SAVE_AS: &str = "file.saveAs";
    /// Print.
    pub const PRINT: &str = "file.print";
    /// Restore (`{"restore": id}`) or discard (`{"discard": id}`) a copy kept by crash recovery.
    pub const RECOVER: &str = "file.recover";
    /// The answer to the unsaved-changes question: `{"answer": "save" | "dontSave" | "cancel",
    /// "then": <the tag the app asked with>}`.
    pub const UNSAVED_ANSWER: &str = "file.unsavedAnswer";
    /// Undo.
    pub const UNDO: &str = "edit.undo";
    /// Redo.
    pub const REDO: &str = "edit.redo";
}

/// The shell's own commands. Apps may bind keys to them and put them on their ribbon tabs.
pub mod shell_ids {
    /// Open the File page (`{"page": "new" | "open" | ... | "<app page id>"}`, default `new`).
    pub const FILE_PAGE: &str = "shell.filePage";
    /// Close the File page.
    pub const CLOSE_FILE_PAGE: &str = "shell.closeFilePage";
    /// Show or hide a panel (`{"panel": id}`).
    pub const PANEL: &str = "shell.panel";
    /// Show or hide a side's panel (`{"side": "left" | "right" | "bottom"}`).
    pub const SIDE: &str = "shell.side";
    /// Show the ribbon's groups, or its tabs only.
    pub const RIBBON: &str = "shell.ribbon";
    /// Show key tips on the ribbon.
    pub const KEYTIPS: &str = "shell.keytips";
    /// The command palette.
    pub const PALETTE: &str = "shell.palette";
    /// The Preferences page.
    pub const PREFERENCES: &str = "shell.preferences";
    /// The Keys page.
    pub const KEYS: &str = "shell.keys";
    /// The About page.
    pub const ABOUT: &str = "shell.about";
    /// Check for a newer release now.
    pub const CHECK_FOR_UPDATES: &str = "shell.checkForUpdates";
    /// Switch theme (`{"mode": "light" | "dark" | "extra_dark"}`; none: light and dark in turn).
    pub const THEME: &str = "shell.theme";
    /// Show the first-run tour again.
    pub const TOUR: &str = "shell.tour";
}

/// The shell's own commands as the palette and Keys page list them.
pub fn shell_commands() -> Vec<CommandInfo> {
    use shell_ids::{ABOUT, CHECK_FOR_UPDATES, CLOSE_FILE_PAGE, FILE_PAGE, KEYS, KEYTIPS, PALETTE, PANEL, PREFERENCES, RIBBON, SIDE, THEME, TOUR};
    let c = |id: &str, label: &str, shortcut: &str, icon: &str| CommandInfo {
        id: id.into(),
        label: label.into(),
        shortcut: shortcut.into(),
        location: "Window".into(),
        icon: icon.into(),
        takes_params: false,
    };
    vec![
        c(FILE_PAGE, tl("File Page"), "", "file"),
        c(RIBBON, tl("Show or Hide the Ribbon"), "Mod+F1", "chevronUp"),
        c(KEYTIPS, tl("Key Tips"), "F10", "launcher"),
        c(PALETTE, tl("Command Palette"), "Mod+Shift+P", "search"),
        c(PREFERENCES, tl("Preferences"), "Mod+,", "options"),
        c(KEYS, tl("Keys"), "", "macros"),
        c(ABOUT, tl("About"), "", "help"),
        c(CHECK_FOR_UPDATES, tl("Check for Updates"), "", "recover"),
        c(THEME, tl("Light or Dark Theme"), "", "darkMode"),
        c(TOUR, tl("Show the Tour"), "", "help"),
        c(CLOSE_FILE_PAGE, tl("Close the File Page"), "", "close"),
        CommandInfo { takes_params: true, ..c(PANEL, tl("Show or Hide a Panel"), "", "navPane") },
        CommandInfo { takes_params: true, ..c(SIDE, tl("Show or Hide a Side's Panel"), "", "navPane") },
    ]
}

/// What to show for `id`: the shell's own command, else the app's.
pub fn info(cmds: &dyn Commands, id: &str) -> Option<CommandInfo> {
    if id.starts_with("shell.") {
        return shell_commands().into_iter().find(|c| c.id == id);
    }
    cmds.command(id)
}

/// Commands matching `query` (every word in the id, label or location), those without
/// parameters only, at most `max`.
pub fn palette_matches(all: &[CommandInfo], query: &str, max: usize) -> Vec<CommandInfo> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    all.iter()
        .filter(|c| !c.takes_params)
        .filter(|c| {
            let hay = format!("{} {} {}", c.id, c.label, c.location).to_lowercase();
            words.iter().all(|w| hay.contains(w.as_str()))
        })
        .take(max)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_palette_finds_commands_by_every_word() {
        let all = vec![
            CommandInfo { location: "File".into(), ..CommandInfo::new("file.save", "Save", "Mod+S") },
            CommandInfo { location: "File".into(), ..CommandInfo::new("file.saveAs", "Save As…", "Mod+Shift+S") },
            CommandInfo { takes_params: true, ..CommandInfo::new("file.openPath", "Open Path", "") },
            CommandInfo::new("edit.undo", "Undo", "Mod+Z"),
        ];
        let ids = |q: &str| palette_matches(&all, q, 10).into_iter().map(|c| c.id).collect::<Vec<_>>();
        assert_eq!(ids("save"), ["file.save", "file.saveAs"]);
        assert_eq!(ids("file as"), ["file.saveAs"]);
        assert_eq!(ids("open"), Vec::<String>::new(), "commands with parameters are left out");
        assert_eq!(ids("").len(), 3);
        assert_eq!(palette_matches(&all, "", 1).len(), 1);
    }

    #[test]
    fn shell_commands_are_known_without_the_app() {
        assert_eq!(info(&(), shell_ids::PALETTE).map(|c| c.label), Some("Command Palette".into()));
        assert_eq!(info(&(), "file.save"), None);
        assert!(Cmd::new(shell_ids::RIBBON).is_shell());
        assert_eq!(CommandInfo::new("a", "A", "Mod+Y / Mod+Shift+Z").first_shortcut(), "Mod+Y");
    }
}
