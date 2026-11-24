// SPDX-License-Identifier: MIT OR Apache-2.0
//! What an app gives the shell: its document view, its ribbon tabs and panels, its commands, and
//! a few facts about the open document.

use egui::{Color32, Painter, Rect, Ui};

use crate::backstage::{Page, PageSpec};
use crate::commands::{Cmd, Commands};
use crate::ribbon::TabSpec;
use crate::shell::Shell;
use crate::workspace::PanelSpec;

/// What the About page says besides the app's name and version.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct About {
    /// A line or two on what the app is.
    pub summary: String,
    /// The credit line, plain text, shown as given.
    pub credit: String,
    /// Acknowledgments: the projects and fonts the app is built with, plain text.
    pub acknowledgments: String,
    /// The app's website, shown as a link.
    pub website: Option<String>,
}

/// An app built on the shell. Only [`App::document_name`], [`App::run`] and
/// [`App::document_ui`] (and [`Commands::command`]) must be written; everything else has a
/// default.
///
/// Drawing methods get the [`Shell`] so they can draw its controls (`jcj_shell::widgets`) and
/// queue commands; the shell runs queued commands, the app's through [`App::run`], once the frame
/// is drawn.
pub trait App: Commands {
    /// The open document's name for the quick-access row ("Untitled", a file name).
    fn document_name(&self) -> String;

    /// The open document's file, if it has one (the Save page shows it).
    fn document_path(&self) -> Option<String> {
        None
    }

    /// Whether the document has changes not saved yet.
    fn is_dirty(&self) -> bool {
        false
    }

    /// Whether the document has a file of its own (only such documents are auto-saved).
    fn has_file(&self) -> bool {
        self.document_path().is_some()
    }

    /// A number that changes whenever the document does: auto-save's clock starts at a change.
    fn revision(&self) -> u64 {
        0
    }

    /// Run one of the app's commands. An error is shown in the status bar.
    fn run(&mut self, cmd: &Cmd, shell: &mut Shell) -> Result<(), String>;

    /// Save the document now: auto-save calls it when a save is due. The file work is the app's
    /// (with `jcj-project`); the shell only schedules it.
    fn autosave(&mut self) -> Result<(), String> {
        Ok(())
    }

    /// Draw the document (the central area).
    fn document_ui(&mut self, ui: &mut Ui, shell: &mut Shell);

    /// The ribbon's tabs now (a contextual tab may come and go).
    fn ribbon_tabs(&self) -> Vec<TabSpec> {
        Vec::new()
    }

    /// Draw tab `tab`'s groups (`jcj_shell::widgets::group` and the controls in it).
    fn ribbon_ui(&mut self, tab: &str, ui: &mut Ui, shell: &mut Shell) {
        let _ = (tab, ui, shell);
    }

    /// The side panels the app offers.
    fn panels(&self) -> Vec<PanelSpec> {
        Vec::new()
    }

    /// Draw panel `panel`'s contents.
    fn panel_ui(&mut self, panel: &str, ui: &mut Ui, shell: &mut Shell) {
        let _ = (panel, ui, shell);
    }

    /// Draw the app's part of the status bar (its left side).
    fn status_ui(&mut self, ui: &mut Ui, shell: &mut Shell) {
        let _ = (ui, shell);
    }

    /// The app's own File pages, shown after the shell's (before Preferences).
    fn file_pages(&self) -> Vec<PageSpec> {
        Vec::new()
    }

    /// Draw the app's part of a File page: all of an app page ([`Page::App`]), or what follows
    /// the shell's content on a built-in one (templates on New, formats on Export, print
    /// choices on Print).
    fn file_page_ui(&mut self, page: &Page, ui: &mut Ui, shell: &mut Shell) {
        let _ = (page, ui, shell);
    }

    /// Draw the app's own preferences, after the shell's on the Preferences page.
    fn preferences_ui(&mut self, ui: &mut Ui, shell: &mut Shell) {
        let _ = (ui, shell);
    }

    /// What the About page says.
    fn about(&self) -> About {
        About::default()
    }

    /// Whether a document is open. When it is not and the app turned on
    /// [`crate::ShellOptions::start_window`], the shell shows the start window instead of
    /// [`App::document_ui`].
    fn has_document(&self) -> bool {
        true
    }

    /// The templates new documents can start from. When there are any, the File page's New page
    /// and the start window show them; picking one runs `file.newFromTemplate` with
    /// `{"template": id}` ([`crate::start`]).
    fn templates(&self) -> Vec<crate::start::TemplateSpec> {
        Vec::new()
    }

    /// The width of the app's mark at `height` points (0: no mark).
    fn mark_width(&self, height: f32) -> f32 {
        let _ = height;
        0.0
    }

    /// Paint the app's mark in `rect` in `ink` (the quick-access row, the About page).
    fn paint_mark(&self, painter: &Painter, rect: Rect, ink: Color32) {
        let _ = (painter, rect, ink);
    }
}
