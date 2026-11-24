// SPDX-License-Identifier: MIT OR Apache-2.0
//! Software updates as the window shows them.
//!
//! [`Updates`] holds what to show and what the user asked for; the work (checking the app's
//! GitHub releases, downloading, verifying, installing) is [`desktop::Updater`]'s, with
//! craft-update on threads of its own. A build that cannot update (a web build, a development
//! build without a release source) keeps the status at [`Status::Unsupported`] or
//! [`Status::Idle`] and has nothing to run.

use egui::{Context, RichText, Ui};
use jcj_theme::Theme;

use crate::commands::{Cmd, shell_ids};
use crate::shell::Shell;

#[cfg(not(target_arch = "wasm32"))]
pub mod desktop;

/// Where an update stands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Status {
    /// Nothing checked yet in this run.
    #[default]
    Idle,
    /// A check is under way.
    Checking,
    /// The latest release is this version or older.
    Current,
    /// A newer release.
    Available(Offer),
    /// Downloading, verifying or installing; the words say which.
    Installing(String),
    /// The new version is in place; it starts when the app restarts.
    Ready(String),
    /// The update did not finish, and why.
    Failed(String),
    /// This run is the version an update installed.
    Updated(String),
    /// This build can't check, and why.
    Unsupported(String),
}

/// A newer release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    /// Its version (`0.2.0`).
    pub version: String,
    /// The release's page, for its notes and its files.
    pub page: String,
    /// Whether this copy can install it itself. When it can't (an unsigned copy, a development
    /// build, a bundle the updater doesn't replace), the user gets the release page instead.
    pub installs: bool,
}

/// What the user asked the updater to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    /// Check now.
    Check,
    /// Install the release offered.
    Install,
    /// Close the window so the installed version starts.
    Restart,
}

/// The update state the window keeps.
#[derive(Debug, Default)]
pub struct Updates {
    /// Where an update stands.
    pub status: Status,
    /// Check once when the app starts (a preference, on by default).
    pub check_at_start: bool,
    /// Requests the updater has not taken yet.
    pub requests: Vec<Request>,
    /// Show the window offering the update: set when a check at start finds one, or when a
    /// requested check ends.
    pub prompt: bool,
}

impl Updates {
    /// The requests since the updater last asked, oldest first.
    pub fn take_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.requests)
    }

    /// A check the user asked for: its answer is always shown.
    pub fn check_now(&mut self) {
        if matches!(self.status, Status::Unsupported(_)) {
            self.prompt = true;
            return;
        }
        if !matches!(self.status, Status::Checking | Status::Installing(_)) {
            self.requests.push(Request::Check);
            self.status = Status::Checking;
        }
        self.prompt = true;
    }

    /// The words for the status of app `name` running `version`, and whether it is a problem.
    pub fn describe(&self, name: &str, version: &str) -> (String, bool) {
        match &self.status {
            Status::Idle => (format!("{name} {version}."), false),
            Status::Checking => ("Checking for updates…".into(), false),
            Status::Current => (format!("{name} {version} is the latest version."), false),
            Status::Available(o) if o.installs => (format!("{name} {} is available. You have {version}.", o.version), false),
            Status::Available(o) => {
                (format!("{name} {} is available. You have {version}. Download it from the release page and replace this copy.", o.version), false)
            }
            Status::Installing(what) => (what.clone(), false),
            Status::Ready(v) => (format!("{name} {v} is installed. Restart to use it; your documents are kept."), false),
            Status::Failed(why) => (format!("The update did not finish: {why}"), true),
            Status::Updated(v) => (format!("{name} was updated to {v}."), false),
            Status::Unsupported(why) => (why.clone(), false),
        }
    }
}

/// The buttons that fit the status, under its words. Shared by the update window and the
/// About page.
pub fn actions(shell: &mut Shell, ui: &mut Ui) {
    let t = Theme::get(ui.ctx());
    let (text, problem) = shell.updates.describe(&shell.name, &shell.version);
    ui.label(RichText::new(text).color(if problem { t.danger } else { t.text }));
    ui.add_space(6.0);
    ui.horizontal(|ui| match shell.updates.status.clone() {
        Status::Available(offer) => {
            if offer.installs && ui.button("Install and Restart").clicked() {
                shell.updates.requests.push(Request::Install);
                shell.updates.status = Status::Installing("Downloading…".into());
            }
            if ui.button(if offer.installs { "Release Notes" } else { "Open Release Page" }).clicked() {
                ui.ctx().open_url(egui::OpenUrl::new_tab(&offer.page));
            }
        }
        Status::Ready(_) => {
            if ui.button("Restart Now").clicked() {
                shell.updates.requests.push(Request::Restart);
            }
        }
        Status::Checking | Status::Installing(_) => {
            ui.spinner();
        }
        Status::Unsupported(_) => {}
        _ => {
            if ui.button("Check for Updates").clicked() {
                shell.run(Cmd::new(shell_ids::CHECK_FOR_UPDATES));
            }
        }
    });
}

/// The window that offers an update or answers a check.
pub(crate) fn window(shell: &mut Shell, ctx: &Context) {
    if !shell.updates.prompt {
        return;
    }
    let mut open = true;
    egui::Window::new("Software Update").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
        ui.set_max_width(380.0);
        actions(shell, ui);
    });
    if !open {
        shell.updates.prompt = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_check_asked_for_is_queued_once_and_answered() {
        let mut u = Updates::default();
        u.check_now();
        u.check_now();
        assert_eq!(u.take_requests(), [Request::Check]);
        assert!(u.prompt);
        assert_eq!(u.describe("Sample", "1.0").0, "Checking for updates…");
        u.status = Status::Unsupported("This build can't update itself.".into());
        u.check_now();
        assert!(u.take_requests().is_empty());
        u.status = Status::Available(Offer { version: "2.0".into(), page: String::new(), installs: false });
        assert!(u.describe("Sample", "1.0").0.contains("release page"));
        u.status = Status::Failed("offline".into());
        assert_eq!(u.describe("Sample", "1.0"), ("The update did not finish: offline".into(), true));
    }
}
