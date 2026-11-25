// SPDX-License-Identifier: MIT OR Apache-2.0
//! The shell's dialogs: the command palette, "save changes first?", recovering unsaved
//! documents, and the bodies of the Preferences and Keys pages. Each one acts by running
//! commands, so undo and agents work the same.

use egui::{Color32, Context, RichText, Ui};
use jcj_i18n::{tr, tr_with};
use jcj_theme::Mode;
use serde_json::json;

use crate::app::App;
use crate::commands::{Cmd, CommandInfo, Commands, ids, palette_matches, shell_commands};
use crate::prefs::{MAX_AUTOSAVE_MINUTES, MAX_IDLE_SECONDS};
use crate::shell::Shell;

/// Most commands the palette lists at once.
const PALETTE_ROWS: usize = 40;

/// The command palette's state while it shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Palette {
    /// What has been typed.
    pub query: String,
}

/// "Save changes first?", asked before something that would lose unsaved changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsavedPrompt {
    /// The document's name.
    pub name: String,
    /// What is about to happen, as words that follow "before" ("opening another document").
    pub doing: String,
    /// Handed back with the answer ([`ids::UNSAVED_ANSWER`]), so the app knows what to go on with.
    pub tag: String,
}

/// The user's answer to [`UnsavedPrompt`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsavedAnswer {
    /// Save, then go on.
    Save,
    /// Go on without saving.
    DontSave,
    /// Don't go on.
    Cancel,
}

impl UnsavedAnswer {
    /// The answer's name in [`ids::UNSAVED_ANSWER`]'s parameters.
    pub fn key(self) -> &'static str {
        match self {
            UnsavedAnswer::Save => "save",
            UnsavedAnswer::DontSave => "dontSave",
            UnsavedAnswer::Cancel => "cancel",
        }
    }

    /// The answer named `k`.
    pub fn from_key(k: &str) -> Option<Self> {
        [UnsavedAnswer::Save, UnsavedAnswer::DontSave, UnsavedAnswer::Cancel].into_iter().find(|a| a.key() == k)
    }
}

/// A copy crash recovery kept, offered to restore or discard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryCopy {
    /// Its id, handed back in [`ids::RECOVER`]'s parameters.
    pub id: String,
    /// The document's name.
    pub name: String,
    /// When it was kept, as words.
    pub saved_at: String,
}

/// The Keys page's state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeysPage {
    /// The filter typed.
    pub filter: String,
    /// The command waiting for its new key, if any.
    pub capturing: Option<String>,
}

/// The dialogs' state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dialogs {
    /// The command palette, while it shows.
    pub palette: Option<Palette>,
    /// "Save changes first?", while it shows.
    pub unsaved: Option<UnsavedPrompt>,
    /// Copies crash recovery kept (the app fills this at start).
    pub recovery: Vec<RecoveryCopy>,
    /// The recovery window shows.
    pub show_recovery: bool,
    /// Whether restoring is possible now (the document open is untouched); the app sets it.
    pub can_restore: bool,
    /// The Keys page.
    pub keys: KeysPage,
}

/// Show whichever dialogs are open.
pub(crate) fn show<A: App>(shell: &mut Shell, ctx: &Context, app: &mut A) {
    palette_window(shell, ctx, &*app);
    recovery_window(shell, ctx);
    unsaved_window(shell, ctx);
}

/// Every command the palette and the Keys page list: the app's, then the shell's.
pub(crate) fn all_commands(cmds: &dyn Commands) -> Vec<CommandInfo> {
    let mut all = cmds.all_commands();
    all.extend(shell_commands());
    all
}

fn palette_window(shell: &mut Shell, ctx: &Context, cmds: &dyn Commands) {
    let Some(mut palette) = shell.dialogs.palette.take() else { return };
    let mut open = true;
    let mut chosen = None;
    // Found and shown by their labels in the language in use.
    let all: Vec<CommandInfo> = all_commands(cmds)
        .into_iter()
        .map(|mut c| {
            c.label = tr(&c.label);
            c
        })
        .collect();
    egui::Window::new(tr("Commands")).open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
        let label = ui.label(tr("Find a command"));
        let edit = ui.add(egui::TextEdit::singleline(&mut palette.query)).labelled_by(label.id);
        edit.request_focus();
        let matches = palette_matches(&all, &palette.query, PALETTE_ROWS);
        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            chosen = matches.first().map(|m| m.id.clone());
        }
        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            for c in &matches {
                let key = c.first_shortcut();
                let text = if key.is_empty() { c.label.clone() } else { format!("{}    {}", c.label, crate::keys::display(key, shell.mac)) };
                if ui.selectable_label(false, text).clicked() {
                    chosen = Some(c.id.clone());
                }
            }
        });
    });
    if ui_escape(ctx) {
        open = false;
    }
    if let Some(id) = chosen {
        shell.run(Cmd::new(id));
        return;
    }
    if open {
        shell.dialogs.palette = Some(palette);
    }
}

fn ui_escape(ctx: &Context) -> bool {
    ctx.input(|i| i.key_pressed(egui::Key::Escape))
}

fn recovery_window(shell: &mut Shell, ctx: &Context) {
    if !shell.dialogs.show_recovery {
        return;
    }
    let copies = shell.dialogs.recovery.clone();
    let can_restore = shell.dialogs.can_restore;
    let mut open = true;
    let mut action = None;
    egui::Window::new(tr("Recover Unsaved Documents")).open(&mut open).show(ctx, |ui| {
        if copies.is_empty() {
            ui.label(tr("There are no unsaved documents to recover."));
        } else if !can_restore {
            ui.label(tr("Start a new document to restore one into it."));
        }
        for c in &copies {
            ui.horizontal(|ui| {
                ui.label(format!("{} — {}", c.name, c.saved_at));
                if ui.add_enabled(can_restore, egui::Button::new(tr("Restore"))).clicked() {
                    action = Some(json!({"restore": c.id}));
                }
                if ui.button(tr("Discard")).clicked() {
                    action = Some(json!({"discard": c.id}));
                }
            });
        }
    });
    if let Some(params) = action {
        shell.run(Cmd::with(ids::RECOVER, params));
    }
    shell.dialogs.show_recovery = open;
}

fn unsaved_window(shell: &mut Shell, ctx: &Context) {
    let Some(q) = shell.dialogs.unsaved.clone() else { return };
    let mut answer = None;
    egui::Window::new(tr("Unsaved Changes")).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
        ui.label(tr_with("Save the changes to “{name}” before {doing}?", &[("name", &q.name), ("doing", &tr(&q.doing))]));
        ui.label(tr("If you don't save, your changes since the last save are lost."));
        ui.horizontal(|ui| {
            if ui.button(tr("Save")).clicked() {
                answer = Some(UnsavedAnswer::Save);
            }
            if ui.button(tr("Don't Save")).clicked() {
                answer = Some(UnsavedAnswer::DontSave);
            }
            if ui.button(tr("Cancel")).clicked() {
                answer = Some(UnsavedAnswer::Cancel);
            }
        });
    });
    if answer.is_none() && ui_escape(ctx) {
        answer = Some(UnsavedAnswer::Cancel);
    }
    if let Some(a) = answer {
        shell.dialogs.unsaved = None;
        shell.run(Cmd::with(ids::UNSAVED_ANSWER, json!({"answer": a.key(), "then": q.tag})));
    }
}

/// The shell's preferences, on the Preferences page: auto-save, the theme, starting, the user's
/// name.
pub fn prefs_body(shell: &mut Shell, ui: &mut Ui) {
    ui.heading(tr("Auto-save"));
    ui.checkbox(&mut shell.autosave.enabled, tr("Save documents automatically"));
    let mut minutes = shell.autosave.every.as_secs() / 60;
    ui.horizontal(|ui| {
        // Each number is named by the words before it, for screen readers.
        let every = ui.label(tr("Every"));
        if ui.add(egui::DragValue::new(&mut minutes).range(1..=u64::from(MAX_AUTOSAVE_MINUTES))).labelled_by(every.id).changed() {
            shell.autosave.every = std::time::Duration::from_secs(minutes * 60);
        }
        let pause = ui.label(tr("minutes, once typing pauses for"));
        let mut idle = shell.autosave.idle.as_secs();
        if ui.add(egui::DragValue::new(&mut idle).range(1..=u64::from(MAX_IDLE_SECONDS))).labelled_by(pause.id).changed() {
            shell.autosave.idle = std::time::Duration::from_secs(idle);
        }
        ui.label(tr("seconds"));
    });
    ui.label(tr("Only documents saved to a file are saved automatically; crash recovery keeps the others."));
    ui.separator();
    ui.heading(tr("Theme"));
    ui.horizontal(|ui| {
        for (mode, label) in [(Mode::Light, tr("Light")), (Mode::Dark, tr("Dark")), (Mode::ExtraDark, tr("Extra dark"))] {
            if ui.selectable_label(shell.mode == mode, label).clicked() {
                shell.mode = mode;
            }
        }
    });
    ui.separator();
    ui.heading(tr("Starting"));
    ui.checkbox(&mut shell.reopen_last, tr("Open the documents that were open when the app last closed"));
    ui.checkbox(&mut shell.updates.check_at_start, tr("Check for updates when the app starts"));
    ui.separator();
    ui.heading(tr("You"));
    ui.horizontal(|ui| {
        let label = ui.label(tr("Your name (notes and tracked changes)"));
        ui.text_edit_singleline(&mut shell.user_name).labelled_by(label.id);
    });
}

/// Every command with its key and the user's own, on the Keys page: "Set…" waits for a key,
/// "Reset" goes back to the command's own.
pub fn keys_body(shell: &mut Shell, ui: &mut Ui, cmds: &dyn Commands) {
    let mut page = std::mem::take(&mut shell.dialogs.keys);
    let mut set: Option<(String, String)> = None;
    let mut clear: Option<String> = None;
    let all: Vec<CommandInfo> = all_commands(cmds).into_iter().filter(|c| !c.takes_params).collect();
    ui.horizontal(|ui| {
        let label = ui.label(tr("Find"));
        ui.text_edit_singleline(&mut page.filter).labelled_by(label.id);
    });
    if let Some(id) = &page.capturing {
        ui.colored_label(Color32::from_rgb(200, 120, 30), tr_with("Press the new key for {command} (Escape to cancel)", &[("command", id)]));
        let pressed = ui.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Key { key, pressed: true, modifiers, .. } => Some((*key, *modifiers)),
                _ => None,
            })
        });
        if let Some((key, m)) = pressed {
            if key == egui::Key::Escape {
                page.capturing = None;
            } else if let Some(sc) = crate::keys::shortcut(key, m, shell.mac) {
                set = Some((sc, id.clone()));
                page.capturing = None;
            }
        }
    }
    ui.separator();
    egui::ScrollArea::vertical().id_salt("jcj-keys-list").show(ui, |ui| {
        egui::Grid::new("jcj-keys").striped(true).num_columns(4).show(ui, |ui| {
            let f = page.filter.to_lowercase();
            for c in &all {
                let label = tr(&c.label);
                if !f.is_empty() && !format!("{} {} {} {}", c.id, c.label, label, c.location).to_lowercase().contains(&f) {
                    continue;
                }
                ui.label(label).on_hover_text(c.id.as_str());
                ui.label(RichText::new(crate::keys::display(c.first_shortcut(), shell.mac)).weak());
                let mine: Vec<String> =
                    shell.keys.user().iter().filter(|(_, x)| x.as_str() == c.id).map(|(k, _)| crate::keys::display(k, shell.mac)).collect();
                ui.label(mine.join(", "));
                ui.horizontal(|ui| {
                    if ui.small_button(tr("Set…")).clicked() {
                        page.capturing = Some(c.id.clone());
                    }
                    if !mine.is_empty() && ui.small_button(tr("Reset")).clicked() {
                        clear = Some(c.id.clone());
                    }
                });
                ui.end_row();
            }
        });
    });
    if let Some((sc, id)) = set {
        shell.keys.bind(&sc, &id);
    }
    if let Some(id) = clear {
        shell.keys.reset(&id);
    }
    shell.dialogs.keys = page;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsaved_answers_have_names() {
        for a in [UnsavedAnswer::Save, UnsavedAnswer::DontSave, UnsavedAnswer::Cancel] {
            assert_eq!(UnsavedAnswer::from_key(a.key()), Some(a));
        }
        assert_eq!(UnsavedAnswer::from_key("maybe"), None);
    }
}
