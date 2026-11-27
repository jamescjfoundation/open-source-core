// SPDX-License-Identifier: MIT OR Apache-2.0
//! Keys: key presses to shortcut strings, the portable form commands declare (`Mod+Shift+Left`,
//! `Enter`, `Mod+3`), and the map from shortcuts to commands.
//!
//! `Mod` is Command on macOS and Ctrl elsewhere. On macOS the platform's own text-editing keys
//! that have a portable form are translated to it first: Command+Left/Right goes to the line's
//! ends (`Home`, `End`), Command+Up/Down to the document's (`Mod+Home`, `Mod+End`). Option+Left
//! and Option+Right stay `Alt+Left` and `Alt+Right`, the keys that move by word on every system:
//! a command that moves by word declares them (`Mod+Left / Alt+Left`), as the engine's do.
//!
//! A [`KeyMap`] holds the commands' own keys and the user's: the user's come first, and a user
//! key bound to nothing (`""`) turns a command's key off.
//!
//! A document view that takes key presses itself asks [`KeyMap::command_for`] what each runs.
//! It adds the one rule every editor shares for a shifted key that has no command of its own
//! ([`KeyMap::shifted`]): Shift with a key that moves the caret extends the selection, and
//! Shift+Backspace still deletes back.

use std::collections::BTreeMap;

use egui::{Key, Modifiers};

use crate::commands::{Cmd, CommandInfo};

/// The prefix of the ids of commands that move the caret (`caret.left`, `caret.wordRight`): they
/// take `{"extend": true}` to move the selection's end and leave its start.
pub const CARET_COMMANDS: &str = "caret.";
/// The command that deletes back from the caret.
pub const BACKSPACE: &str = "text.backspace";

/// Every key that can be part of a shortcut, with its portable name.
const KEYS: [(Key, &str); 66] = [
    (Key::ArrowLeft, "Left"),
    (Key::ArrowRight, "Right"),
    (Key::ArrowUp, "Up"),
    (Key::ArrowDown, "Down"),
    (Key::Home, "Home"),
    (Key::End, "End"),
    (Key::PageUp, "PageUp"),
    (Key::PageDown, "PageDown"),
    (Key::Enter, "Enter"),
    (Key::Tab, "Tab"),
    (Key::Backspace, "Backspace"),
    (Key::Delete, "Delete"),
    (Key::Escape, "Escape"),
    (Key::Comma, ","),
    (Key::Num0, "0"),
    (Key::Num1, "1"),
    (Key::Num2, "2"),
    (Key::Num3, "3"),
    (Key::Num4, "4"),
    (Key::Num5, "5"),
    (Key::Num6, "6"),
    (Key::Num7, "7"),
    (Key::Num8, "8"),
    (Key::Num9, "9"),
    (Key::A, "A"),
    (Key::B, "B"),
    (Key::C, "C"),
    (Key::D, "D"),
    (Key::E, "E"),
    (Key::F, "F"),
    (Key::G, "G"),
    (Key::H, "H"),
    (Key::I, "I"),
    (Key::J, "J"),
    (Key::K, "K"),
    (Key::L, "L"),
    (Key::M, "M"),
    (Key::N, "N"),
    (Key::O, "O"),
    (Key::P, "P"),
    (Key::Q, "Q"),
    (Key::R, "R"),
    (Key::S, "S"),
    (Key::T, "T"),
    (Key::U, "U"),
    (Key::V, "V"),
    (Key::W, "W"),
    (Key::X, "X"),
    (Key::Y, "Y"),
    (Key::Z, "Z"),
    (Key::F1, "F1"),
    (Key::F2, "F2"),
    (Key::F3, "F3"),
    (Key::F4, "F4"),
    (Key::F5, "F5"),
    (Key::F6, "F6"),
    (Key::F7, "F7"),
    (Key::F8, "F8"),
    (Key::F9, "F9"),
    (Key::F10, "F10"),
    (Key::F11, "F11"),
    (Key::F12, "F12"),
    (Key::Space, "Space"),
    (Key::Minus, "-"),
    (Key::Equals, "="),
    (Key::Slash, "/"),
];

/// The portable name of `key`, if it can be part of a shortcut.
pub fn key_name(key: Key) -> Option<&'static str> {
    KEYS.iter().find(|(k, _)| *k == key).map(|(_, n)| *n)
}

/// The shortcut string for a key press: modifiers in the order `Mod`, `Ctrl` (macOS only,
/// where Control is not `Mod`), `Alt`, `Shift`, then the key. `mac` applies macOS's own keys.
pub fn shortcut(key: Key, m: Modifiers, mac: bool) -> Option<String> {
    let name = key_name(key)?;
    let (mut cmd, alt, shift) = (m.command, m.alt, m.shift);
    let ctrl = mac && m.ctrl;
    let mut name = name;
    if mac {
        match (cmd, alt, name) {
            // Option+Left/Right stay `Alt+Left`/`Alt+Right`: by word, on every system. They
            // are not made `Mod+Left`, which the engine binds to the line's start on macOS.
            // Command+Left/Right: the line's ends.
            (true, false, "Left") => {
                cmd = false;
                name = "Home";
            }
            (true, false, "Right") => {
                cmd = false;
                name = "End";
            }
            // Command+Up/Down: the document's ends.
            (true, false, "Up") => name = "Home",
            (true, false, "Down") => name = "End",
            _ => {}
        }
    }
    Some(Shortcut { cmd, ctrl, alt, shift, key: name.to_string() }.to_string())
}

/// A shortcut taken apart.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Shortcut {
    /// `Mod`: Command on macOS, Ctrl elsewhere.
    pub cmd: bool,
    /// `Ctrl` on macOS (elsewhere Ctrl is `Mod`).
    pub ctrl: bool,
    /// `Alt` (Option on macOS).
    pub alt: bool,
    /// `Shift`.
    pub shift: bool,
    /// The key's portable name.
    pub key: String,
}

impl Shortcut {
    /// Whether the shortcut has a modifier other than Shift, or is a function key: such keys
    /// never type text, so the shell may take them before the document sees them.
    pub fn is_command_key(&self) -> bool {
        self.cmd || self.ctrl || self.alt || (self.key.len() > 1 && self.key.starts_with('F') && self.key[1..].bytes().all(|b| b.is_ascii_digit()))
    }
}

impl std::fmt::Display for Shortcut {
    /// The canonical form: `Mod+Ctrl+Alt+Shift+Key`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (on, part) in [(self.cmd, "Mod+"), (self.ctrl, "Ctrl+"), (self.alt, "Alt+"), (self.shift, "Shift+")] {
            if on {
                f.write_str(part)?;
            }
        }
        f.write_str(&self.key)
    }
}

/// Read a shortcut written by a person or a command (`mod+shift+z`, `Shift+Mod+Z`, `Cmd+S`,
/// `Ctrl+Alt+Delete`). Modifiers may come in any order and any case; `Cmd`, `Command` and
/// `CmdOrCtrl` are `Mod`, and `Option` is `Alt`. Returns `None` for an unknown key or modifier,
/// a repeated modifier, or no key.
pub fn parse_shortcut(text: &str) -> Option<Shortcut> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    // The key is after the last `+`, unless the key itself is `+` (`Mod++`) or the text is `+`.
    let (mods, key) = match text.strip_suffix("++") {
        Some(rest) => (rest, "+"),
        None => match text.rsplit_once('+') {
            Some((m, k)) => (m, k),
            None => ("", text),
        },
    };
    let key = key.trim();
    let canonical = KEYS.iter().map(|(_, n)| *n).find(|n| n.eq_ignore_ascii_case(key))?;
    let mut s = Shortcut { cmd: false, ctrl: false, alt: false, shift: false, key: canonical.to_string() };
    if !mods.is_empty() {
        for part in mods.split('+') {
            let flag = match part.trim().to_ascii_lowercase().as_str() {
                "mod" | "cmd" | "command" | "cmdorctrl" => &mut s.cmd,
                "ctrl" | "control" => &mut s.ctrl,
                "alt" | "option" | "opt" => &mut s.alt,
                "shift" => &mut s.shift,
                _ => return None,
            };
            if *flag {
                return None;
            }
            *flag = true;
        }
    }
    Some(s)
}

/// The canonical form of a shortcut string, or `None` if it does not read.
pub fn normalize(text: &str) -> Option<String> {
    parse_shortcut(text).map(|s| s.to_string())
}

/// Most keys of the user's own kept.
pub const MAX_USER_KEYS: usize = 500;

/// Shortcuts to commands: the commands' own keys, and the user's on top.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyMap {
    defaults: BTreeMap<String, String>,
    user: BTreeMap<String, String>,
}

impl KeyMap {
    /// A map of the keys `commands` declare (each alternative of `a / b`). Keys that do not read
    /// are skipped; when two commands declare one key, the first keeps it.
    pub fn from_commands<'a>(commands: impl IntoIterator<Item = &'a CommandInfo>) -> Self {
        let mut defaults = BTreeMap::new();
        for c in commands {
            for alt in c.shortcut.split(" / ").filter_map(normalize) {
                defaults.entry(alt).or_insert_with(|| c.id.clone());
            }
        }
        KeyMap { defaults, user: BTreeMap::new() }
    }

    /// The user's own keys: shortcut → command id (`""` turns a key off).
    pub fn user(&self) -> &BTreeMap<String, String> {
        &self.user
    }

    /// Replace the user's own keys (from the preferences). Keys that do not read are dropped,
    /// and at most [`MAX_USER_KEYS`] are kept.
    pub fn set_user(&mut self, keys: &BTreeMap<String, String>) {
        self.user =
            keys.iter().filter(|(_, id)| id.len() <= 120).filter_map(|(k, id)| Some((normalize(k)?, id.clone()))).take(MAX_USER_KEYS).collect();
    }

    /// Bind `shortcut` to command `id` for this user (`""`: the key does nothing). Returns
    /// whether the shortcut read.
    pub fn bind(&mut self, shortcut: &str, id: &str) -> bool {
        let Some(k) = normalize(shortcut) else { return false };
        if self.user.len() >= MAX_USER_KEYS && !self.user.contains_key(&k) {
            return false;
        }
        self.user.insert(k, id.to_string());
        true
    }

    /// Forget the user's keys for command `id`.
    pub fn reset(&mut self, id: &str) {
        self.user.retain(|_, c| c != id);
    }

    /// The command `shortcut` runs, if any: the user's key first, then the commands' own.
    pub fn bound_to(&self, shortcut: &str) -> Option<&str> {
        let k = normalize(shortcut)?;
        match self.user.get(&k) {
            Some(id) => Some(id.as_str()).filter(|id| !id.is_empty()),
            None => self.defaults.get(&k).map(String::as_str),
        }
    }

    /// What a shifted key with no command of its own does, by the key without Shift: when that
    /// runs a caret movement ([`CARET_COMMANDS`]), the same movement extending the selection
    /// (`{"extend": true}`); when it deletes back ([`BACKSPACE`]), the same. `None` for a key
    /// without Shift, and for one whose unshifted key runs anything else or nothing.
    ///
    /// This is the rule alone: it does not ask whether `shortcut` is bound itself.
    /// [`KeyMap::command_for`] does both.
    pub fn shifted(&self, shortcut: &str) -> Option<Cmd> {
        let mut key = parse_shortcut(shortcut)?;
        if !key.shift {
            return None;
        }
        key.shift = false;
        let id = self.bound_to(&key.to_string())?;
        if id.starts_with(CARET_COMMANDS) {
            Some(Cmd::with(id, serde_json::json!({ "extend": true })))
        } else if id == BACKSPACE {
            Some(Cmd::new(id))
        } else {
            None
        }
    }

    /// The command a key press runs: the one bound to `shortcut` ([`KeyMap::bound_to`]), or
    /// else what the rule for shifted keys gives ([`KeyMap::shifted`]). A key the user turned
    /// off runs nothing.
    pub fn command_for(&self, shortcut: &str) -> Option<Cmd> {
        let key = normalize(shortcut)?;
        match self.user.get(&key) {
            Some(id) => (!id.is_empty()).then(|| Cmd::new(id.as_str())),
            None => self.defaults.get(&key).map(|id| Cmd::new(id.as_str())).or_else(|| self.shifted(&key)),
        }
    }

    /// The keys that run command `id` now, the user's first.
    pub fn keys_for(&self, id: &str) -> Vec<String> {
        let mut v: Vec<String> = self.user.iter().filter(|(_, c)| c.as_str() == id).map(|(k, _)| k.clone()).collect();
        v.extend(self.defaults.iter().filter(|(k, c)| c.as_str() == id && !self.user.contains_key(*k)).map(|(k, _)| k.clone()));
        v
    }
}

/// How a shortcut is shown on this platform: `Mod` as ⌘ on macOS and Ctrl elsewhere.
pub fn display(shortcut: &str, mac: bool) -> String {
    let Some(s) = parse_shortcut(shortcut) else { return shortcut.to_string() };
    if mac {
        let mut out = String::new();
        for (on, sym) in [(s.ctrl, "⌃"), (s.alt, "⌥"), (s.shift, "⇧"), (s.cmd, "⌘")] {
            if on {
                out.push_str(sym);
            }
        }
        out.push_str(&s.key);
        out
    } else {
        s.to_string().replace("Mod+", "Ctrl+")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(command: bool, alt: bool, shift: bool) -> Modifiers {
        Modifiers { alt, ctrl: command, shift, mac_cmd: false, command }
    }

    #[test]
    fn portable_shortcuts() {
        assert_eq!(shortcut(Key::Enter, Modifiers::NONE, false).as_deref(), Some("Enter"));
        assert_eq!(shortcut(Key::Num3, m(true, false, false), false).as_deref(), Some("Mod+3"));
        assert_eq!(shortcut(Key::Z, m(true, false, true), false).as_deref(), Some("Mod+Shift+Z"));
        assert_eq!(shortcut(Key::L, m(true, true, false), false).as_deref(), Some("Mod+Alt+L"));
        assert_eq!(shortcut(Key::ArrowLeft, m(true, false, true), false).as_deref(), Some("Mod+Shift+Left"));
        assert_eq!(shortcut(Key::Comma, m(true, false, false), false).as_deref(), Some("Mod+,"));
        assert_eq!(shortcut(Key::Backtick, Modifiers::NONE, false), None);
    }

    #[test]
    fn macos_text_keys_become_portable_ones() {
        let opt = Modifiers { alt: true, ..Modifiers::NONE };
        let cmd = Modifiers { command: true, mac_cmd: true, ..Modifiers::NONE };
        let cmd_shift = Modifiers { shift: true, ..cmd };
        assert_eq!(shortcut(Key::ArrowLeft, opt, true).as_deref(), Some("Alt+Left"));
        assert_eq!(shortcut(Key::ArrowRight, Modifiers { shift: true, ..opt }, true).as_deref(), Some("Alt+Shift+Right"));
        assert_eq!(shortcut(Key::ArrowLeft, cmd, true).as_deref(), Some("Home"));
        assert_eq!(shortcut(Key::ArrowRight, cmd_shift, true).as_deref(), Some("Shift+End"));
        assert_eq!(shortcut(Key::ArrowUp, cmd, true).as_deref(), Some("Mod+Home"));
        assert_eq!(shortcut(Key::ArrowDown, cmd_shift, true).as_deref(), Some("Mod+Shift+End"));
        assert_eq!(shortcut(Key::Num1, cmd, true).as_deref(), Some("Mod+1"));
        // Control on a Mac is its own modifier, not Mod.
        assert_eq!(shortcut(Key::A, Modifiers { ctrl: true, ..Modifiers::NONE }, true).as_deref(), Some("Ctrl+A"));
    }

    /// The engine declares its caret keys per system (`mac_or` in its caret commands, and its
    /// Promote and Demote on Ctrl+Shift+Left/Right on macOS, Alt+Shift+Left/Right elsewhere).
    /// The keys a person presses on macOS reach the right command with the keys the engine
    /// declares there; elsewhere nothing is translated.
    #[test]
    fn macos_text_keys_reach_the_engine_s_caret_commands() {
        let declared = |mac: bool| {
            let (word_left, word_right, home, end, promote, demote) = if mac {
                ("Alt+Left", "Alt+Right", "Home / Mod+Left", "End / Mod+Right", "Ctrl+Shift+Left", "Ctrl+Shift+Right")
            } else {
                ("Mod+Left / Alt+Left", "Mod+Right / Alt+Right", "Home", "End", "Alt+Shift+Left", "Alt+Shift+Right")
            };
            [
                CommandInfo::new("caret.left", "Left", "Left"),
                CommandInfo::new("caret.wordLeft", "Word Left", word_left),
                CommandInfo::new("caret.wordRight", "Word Right", word_right),
                CommandInfo::new("caret.home", "Line Start", home),
                CommandInfo::new("caret.end", "Line End", end),
                CommandInfo::new("caret.paraUp", "Paragraph Up", "Mod+Up / Alt+Up"),
                CommandInfo::new("caret.docStart", "Document Start", "Mod+Home"),
                CommandInfo::new("caret.docEnd", "Document End", "Mod+End"),
                CommandInfo::new("para.promote", "Promote", promote),
                CommandInfo::new("para.demote", "Demote", demote),
            ]
        };
        let opt = Modifiers { alt: true, ..Modifiers::NONE };
        let cmd = Modifiers { command: true, mac_cmd: true, ..Modifiers::NONE };
        let shift = |m: Modifiers| Modifiers { shift: true, ..m };
        let extend = |id: &str| Some(Cmd::with(id, serde_json::json!({ "extend": true })));
        let mac = KeyMap::from_commands(&declared(true));
        let press = |key: Key, m: Modifiers| shortcut(key, m, true).and_then(|s| mac.command_for(&s));
        assert_eq!(press(Key::ArrowLeft, opt), Some(Cmd::new("caret.wordLeft")));
        assert_eq!(press(Key::ArrowRight, opt), Some(Cmd::new("caret.wordRight")));
        assert_eq!(press(Key::ArrowLeft, shift(opt)), extend("caret.wordLeft"));
        assert_eq!(press(Key::ArrowRight, shift(opt)), extend("caret.wordRight"));
        assert_eq!(press(Key::ArrowLeft, cmd), Some(Cmd::new("caret.home")));
        assert_eq!(press(Key::ArrowRight, shift(cmd)), extend("caret.end"));
        assert_eq!(press(Key::ArrowUp, cmd), Some(Cmd::new("caret.docStart")));
        assert_eq!(press(Key::ArrowDown, shift(cmd)), extend("caret.docEnd"));
        let control = Modifiers { ctrl: true, ..Modifiers::NONE };
        assert_eq!(press(Key::ArrowLeft, shift(control)), Some(Cmd::new("para.promote")), "Control is its own modifier on a Mac");
        // Elsewhere nothing is translated: Ctrl+Left moves by word, and Alt+Shift+Left promotes.
        let other = KeyMap::from_commands(&declared(false));
        let ctrl = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
        let press = |key: Key, m: Modifiers| shortcut(key, m, false).and_then(|s| other.command_for(&s));
        assert_eq!(press(Key::ArrowLeft, ctrl), Some(Cmd::new("caret.wordLeft")));
        assert_eq!(press(Key::ArrowRight, shift(ctrl)), extend("caret.wordRight"));
        assert_eq!(press(Key::ArrowLeft, opt), Some(Cmd::new("caret.wordLeft")));
        assert_eq!(press(Key::ArrowLeft, shift(opt)), Some(Cmd::new("para.promote")));
    }

    #[test]
    fn shortcuts_parse_in_any_order_and_case() {
        assert_eq!(normalize("mod+shift+z").as_deref(), Some("Mod+Shift+Z"));
        assert_eq!(normalize("Shift+Mod+Z").as_deref(), Some("Mod+Shift+Z"));
        assert_eq!(normalize("Cmd+S").as_deref(), Some("Mod+S"));
        assert_eq!(normalize("Option+Shift+left").as_deref(), Some("Alt+Shift+Left"));
        assert_eq!(normalize("Ctrl+Alt+Delete").as_deref(), Some("Ctrl+Alt+Delete"));
        assert_eq!(normalize(" f10 ").as_deref(), Some("F10"));
        assert_eq!(normalize("Mod+,").as_deref(), Some("Mod+,"));
        for bad in ["", "Mod+", "Foo+A", "Mod+Mod+A", "F13", "Mod+Shift", "Mod++"] {
            assert_eq!(normalize(bad), None, "{bad}");
        }
        let s = parse_shortcut("Alt+X").unwrap();
        assert!(s.is_command_key());
        assert!(parse_shortcut("F5").unwrap().is_command_key());
        assert!(!parse_shortcut("Shift+Enter").unwrap().is_command_key());
        assert!(!parse_shortcut("F").unwrap().is_command_key());
    }

    #[test]
    fn user_keys_come_first_and_can_turn_keys_off() {
        let cmds = [
            CommandInfo::new("edit.redo", "Redo", "Mod+Y / Mod+Shift+Z"),
            CommandInfo::new("edit.undo", "Undo", "Mod+Z"),
            CommandInfo::new("other", "Other", "Mod+Z / not a key"),
        ];
        let mut k = KeyMap::from_commands(&cmds);
        assert_eq!(k.bound_to("Mod+Shift+Z"), Some("edit.redo"));
        assert_eq!(k.bound_to("shift+mod+z"), Some("edit.redo"));
        assert_eq!(k.bound_to("Mod+Z"), Some("edit.undo"), "the first command keeps a key two declare");
        assert!(k.bind("Mod+U", "edit.undo"));
        assert!(k.bind("Mod+Y", ""));
        assert!(!k.bind("nonsense+Q", "x"));
        assert_eq!(k.bound_to("Mod+U"), Some("edit.undo"));
        assert_eq!(k.bound_to("Mod+Y"), None, "turned off");
        assert_eq!(k.keys_for("edit.undo"), ["Mod+U", "Mod+Z"]);
        k.reset("edit.undo");
        assert_eq!(k.keys_for("edit.undo"), ["Mod+Z"]);
        let mut saved = BTreeMap::new();
        saved.insert("shift+mod+k".to_string(), "edit.redo".to_string());
        saved.insert("bad key".to_string(), "edit.redo".to_string());
        k.set_user(&saved);
        assert_eq!(k.user().keys().collect::<Vec<_>>(), ["Mod+Shift+K"]);
    }

    #[test]
    fn a_shifted_caret_key_extends_the_selection() {
        let cmds = [
            CommandInfo::new("caret.left", "Left", "Left"),
            CommandInfo::new("caret.wordLeft", "Word Left", "Mod+Left / Alt+Left"),
            CommandInfo::new("caret.docEnd", "Document End", "Mod+End"),
            CommandInfo::new("text.backspace", "Backspace", "Backspace"),
            CommandInfo::new("text.newParagraph", "New Paragraph", "Enter"),
            CommandInfo::new("text.lineBreak", "Line Break", "Shift+Enter"),
            CommandInfo::new("text.deleteSentenceBack", "Delete Previous Sentence", "Mod+Shift+Backspace"),
            CommandInfo::new("text.deleteWordBack", "Delete Previous Word", "Mod+Backspace"),
        ];
        let mut k = KeyMap::from_commands(&cmds);
        let extend = |id: &str| Some(Cmd::with(id, serde_json::json!({ "extend": true })));
        // Shift with a caret movement's key: the same movement, extending.
        assert_eq!(k.shifted("Shift+Left"), extend("caret.left"));
        assert_eq!(k.command_for("Shift+Left"), extend("caret.left"));
        assert_eq!(k.command_for("mod+shift+left"), extend("caret.wordLeft"));
        assert_eq!(k.command_for("Alt+Shift+Left"), extend("caret.wordLeft"));
        assert_eq!(k.command_for("Mod+Shift+End"), extend("caret.docEnd"));
        // Shift+Backspace still deletes back.
        assert_eq!(k.command_for("Shift+Backspace"), Some(Cmd::new("text.backspace")));
        // A key without Shift runs its command as it is; the rule says nothing about it.
        assert_eq!(k.command_for("Left"), Some(Cmd::new("caret.left")));
        assert_eq!(k.shifted("Left"), None);
        // A shifted key with a command of its own runs that command.
        assert_eq!(k.command_for("Shift+Enter"), Some(Cmd::new("text.lineBreak")));
        assert_eq!(k.command_for("Mod+Shift+Backspace"), Some(Cmd::new("text.deleteSentenceBack")));
        // The rule covers caret movements and deleting back, nothing else.
        assert_eq!(k.command_for("Shift+Right"), None, "no command moves right here");
        assert_eq!(k.shifted("Shift+Enter"), None, "Enter moves no caret");
        assert_eq!(k.command_for("Shift+F13"), None);
        // The user's keys come first: one bound to another command, and one turned off.
        assert!(k.bind("Shift+Left", "edit.other"));
        assert_eq!(k.command_for("Shift+Left"), Some(Cmd::new("edit.other")));
        assert!(k.bind("Shift+Left", ""));
        assert_eq!(k.command_for("Shift+Left"), None, "a key the user turned off stays off");
        assert_eq!(k.shifted("Shift+Left"), extend("caret.left"), "the rule itself does not look at the shifted key");
        // A caret key the user moved takes its shifted form with it.
        assert!(k.bind("Mod+J", "caret.left"));
        assert_eq!(k.command_for("Mod+Shift+J"), extend("caret.left"));
    }

    #[test]
    fn shortcuts_display_per_platform() {
        assert_eq!(display("Mod+Shift+P", false), "Ctrl+Shift+P");
        assert_eq!(display("Mod+Shift+P", true), "⇧⌘P");
        assert_eq!(display("not a key", true), "not a key");
    }
}
