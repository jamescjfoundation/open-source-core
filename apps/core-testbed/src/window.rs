// SPDX-License-Identifier: MIT OR Apache-2.0
//! The test app's window: a document on the shell, shown as rendered pages, with a board panel
//! and an information panel. Small on purpose: it proves the crates fit together in a real app,
//! and gives every new core interface a place to be used before any product uses it.

use std::path::PathBuf;
use std::time::SystemTime;

use jcj_board::{Board, Corkboard, Selection};
use jcj_engine::Session;
use jcj_interchange::Format;
use jcj_shell::commands::{ids, shell_ids};
use jcj_shell::egui;
use jcj_shell::widgets::{self, Control};
use jcj_shell::{About, App, Cmd, CommandInfo, Commands, PanelSpec, Shell, Side, TabSpec};
use serde_json::json;

use crate::selftest::{APP, sample_document};

/// The test app's own command ids.
pub mod cmd {
    pub const SAMPLE: &str = "testbed.sample";
    pub const INSERT: &str = "testbed.insertText";
    pub const EXPORT_PDF: &str = "testbed.exportPdf";
    pub const EXPORT_ALL: &str = "testbed.exportAll";
    pub const ADD_CARD: &str = "testbed.addCard";
    pub const SNAPSHOT: &str = "testbed.snapshot";
    pub const SELFTEST: &str = "testbed.selftest";
}

/// The test app's state.
pub struct Testbed {
    pub session: Session,
    pub board: Board,
    selection: Selection,
    /// Where Save writes the project file, and exports go beside it.
    pub path: PathBuf,
    saved_revision: u64,
    revision: u64,
    page: Option<(u64, egui::TextureHandle)>,
    pub log: Vec<String>,
    /// Whether the user started a document (the start window shows until then, unless the
    /// file already exists).
    pub started: bool,
    /// Words written this session, toward a session target (the core's writing statistics).
    pub writing: jcj_shell::writing::Writing,
}

impl Testbed {
    /// A test app on the sample document, saving to `path`.
    pub fn new(path: PathBuf) -> Testbed {
        let doc = sample_document().unwrap_or_default();
        Testbed {
            session: crate::selftest::session(doc),
            board: Board::default(),
            selection: Selection::default(),
            path: path.clone(),
            saved_revision: 0,
            revision: 0,
            started: path.is_file(),
            writing: jcj_shell::writing::Writing::new(0).with_target(Some(500)),
            page: None,
            log: Vec::new(),
        }
    }

    fn changed(&mut self) {
        self.revision += 1;
        let words = self.session.doc.body.iter().filter_map(|b| b.as_para()).map(|p| jcj_doc::count_words(&p.plain_text())).sum();
        self.writing.observe(words, jcj_shell::writing::Day::of(std::time::SystemTime::now(), 0));
    }

    fn save(&mut self) -> Result<(), String> {
        let mut project = jcj_project::Project::with_document(&APP, "Main", self.session.doc.clone()).map_err(|e| e.to_string())?;
        let board = serde_json::to_value(&self.board).map_err(|e| e.to_string())?;
        project.app_data_mut().insert("board".into(), board);
        jcj_project::files::save(&self.path, &project).map_err(|e| e.to_string())?;
        self.saved_revision = self.revision;
        self.log.push(format!("saved {}", self.path.display()));
        Ok(())
    }

    fn export(&mut self, format: Format) -> Result<(), String> {
        let bytes = jcj_interchange::export(format, &self.session.doc).map_err(|e| e.to_string())?;
        let out = self.path.with_extension(format.extension());
        std::fs::write(&out, bytes).map_err(|e| e.to_string())?;
        self.log.push(format!("exported {}", out.display()));
        Ok(())
    }

    fn export_pdf(&mut self) -> Result<(), String> {
        let pdf = jcj_output::export_pdf_with_layout(&self.session.doc, &jcj_output::PdfSettings::default()).map_err(|e| e.to_string())?;
        let out = self.path.with_extension("pdf");
        std::fs::write(&out, pdf).map_err(|e| e.to_string())?;
        self.log.push(format!("exported {}", out.display()));
        Ok(())
    }

    /// The first page, rendered, as a texture (redrawn when the document changes).
    fn page_texture(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        if let Some((rev, tex)) = &self.page
            && *rev == self.revision
        {
            return Some(tex.clone());
        }
        let mut cache = jcj_layout::LayoutCache::new();
        let layout = jcj_layout::print_layout(&self.session.doc, &mut cache);
        let image = jcj_output::render_page(&self.session.doc, &layout, 0, 1.0, &jcj_output::RenderSettings::default())?;
        let size = [usize::try_from(image.width).ok()?, usize::try_from(image.height).ok()?];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, &image.straight_rgba());
        let tex = ctx.load_texture("page-1", color, egui::TextureOptions::LINEAR);
        self.page = Some((self.revision, tex.clone()));
        Some(tex)
    }
}

impl Commands for Testbed {
    fn command(&self, id: &str) -> Option<CommandInfo> {
        self.all_commands().into_iter().find(|c| c.id == id)
    }

    fn why_disabled(&self, id: &str) -> Option<String> {
        match id {
            ids::UNDO if !self.session.can_undo() => Some("Nothing to undo".into()),
            _ => None,
        }
    }

    fn all_commands(&self) -> Vec<CommandInfo> {
        vec![
            CommandInfo { icon: "new".into(), ..CommandInfo::new(ids::NEW, "New", "Mod+N") },
            CommandInfo::new(ids::NEW_FROM_TEMPLATE, "New from Template", ""),
            CommandInfo { icon: "save".into(), ..CommandInfo::new(ids::SAVE, "Save", "Mod+S") },
            CommandInfo { icon: "undo".into(), ..CommandInfo::new(ids::UNDO, "Undo", "Mod+Z") },
            CommandInfo { icon: "redo".into(), ..CommandInfo::new(ids::REDO, "Redo", "Mod+Shift+Z") },
            CommandInfo::new(cmd::SAMPLE, "Sample Document", ""),
            CommandInfo::new(cmd::INSERT, "Insert Text", "Mod+T"),
            CommandInfo { icon: "export".into(), ..CommandInfo::new(cmd::EXPORT_PDF, "Export PDF", "Mod+E") },
            CommandInfo::new(cmd::EXPORT_ALL, "Export Every Format", ""),
            CommandInfo::new(cmd::ADD_CARD, "Add Card", ""),
            CommandInfo::new(cmd::SNAPSHOT, "Save With Snapshot", ""),
            CommandInfo::new(cmd::SELFTEST, "Run Self-Test", ""),
        ]
    }
}

impl App for Testbed {
    fn document_name(&self) -> String {
        self.path.file_name().map_or_else(|| "Untitled".into(), |n| n.to_string_lossy().into_owned())
    }

    fn document_path(&self) -> Option<String> {
        Some(self.path.display().to_string())
    }

    fn is_dirty(&self) -> bool {
        self.revision != self.saved_revision
    }

    fn has_file(&self) -> bool {
        self.path.is_file()
    }

    fn revision(&self) -> u64 {
        self.revision
    }

    fn autosave(&mut self) -> Result<(), String> {
        self.save()
    }

    fn status_ui(&mut self, ui: &mut egui::Ui, _shell: &mut Shell) {
        let words = self.writing.session_words();
        jcj_shell::writing::progress_bar(ui, &format!("{words} of 500 words this session"), self.writing.progress().unwrap_or(0.0), 80.0);
    }

    fn has_document(&self) -> bool {
        self.started
    }

    fn templates(&self) -> Vec<jcj_shell::TemplateSpec> {
        vec![
            jcj_shell::TemplateSpec::new("blank", "Blank", "An empty document.").in_group("Documents"),
            jcj_shell::TemplateSpec::new("sample", "Sample", "A document using every feature the testbed shows.").in_group("Documents"),
        ]
    }

    fn run(&mut self, c: &Cmd, _shell: &mut Shell) -> Result<(), String> {
        self.started = true;
        match c.id.as_str() {
            ids::NEW_FROM_TEMPLATE => {
                let doc = match c.params.get("template").and_then(|t| t.as_str()) {
                    Some("sample") => sample_document()?,
                    _ => jcj_doc::new_document(),
                };
                self.session = crate::selftest::session(doc);
                self.changed();
            }
            ids::NEW => {
                self.session = crate::selftest::session(jcj_doc::new_document());
                self.changed();
            }
            ids::SAVE => self.save()?,
            ids::UNDO | ids::REDO => {
                self.session.run(&c.id, &json!({})).map_err(|e| format!("{e:?}"))?;
                self.changed();
            }
            cmd::SAMPLE => {
                self.session = crate::selftest::session(sample_document()?);
                self.changed();
            }
            cmd::INSERT => {
                let text = c.params.get("text").and_then(|t| t.as_str()).unwrap_or("Text typed by the test app. ").to_owned();
                self.session.run("text.insert", &json!({ "text": text })).map_err(|e| format!("{e:?}"))?;
                self.changed();
            }
            cmd::EXPORT_PDF => self.export_pdf()?,
            cmd::EXPORT_ALL => {
                for f in Format::ALL {
                    self.export(f)?;
                }
            }
            cmd::ADD_CARD => {
                let n = self.board.cards.len() + 1;
                self.board.add_card(&format!("Card {n}"), "Written in the test app.").map_err(|e| format!("{e:?}"))?;
                self.changed();
            }
            cmd::SNAPSHOT => {
                let mut project = jcj_project::Project::with_document(&APP, "Main", self.session.doc.clone()).map_err(|e| e.to_string())?;
                if let Some(id) = project.find_document("Main") {
                    project.take_snapshot(id, "Snapshot", SystemTime::now()).map_err(|e| e.to_string())?;
                }
                jcj_project::files::save(&self.path, &project).map_err(|e| e.to_string())?;
                self.saved_revision = self.revision;
                self.log.push("saved with a snapshot".into());
            }
            cmd::SELFTEST => match crate::selftest::run() {
                Ok(steps) => self.log.extend(steps.into_iter().map(|s| format!("ok  {}: {}", s.name, s.detail))),
                Err(e) => return Err(format!("self-test failed: {e}")),
            },
            other => return Err(format!("unknown command {other}")),
        }
        Ok(())
    }

    fn document_ui(&mut self, ui: &mut egui::Ui, _shell: &mut Shell) {
        egui::ScrollArea::both().show(ui, |ui| match self.page_texture(ui.ctx()) {
            Some(tex) => {
                ui.add(egui::Image::new(&tex).fit_to_original_size(1.0));
            }
            None => {
                ui.label("The page could not be drawn.");
            }
        });
    }

    fn ribbon_tabs(&self) -> Vec<TabSpec> {
        vec![TabSpec::new("home", "Home", "H"), TabSpec::new("tools", "Tools", "T")]
    }

    fn ribbon_ui(&mut self, tab: &str, ui: &mut egui::Ui, shell: &mut Shell) {
        let me: &Testbed = self;
        match tab {
            "home" => {
                widgets::group(ui, shell, "Document", |ui, shell| {
                    widgets::big(ui, shell, me, &Control::new(cmd::SAMPLE).label("Sample"));
                    widgets::big(ui, shell, me, &Control::new(cmd::INSERT).label("Insert"));
                });
                widgets::group(ui, shell, "History", |ui, shell| {
                    widgets::small(ui, shell, me, &Control::new(ids::UNDO).icon("undo").label("Undo"));
                    widgets::small(ui, shell, me, &Control::new(ids::REDO).icon("redo").label("Redo"));
                });
                widgets::group(ui, shell, "Board", |ui, shell| {
                    widgets::big(ui, shell, me, &Control::new(cmd::ADD_CARD).label("Add Card"));
                });
            }
            "tools" => {
                widgets::group(ui, shell, "Export", |ui, shell| {
                    widgets::big(ui, shell, me, &Control::new(cmd::EXPORT_PDF).label("PDF"));
                    widgets::big(ui, shell, me, &Control::new(cmd::EXPORT_ALL).label("Every Format"));
                });
                widgets::group(ui, shell, "Checks", |ui, shell| {
                    widgets::big(ui, shell, me, &Control::new(cmd::SELFTEST).label("Self-Test"));
                    widgets::big(ui, shell, me, &Control::new(shell_ids::PALETTE).label("Commands"));
                });
            }
            _ => {}
        }
    }

    fn panels(&self) -> Vec<PanelSpec> {
        vec![PanelSpec::new("board", "Board", "outline", Side::Left), PanelSpec::new("log", "Log", "draft", Side::Bottom)]
    }

    fn panel_ui(&mut self, panel: &str, ui: &mut egui::Ui, _shell: &mut Shell) {
        match panel {
            "board" => {
                let events =
                    Corkboard::new("testbed-board").per_row(2).empty_hint("No cards yet: Home ▸ Add Card").show(ui, &self.board, &self.selection);
                for e in &events {
                    if self.board.apply(e, &mut self.selection) {
                        self.revision += 1;
                    }
                }
            }
            "log" => {
                for line in &self.log {
                    ui.label(line);
                }
            }
            _ => {}
        }
    }

    fn about(&self) -> About {
        About {
            summary: "The test app of open-source-core: it uses every jcj- crate.".into(),
            credit: "Based on WordCraft by the ArtCraft team.".into(),
            acknowledgments: "WordCraft and craft-libs (MIT OR Apache-2.0), egui (MIT OR Apache-2.0).".into(),
            website: Some("https://github.com/jamescjfoundation/open-source-core".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_change_and_save_the_document() {
        let dir = std::env::temp_dir().join(format!("core-testbed-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = Testbed::new(dir.join("test.jcjtest"));
        let mut shell = Shell::new("Core testbed", "0", jcj_shell::theme::Tokens::default());
        app.run(&Cmd::new(cmd::INSERT), &mut shell).unwrap();
        assert!(app.is_dirty());
        app.run(&Cmd::new(ids::UNDO), &mut shell).unwrap();
        app.run(&Cmd::new(cmd::ADD_CARD), &mut shell).unwrap();
        app.run(&Cmd::new(ids::SAVE), &mut shell).unwrap();
        assert!(!app.is_dirty());
        let project = jcj_project::files::open(&app.path, &APP).unwrap();
        assert_eq!(project.document_count(), 1);
        assert!(project.app_data().contains_key("board"));
        app.run(&Cmd::with(ids::NEW_FROM_TEMPLATE, json!({ "template": "sample" })), &mut shell).unwrap();
        assert!(app.writing.session_words() > 0, "the sample's words count as written this session");
        assert!(app.has_document() && !app.templates().is_empty());
        app.run(&Cmd::new(cmd::EXPORT_ALL), &mut shell).unwrap();
        assert!(dir.join("test.epub").is_file() && dir.join("test.docx").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
