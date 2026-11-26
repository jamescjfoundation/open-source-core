// SPDX-License-Identifier: MIT OR Apache-2.0
//! A new foundation app, started from open-source-core's template. It opens a window on the core's
//! shell with one document, shown as rendered pages, and saves it as a project file. Replace the
//! names, add the app's commands, tabs and panels, and keep the app's own rules here; anything a
//! second app would want belongs in the core (AGENTS.md).
//!
//! The start-up is the core's (`jcj_shell::desktop`): this file gives the app's data
//! ([`DESKTOP`]) and what only the app can do ([`DesktopApp`]). The window's title, app id and
//! icon, the settings folder, preferences, the update check, the file dialogs and the steps of
//! every frame and of the end are not written here, so none of them can be left out.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::path::PathBuf;
use std::process::ExitCode;

use jcj_shell::commands::ids;
use jcj_shell::desktop::{Desktop, DesktopApp};
use jcj_shell::egui;
use jcj_shell::prefs::{NoPrefs, Prefs};
use jcj_shell::{About, App, Cmd, CommandInfo, Commands, Shell};
use serde_json::json;

/// This app's identity in project files: change the id, and raise the version when the app's own
/// data changes shape (with a migration, `jcj_project::App::with_migration`).
const APP: jcj_project::App = jcj_project::App::new("org.example.app", 1);

/// This app's data for its start-up: its name, its machine name (the settings folder on Linux,
/// the stem of its release files), the window's app id and its version. Add what the app has,
/// each a field of `Desktop`:
/// - `brand`: `Brand::new(include_str!(...tokens.json), include_bytes!(...icon-256.png))`, the
///   product's design tokens (the core's `crates/jcj-theme/TOKENS.md`) and the window's icon;
/// - `releases`: where its releases are published, which turns the update check on;
/// - `profile_env`: a variable naming another settings folder, for a run kept apart.
const DESKTOP: Desktop = Desktop::new("App", "app", "org.example.app", env!("CARGO_PKG_VERSION"));

struct Doc {
    session: jcj_engine::Session,
    path: PathBuf,
    revision: u64,
    saved: u64,
    page: Option<(u64, egui::TextureHandle)>,
}

impl Doc {
    fn new(path: PathBuf) -> Doc {
        let registry = std::sync::Arc::new(jcj_engine::registry_with(jcj_engine::commands::merged(jcj_features::commands())));
        let doc = match jcj_project::files::open(&path, &APP) {
            Ok(project) => project.documents().next().map(|(_, _, d)| d.clone()).unwrap_or_default(),
            Err(_) => jcj_doc::new_document(),
        };
        Doc { session: jcj_engine::session_with(doc, registry), path, revision: 0, saved: 0, page: None }
    }

    fn save(&mut self) -> Result<(), String> {
        let project = jcj_project::Project::with_document(&APP, "Main", self.session.doc.clone()).map_err(|e| e.to_string())?;
        jcj_project::files::save(&self.path, &project).map_err(|e| e.to_string())?;
        self.saved = self.revision;
        Ok(())
    }
}

impl Commands for Doc {
    fn command(&self, id: &str) -> Option<CommandInfo> {
        self.all_commands().into_iter().find(|c| c.id == id)
    }

    fn all_commands(&self) -> Vec<CommandInfo> {
        vec![
            CommandInfo { icon: "save".into(), ..CommandInfo::new(ids::SAVE, "Save", "Mod+S") },
            CommandInfo { icon: "undo".into(), ..CommandInfo::new(ids::UNDO, "Undo", "Mod+Z") },
            CommandInfo { icon: "redo".into(), ..CommandInfo::new(ids::REDO, "Redo", "Mod+Shift+Z") },
            CommandInfo::new("app.hello", "Insert Greeting", "Mod+G"),
        ]
    }
}

impl App for Doc {
    fn document_name(&self) -> String {
        self.path.file_name().map_or_else(|| "Untitled".into(), |n| n.to_string_lossy().into_owned())
    }

    fn is_dirty(&self) -> bool {
        self.revision != self.saved
    }

    fn revision(&self) -> u64 {
        self.revision
    }

    fn run(&mut self, cmd: &Cmd, _shell: &mut Shell) -> Result<(), String> {
        match cmd.id.as_str() {
            ids::SAVE => return self.save(),
            ids::UNDO | ids::REDO => {
                self.session.run(&cmd.id, &json!({})).map_err(|e| format!("{e:?}"))?;
            }
            "app.hello" => {
                self.session.run("text.insert", &json!({"text": "Hello from a new app. "})).map_err(|e| format!("{e:?}"))?;
            }
            other => return Err(format!("unknown command {other}")),
        }
        self.revision += 1;
        Ok(())
    }

    fn document_ui(&mut self, ui: &mut egui::Ui, _shell: &mut Shell) {
        let stale = self.page.as_ref().is_none_or(|(rev, _)| *rev != self.revision);
        if stale {
            let mut cache = jcj_layout::LayoutCache::new();
            let layout = jcj_layout::print_layout(&self.session.doc, &mut cache);
            if let Some(img) = jcj_output::render_page(&self.session.doc, &layout, 0, 1.0, &jcj_output::RenderSettings::default())
                && let (Ok(w), Ok(h)) = (usize::try_from(img.width), usize::try_from(img.height))
            {
                let color = egui::ColorImage::from_rgba_unmultiplied([w, h], &img.straight_rgba());
                self.page = Some((self.revision, ui.ctx().load_texture("page", color, egui::TextureOptions::LINEAR)));
            }
        }
        egui::ScrollArea::both().show(ui, |ui| {
            if let Some((_, tex)) = &self.page {
                ui.add(egui::Image::new(tex).fit_to_original_size(1.0));
            }
        });
    }

    fn about(&self) -> About {
        About { summary: "A foundation app.".into(), credit: "Based on WordCraft by the ArtCraft team.".into(), ..About::default() }
    }
}

/// What only this app can do at start-up. Every step has a default but the preferences: this
/// app keeps none of its own yet, so its file holds the shell's alone (`NoPrefs`). When it has
/// some, they are a struct here in place of `NoPrefs`, put into effect and read back below.
impl DesktopApp for Doc {
    type Prefs = Prefs<NoPrefs>;

    fn apply_prefs(&mut self, _prefs: &NoPrefs, _shell: &mut Shell) {}

    fn prefs(&self, _last: &NoPrefs) -> NoPrefs {
        NoPrefs {}
    }

    fn window_title(&self) -> Option<String> {
        Some(format!("{} - {}", self.document_name(), DESKTOP.name))
    }
}

fn main() -> ExitCode {
    // The first file named on the command line is the document; with none, a new one.
    DESKTOP.main(|start| Doc::new(start.files.first().cloned().unwrap_or_else(|| PathBuf::from("untitled.app"))))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The start-up without a window: the app is built with the file it was started with, its
    /// preferences file is where the settings folder is, and the window is told its title.
    #[test]
    fn the_app_starts_from_its_data() {
        let dir = std::env::temp_dir().join(format!("app-start-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let ctx = egui::Context::default();
        let files = vec![dir.join("draft.app")];
        let mut window = jcj_shell::desktop::Window::start(&DESKTOP, &ctx, Some(dir.join("settings")), files, |start| {
            Doc::new(start.files.first().cloned().unwrap_or_default())
        });
        assert_eq!(window.app.document_name(), "draft.app");
        assert_eq!(window.app.window_title().as_deref(), Some("draft.app - App"));
        assert_eq!(window.prefs_path(), Some(dir.join("settings").join("preferences.json").as_path()));
        assert_eq!(DESKTOP.viewport().app_id.as_deref(), Some("org.example.app"));
        window.exit();
        drop(window);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
