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
use jcj_shell::desktop::{Desktop, DesktopApp};
use jcj_shell::egui;
use jcj_shell::files::FileAsk;
use jcj_shell::i18n::{tl, trn};
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
    /// Add `{"line": "..."}` to the log panel.
    pub const LOG: &str = "testbed.log";
    pub const SELFTEST: &str = "testbed.selftest";
    /// Export every format into a folder the user chooses.
    pub const EXPORT_TO: &str = "testbed.exportTo";
    /// Show the writing statistics window.
    pub const STATISTICS: &str = "testbed.statistics";
}

/// The extension of the test app's project files.
pub const EXTENSION: &str = "jcjtest";

/// The test app's data for the core's start-up, but for its brand, which is made at run time
/// ([`desktop`]). `CORE_TESTBED_PROFILE_DIR` names another settings folder, for a run kept
/// apart from the developer's own.
pub const DESKTOP: Desktop = Desktop {
    profile_env: Some("CORE_TESTBED_PROFILE_DIR"),
    ..Desktop::new("Core testbed", "core-testbed", "org.jcj.core-testbed", env!("CARGO_PKG_VERSION"))
};

/// The test app's data for the core's start-up, with its brand.
pub fn desktop() -> Desktop {
    Desktop { brand: crate::brand::brand(), ..DESKTOP }
}

/// The test app's own preferences, kept beside the shell's in the settings folder.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct TestbedPrefs {
    /// The interface language's code; empty for English.
    pub language: String,
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
    /// The central area split: the page beside the document's plain text (the core's split view).
    pub split: jcj_shell::split::Split,
    split_state: jcj_shell::split::SplitState,
    /// Words written this session, toward a session target (the core's writing statistics).
    pub writing: jcj_shell::writing::Writing,
    /// The writer's statistics (the core's statistics file and window): words written per day,
    /// counted only upward, and writing time. Kept in memory: the testbed keeps no history.
    pub stats: jcj_shell::writing::stats::Statistics,
    /// What counts writing into `stats`.
    pub tracker: jcj_shell::writing::stats::Tracker,
    /// The statistics window.
    pub stats_view: jcj_shell::writing::stats::StatsView,
    /// The headings panel's tree state (the core's tree widget).
    pub headings: jcj_shell::tree::TreeState<usize>,
    /// The interface language kept in the preferences; empty for English.
    pub language: String,
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
            split: jcj_shell::split::Split::None,
            split_state: jcj_shell::split::SplitState::default(),
            writing: jcj_shell::writing::Writing::new(0).with_target(Some(500)),
            stats: jcj_shell::writing::stats::Statistics {
                goals: jcj_shell::writing::stats::Goals { words_per_day: 500, minutes_per_day: 20, ..Default::default() },
                ..Default::default()
            },
            tracker: jcj_shell::writing::stats::Tracker::new(),
            stats_view: jcj_shell::writing::stats::StatsView::new(jcj_shell::writing::stats::Unit::Words),
            headings: jcj_shell::tree::TreeState::default(),
            language: String::new(),
            page: None,
            log: Vec::new(),
        }
    }

    fn changed(&mut self) {
        self.revision += 1;
        let words = self.session.doc.body.iter().filter_map(|b| b.as_para()).map(|p| jcj_doc::count_words(&p.plain_text())).sum();
        let now = std::time::SystemTime::now();
        self.writing.observe(words, jcj_shell::writing::Day::of(now, 0));
        // The testbed has no pages of its own to count: its unit is words.
        let at = jcj_shell::writing::stats::Moment::of(now, 0);
        self.tracker.count(&mut self.stats, words, None, at, std::time::Instant::now());
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

    /// Open the project file at `path`: its main document and its board.
    fn open(&mut self, path: PathBuf) -> Result<(), String> {
        let project = jcj_project::files::open(&path, &APP).map_err(|e| e.to_string())?;
        let doc = project.find_document("Main").and_then(|id| project.document(id)).cloned().ok_or("the file has no main document")?;
        self.board = project.app_data().get("board").cloned().and_then(|b| serde_json::from_value(b).ok()).unwrap_or_default();
        // Another document: its words are the base, not written.
        self.tracker.forget();
        self.session = crate::selftest::session(doc);
        self.path = path;
        self.changed();
        self.saved_revision = self.revision;
        self.log.push(format!("opened {}", self.path.display()));
        Ok(())
    }

    /// The path a command got from a file dialog (`{"path": "..."}`), if it has one yet.
    fn chosen(c: &Cmd) -> Option<PathBuf> {
        c.params.get("path").and_then(|p| p.as_str()).map(PathBuf::from)
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
            CommandInfo::new("testbed.split", "Split View", ""),
            CommandInfo { icon: "open".into(), ..CommandInfo::new(ids::OPEN, "Open…", "Mod+O") },
            CommandInfo { takes_params: true, ..CommandInfo::new(ids::OPEN_PATH, "Open a File", "") },
            CommandInfo { icon: "save".into(), ..CommandInfo::new(ids::SAVE, "Save", "Mod+S") },
            CommandInfo::new(ids::SAVE_AS, "Save As…", "Mod+Shift+S"),
            CommandInfo::new(cmd::EXPORT_TO, "Export Every Format To…", ""),
            CommandInfo { icon: "undo".into(), ..CommandInfo::new(ids::UNDO, "Undo", "Mod+Z") },
            CommandInfo { icon: "redo".into(), ..CommandInfo::new(ids::REDO, "Redo", "Mod+Shift+Z") },
            CommandInfo::new(cmd::SAMPLE, "Sample Document", ""),
            CommandInfo::new(cmd::INSERT, "Insert Text", "Mod+T"),
            CommandInfo { icon: "export".into(), ..CommandInfo::new(cmd::EXPORT_PDF, "Export PDF", "Mod+E") },
            CommandInfo::new(cmd::EXPORT_ALL, "Export Every Format", ""),
            CommandInfo::new(cmd::ADD_CARD, "Add Card", ""),
            CommandInfo::new(cmd::SNAPSHOT, "Save With Snapshot", ""),
            CommandInfo::new(cmd::SELFTEST, "Run Self-Test", ""),
            CommandInfo::new(cmd::STATISTICS, "Writing Statistics", ""),
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
        // Writing time, once a frame; and the statistics window while it is open.
        let at = jcj_shell::writing::stats::Moment::of(std::time::SystemTime::now(), 0);
        self.tracker.tick(&mut self.stats, at, std::time::Instant::now());
        jcj_shell::writing::stats::statistics_window(ui.ctx(), &mut self.stats, &mut self.stats_view, at.day);
        let words = self.writing.session_words();
        jcj_shell::writing::progress_bar(ui, &format!("{words} of 500 words this session"), self.writing.progress().unwrap_or(0.0), 80.0);
        // A count in the interface language, through the shell's door to the catalogs.
        ui.label(trn("{n} card", "{n} cards", self.board.cards.len() as u64, &[]));
    }

    fn has_document(&self) -> bool {
        self.started
    }

    fn templates(&self) -> Vec<jcj_shell::TemplateSpec> {
        vec![
            jcj_shell::TemplateSpec::new("blank", "Blank", "An empty document.").in_group("Documents"),
            jcj_shell::TemplateSpec::new("sample", "Sample", "A document using every feature the testbed shows.")
                .in_group("Documents")
                .with_preview(sample_preview()),
        ]
    }

    fn run(&mut self, c: &Cmd, shell: &mut Shell) -> Result<(), String> {
        self.started = true;
        match c.id.as_str() {
            // The file commands ask the shell for the system's dialog and come back with its
            // answer: the same command again, now with the path chosen (jcj_shell::files).
            ids::OPEN => {
                shell.ask_file(FileAsk::open().title("Open").filter("Testbed projects", &[EXTENSION]), Cmd::new(ids::OPEN_PATH));
            }
            ids::OPEN_PATH => self.open(Self::chosen(c).ok_or("no file was named")?)?,
            ids::SAVE_AS => match Self::chosen(c) {
                Some(path) => {
                    self.path = if path.extension().is_some() { path } else { path.with_extension(EXTENSION) };
                    self.save()?;
                }
                None => {
                    let name = self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    let mut ask = FileAsk::save().title("Save As").filter("Testbed projects", &[EXTENSION]).file_name(name);
                    if let Some(folder) = self.path.parent().filter(|f| f.is_dir()) {
                        ask = ask.in_folder(folder);
                    }
                    shell.ask_file_or(
                        ask,
                        Cmd::new(ids::SAVE_AS),
                        Cmd::with(cmd::LOG, json!({ "line": "Save As was closed without a place chosen" })),
                    );
                }
            },
            cmd::EXPORT_TO => match Self::chosen(c) {
                Some(folder) => {
                    let stem = self.path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "untitled".into());
                    for f in Format::ALL {
                        let bytes = jcj_interchange::export(f, &self.session.doc).map_err(|e| e.to_string())?;
                        let out = folder.join(format!("{stem}.{}", f.extension()));
                        std::fs::write(&out, bytes).map_err(|e| format!("{}: {e}", out.display()))?;
                    }
                    self.log.push(format!("exported every format to {}", folder.display()));
                }
                None => {
                    shell.ask_file(FileAsk::folder().title("Export every format to"), Cmd::new(cmd::EXPORT_TO));
                }
            },
            cmd::LOG => {
                let line = c.params.get("line").and_then(|l| l.as_str()).unwrap_or_default();
                // Run for a dialog closed without choosing, or for one that could not be shown.
                self.log.push(match jcj_shell::files::unavailable(c) {
                    Some(why) => format!("{line} (no dialog could be shown: {why})"),
                    None => line.to_owned(),
                });
            }
            cmd::STATISTICS => self.stats_view.open = true,
            "testbed.split" => {
                self.split = if self.split == jcj_shell::split::Split::None { jcj_shell::split::Split::Side } else { jcj_shell::split::Split::None };
            }
            ids::NEW_FROM_TEMPLATE => {
                let doc = match c.params.get("template").and_then(|t| t.as_str()) {
                    Some("sample") => sample_document()?,
                    _ => jcj_doc::new_document(),
                };
                // Another document: its words are the base, not written.
                self.tracker.forget();
                self.session = crate::selftest::session(doc);
                self.changed();
            }
            ids::NEW => {
                // Another document: its words are the base, not written.
                self.tracker.forget();
                self.session = crate::selftest::session(jcj_doc::new_document());
                self.changed();
            }
            ids::SAVE => self.save()?,
            ids::UNDO | ids::REDO => {
                self.session.run(&c.id, &json!({})).map_err(|e| format!("{e:?}"))?;
                self.changed();
            }
            cmd::SAMPLE => {
                // Another document: its words are the base, not written.
                self.tracker.forget();
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
        let tex = self.page_texture(ui.ctx());
        let text: Vec<String> = self.session.doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).collect();
        let mut state = self.split_state;
        jcj_shell::split::split(
            ui,
            ui.make_persistent_id("testbed-split"),
            self.split,
            &mut state,
            |ui| {
                egui::ScrollArea::both().id_salt("page").show(ui, |ui| match &tex {
                    Some(tex) => {
                        ui.add(egui::Image::new(tex).fit_to_original_size(1.0));
                    }
                    None => {
                        ui.label("The page could not be drawn.");
                    }
                });
            },
            |ui| {
                egui::ScrollArea::vertical().id_salt("text").show(ui, |ui| {
                    for p in &text {
                        ui.label(p);
                    }
                });
            },
        );
        self.split_state = state;
    }

    fn ribbon_tabs(&self) -> Vec<TabSpec> {
        vec![TabSpec::new("home", "Home", "H"), TabSpec::new("tools", "Tools", "T")]
    }

    fn ribbon_ui(&mut self, tab: &str, ui: &mut egui::Ui, shell: &mut Shell) {
        let me: &Testbed = self;
        match tab {
            "home" => {
                widgets::group(ui, shell, "Document", |ui, shell| {
                    widgets::big(ui, shell, me, &Control::new(cmd::SAMPLE).label(tl("Sample")));
                    widgets::big(ui, shell, me, &Control::new(cmd::INSERT).label(tl("Insert")));
                });
                widgets::group(ui, shell, "History", |ui, shell| {
                    widgets::small(ui, shell, me, &Control::new(ids::UNDO).icon("undo").label("Undo"));
                    widgets::small(ui, shell, me, &Control::new(ids::REDO).icon("redo").label("Redo"));
                });
                widgets::group(ui, shell, "Board", |ui, shell| {
                    widgets::big(ui, shell, me, &Control::new(cmd::ADD_CARD).label(tl("Add Card")));
                });
            }
            "tools" => {
                widgets::group(ui, shell, "Export", |ui, shell| {
                    widgets::big(ui, shell, me, &Control::new(cmd::EXPORT_PDF).label("PDF"));
                    widgets::big(ui, shell, me, &Control::new(cmd::EXPORT_ALL).label(tl("Every Format")));
                });
                widgets::group(ui, shell, "Checks", |ui, shell| {
                    widgets::big(ui, shell, me, &Control::new(cmd::SELFTEST).label(tl("Self-Test")));
                    widgets::big(ui, shell, me, &Control::new(shell_ids::PALETTE).label("Commands"));
                });
            }
            _ => {}
        }
    }

    fn panels(&self) -> Vec<PanelSpec> {
        vec![
            PanelSpec::new("board", "Board", "outline", Side::Left),
            PanelSpec::new("headings", "Headings", "outline", Side::Right),
            PanelSpec::new("log", "Log", "draft", Side::Bottom),
        ]
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
            "headings" => {
                // The document's headings as a tree (jcj_shell::tree); a click logs the heading.
                let rows: Vec<jcj_shell::tree::TreeRow<usize>> = jcj_doc::walk::body_paragraphs(&self.session.doc)
                    .into_iter()
                    .enumerate()
                    .filter(|(_, (_, p))| p.props.style.as_deref().is_some_and(|s| s.starts_with("Heading")))
                    .map(|(i, (_, p))| jcj_shell::tree::TreeRow::new(i, 0, jcj_doc::plain_text(p)))
                    .collect();
                for e in jcj_shell::tree::tree(ui, "testbed-headings", &mut self.headings, &rows) {
                    if let jcj_shell::tree::TreeEvent::Select(i) = e {
                        self.headings.selected = Some(i);
                        self.log.push(format!("heading {i} selected"));
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

    fn mark_width(&self, height: f32) -> f32 {
        crate::brand::mark_width(height)
    }

    fn paint_mark(&self, painter: &egui::Painter, rect: egui::Rect, ink: egui::Color32) {
        crate::brand::paint_mark(painter, rect, ink);
    }
}

/// What only the testbed can do at start-up; the core's start-up does the rest
/// (`jcj_shell::desktop`).
impl DesktopApp for Testbed {
    type Prefs = jcj_shell::prefs::Prefs<TestbedPrefs>;

    fn apply_prefs(&mut self, prefs: &TestbedPrefs, _shell: &mut Shell) {
        self.language = prefs.language.clone();
        // `CORE_TESTBED_LANG` is the language for one run; the preference is not changed by it.
        let code = std::env::var("CORE_TESTBED_LANG").unwrap_or_else(|_| self.language.clone());
        if !code.trim().is_empty() {
            crate::languages::set(&code);
        }
    }

    fn prefs(&self, _last: &TestbedPrefs) -> TestbedPrefs {
        TestbedPrefs { language: self.language.clone() }
    }

    fn open(&mut self, files: &[PathBuf], _shell: &mut Shell) {
        // A project file that exists is opened; any other name is where Save will write.
        if let Some(path) = files.first().filter(|p| p.is_file() && p.extension().is_some_and(|e| e == EXTENSION))
            && let Err(e) = self.open(path.clone())
        {
            self.log.push(format!("could not open {}: {e}", path.display()));
        }
    }

    fn window_title(&self) -> Option<String> {
        Some(format!("{} - {}", self.document_name(), DESKTOP.name))
    }
}

/// The first lines of the sample template, as its tile shows them (built once).
fn sample_preview() -> jcj_shell::TemplatePreview {
    static LINES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    let lines = LINES.get_or_init(|| {
        let doc = sample_document().unwrap_or_default();
        doc.body.iter().filter_map(|b| b.as_para()).map(|p| p.plain_text()).filter(|t| !t.trim().is_empty()).take(3).collect()
    });
    jcj_shell::TemplatePreview::lines(lines)
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
        let today = jcj_shell::writing::stats::Moment::of(std::time::SystemTime::now(), 0).day;
        let before = app.stats.on(today).words;
        app.run(&Cmd::new(cmd::INSERT), &mut shell).unwrap();
        assert!(app.stats.on(today).words > before, "words typed count in the day's statistics");
        app.run(&Cmd::new(ids::UNDO), &mut shell).unwrap();
        assert!(app.stats.on(today).words > before, "and undoing them takes nothing away");
        app.run(&Cmd::new(cmd::STATISTICS), &mut shell).unwrap();
        assert!(app.stats_view.open);
        assert!(app.has_document() && !app.templates().is_empty());
        let sample = app.templates().into_iter().find(|t| t.id == "sample").and_then(|t| t.preview).unwrap();
        assert_eq!(sample.text_lines().first().map(String::as_str), Some("A Sample Document"), "the sample's tile shows its first lines");
        app.run(&Cmd::new(cmd::EXPORT_ALL), &mut shell).unwrap();
        assert!(dir.join("test.epub").is_file() && dir.join("test.docx").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Open, Save As and Export To go through the shell's file dialogs, answered here by a
    /// stand-in for the system's: each answer comes back as the command with the path chosen.
    #[test]
    fn file_commands_ask_the_shell_and_run_with_the_answer() {
        use egui_kittest::Harness;
        use jcj_shell::files::{FileAnswer, FileKind};

        let dir = std::env::temp_dir().join(format!("core-testbed-files-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("out")).unwrap();
        let app = Testbed::new(dir.join("first.jcjtest"));
        let shell = Shell::new("Core testbed", "0", jcj_shell::theme::Tokens::default());
        let mut files = jcj_shell::files::FileDialogs::new();
        let answers = dir.clone();
        files.answer_with(Some(Box::new(move |ask| match ask.kind {
            FileKind::Save if ask.file_name.as_deref() == Some("first.jcjtest") => FileAnswer::Chosen(vec![answers.join("second")]),
            FileKind::Save => FileAnswer::Cancelled,
            FileKind::Folder => FileAnswer::Chosen(vec![answers.join("out")]),
            FileKind::Open | FileKind::OpenMany => FileAnswer::Chosen(vec![answers.join("second.jcjtest")]),
        })));
        type State = (Shell, jcj_shell::files::FileDialogs, Testbed);
        let mut h = Harness::builder().with_size(egui::vec2(1200.0, 800.0)).build_ui_state(
            |ui, s: &mut State| {
                let (shell, files, app) = s;
                shell.ui(ui, app);
                files.tick(shell, ui.ctx());
            },
            (shell, files, app),
        );
        let run = |h: &mut Harness<'_, State>, id: &str| {
            h.state_mut().0.run(Cmd::new(id));
            h.run();
            h.run();
            assert!(h.state().0.message.is_empty(), "{id}: {}", h.state().0.message);
        };
        run(&mut h, cmd::ADD_CARD);
        run(&mut h, ids::SAVE_AS);
        assert_eq!(h.state().2.path, dir.join("second.jcjtest"), "the extension is added");
        assert!(dir.join("second.jcjtest").is_file() && !h.state().2.is_dirty());
        // Closed without choosing: nothing is saved, and the app is told.
        run(&mut h, ids::SAVE_AS);
        assert!(h.state().2.log.iter().any(|l| l.contains("without a place chosen")));
        run(&mut h, cmd::EXPORT_TO);
        assert!(dir.join("out/second.epub").is_file() && dir.join("out/second.txt").is_file());
        run(&mut h, ids::NEW);
        assert!(h.state().2.board.cards.len() == 1 && h.state().2.is_dirty());
        run(&mut h, cmd::ADD_CARD);
        run(&mut h, ids::OPEN);
        let app = &h.state().2;
        assert!(!app.is_dirty() && app.board.cards.len() == 1, "the saved file is open again");
        assert!(app.log.iter().any(|l| l.starts_with("opened ")));
        // No dialog can be shown: the status bar says why, and the step that waited is given up.
        h.state_mut().1.answer_with(Some(Box::new(|_| FileAnswer::Unavailable("no dialog service".into()))));
        h.state_mut().0.run(Cmd::new(ids::SAVE_AS));
        h.run();
        h.run();
        let (shell, _, app) = h.state();
        assert_eq!(shell.message, "The file dialog could not be shown: no dialog service");
        assert_eq!(
            app.log.last().map(String::as_str),
            Some("Save As was closed without a place chosen (no dialog could be shown: no dialog service)")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The core's start-up runs the testbed with no window toolkit: its brand reaches the
    /// window and the shell, a project file named at start is opened, its preferences are read
    /// and written, and its title follows the document.
    #[test]
    fn the_core_s_start_up_starts_and_ends_the_testbed() {
        use jcj_shell::desktop::Window;
        use jcj_shell::prefs::Prefs;

        let dir = std::env::temp_dir().join(format!("core-testbed-desktop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let settings = dir.join("settings");
        std::fs::create_dir_all(&settings).unwrap();
        // A project saved by an earlier run, and that run's preferences.
        let project = dir.join("kept.jcjtest");
        let mut earlier = Testbed::new(project.clone());
        earlier.board.add_card("Kept", "From the earlier run.").unwrap();
        earlier.save().unwrap();
        std::fs::write(settings.join("preferences.json"), r#"{"shell": {"theme": "dark", "userName": "A. Tester"}, "app": {"language": ""}}"#)
            .unwrap();

        let viewport = desktop().viewport();
        assert_eq!((viewport.title.as_deref(), viewport.app_id.as_deref()), (Some("Core testbed"), Some("org.jcj.core-testbed")));
        assert!(viewport.icon.is_some_and(|icon| icon.width == 64), "the window has the testbed's icon");
        assert_eq!(DESKTOP.settings_dir_from(Some("elsewhere".into())), Some(PathBuf::from("elsewhere")));
        assert_eq!(DESKTOP.release_source(), None, "the testbed installs no updates");
        // An app that does says where its releases are, and the update check is told the rest.
        let releases = jcj_shell::desktop::Releases { repo: "example-org/core-testbed", tagline: "A test app", developer: "its contributors" };
        let updating = Desktop { releases: Some(releases), ..DESKTOP };
        assert_eq!(updating.release_source().map(|s| (s.id, s.repo, s.version)), Some(("core-testbed", "example-org/core-testbed", DESKTOP.version)));

        let ctx = egui::Context::default();
        let files = jcj_shell::desktop::files_named(["--quiet".into(), project.clone().into_os_string()]);
        let mut window = Window::start(&desktop(), &ctx, Some(settings.clone()), files, |start| {
            assert!(start.settings.is_some_and(|folder| folder.is_dir()));
            start.shell_with(|shell| shell.with_options(jcj_shell::ShellOptions { start_window: true, ..jcj_shell::ShellOptions::default() }));
            Testbed::new(start.files.first().cloned().unwrap_or_default())
        });
        assert!(window.shell.options.start_window);
        assert_eq!(window.shell.tokens().name, "core-testbed", "the brand's tokens");
        assert_eq!((window.shell.mode, window.shell.user_name.as_str()), (jcj_shell::theme::Mode::Dark, "A. Tester"));
        assert_eq!(window.app.board.cards.len(), 1, "the project named at start is open");
        assert_eq!(window.prefs_path(), Some(settings.join("preferences.json").as_path()));

        // A frame: the window is told its title; a change of a preference is written.
        window.shell.mode = jcj_shell::theme::Mode::Light;
        window.app.language = "es".into();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            window.logic(ui.ctx());
            window.ui_offscreen(ui);
        });
        // No picture is made of the frame: its textures are not needed.
        out.textures_delta.clear();
        let commands = out.viewport_output.get(&egui::ViewportId::ROOT).map(|v| v.commands.clone()).unwrap_or_default();
        assert!(commands.contains(&egui::ViewportCommand::Title("kept.jcjtest - Core testbed".into())), "{commands:?}");
        window.exit();
        let kept: Prefs<TestbedPrefs> = serde_json::from_str(&std::fs::read_to_string(settings.join("preferences.json")).unwrap()).unwrap();
        assert_eq!((kept.shell.theme, kept.app.language.as_str()), (jcj_shell::theme::Mode::Light, "es"));
        assert_eq!(kept.shell.user_name, "A. Tester", "what the window did not change is kept");
        drop(window);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
