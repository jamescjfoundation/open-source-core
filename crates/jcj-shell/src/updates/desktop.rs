// SPDX-License-Identifier: MIT OR Apache-2.0
//! Checking for, downloading and installing updates with craft-update.
//!
//! Releases come only from the app's own GitHub repository ([`ReleaseSource::repo`]). Every
//! download is checked against the release's SHA-256 before anything is replaced, and a copy that
//! can't replace itself (a development build, an unsigned Windows copy, a bundle the updater
//! doesn't manage) is offered the release page instead. The work runs on threads; the window only
//! sees [`super::Updates`].

use std::sync::mpsc::{Receiver, Sender, channel};

use jcj_upstream::update::self_update::{self, Outcome, Restart};
use jcj_upstream::update::{App as UpdateApp, Http, JobPhase, Paths, Platform, Release, Stop};

use super::{Offer, Request, Status, Updates};

/// Where an app's releases come from, and how they are named.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReleaseSource {
    /// The stem of every release file (`<id>-<version>-<os>-<arch>.<ext>`) and the app's
    /// machine name.
    pub id: &'static str,
    /// The app's name as installers show it.
    pub name: &'static str,
    /// What it's for, in a few words (installers' menu entries).
    pub tagline: &'static str,
    /// The GitHub `owner/repo` releases are read and downloaded from, and only from.
    pub repo: &'static str,
    /// Who makes it, as the update prompt says.
    pub developer: &'static str,
    /// The version running (`env!("CARGO_PKG_VERSION")`).
    pub version: &'static str,
    /// Whether this platform can install an update in place. The app turns it off where the
    /// updater can't (craft-update at our pin replaces a macOS app only when it carries ArtCraft's
    /// own Developer ID).
    pub installs_in_place: bool,
}

impl ReleaseSource {
    fn app(&self) -> UpdateApp {
        UpdateApp { developer: self.developer, ..UpdateApp::new(self.id, self.name, self.tagline, self.repo) }
    }

    /// The releases page.
    pub fn releases_url(&self) -> String {
        self.app().releases_url()
    }
}

enum Message {
    Checked { found: Result<Option<Release>, String>, asked: bool },
    Phase(String),
    Installed(Result<Restart, String>),
}

/// The desktop side of updates: the worker threads' answers and the update waiting for the exit.
pub struct Updater {
    source: ReleaseSource,
    tx: Sender<Message>,
    rx: Receiver<Message>,
    /// The release last found newer, for Install.
    offered: Option<Release>,
    /// The helper that starts the new version once this process ends.
    restart: Option<Restart>,
    checked_at_start: bool,
}

impl std::fmt::Debug for Updater {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Updater").field("source", &self.source).field("checked_at_start", &self.checked_at_start).finish_non_exhaustive()
    }
}

impl Updater {
    /// An updater for `source`.
    pub fn new(source: ReleaseSource) -> Self {
        let (tx, rx) = channel();
        Updater { source, tx, rx, offered: None, restart: None, checked_at_start: false }
    }

    /// What the last run's update came to, shown once in this run.
    pub fn read_outcome(&self, ui: &mut Updates) {
        let Ok(paths) = Paths::for_app(&self.source.app()) else { return };
        match self_update::take_outcome(&paths, self.source.version) {
            Some(Outcome::Installed(v)) => ui.status = Status::Updated(v.to_string()),
            Some(Outcome::Failed(why)) => {
                ui.status = Status::Failed(why);
                ui.prompt = true;
            }
            None => {}
        }
    }

    /// Take the window's requests and the workers' answers. Returns true when the window should
    /// close so the installed version starts.
    pub fn tick(&mut self, ui: &mut Updates, ctx: &egui::Context) -> bool {
        if !self.checked_at_start {
            self.checked_at_start = true;
            if ui.check_at_start && ui.status == Status::Idle {
                ui.status = Status::Checking;
                self.check(ctx, false);
            }
        }
        let mut close = false;
        for request in ui.take_requests() {
            match request {
                Request::Check => self.check(ctx, true),
                Request::Install => self.install(ui, ctx),
                Request::Restart => close = self.restart.is_some(),
            }
        }
        while let Ok(message) = self.rx.try_recv() {
            match message {
                Message::Checked { found, asked } => self.checked(ui, found, asked),
                Message::Phase(words) => {
                    if matches!(ui.status, Status::Installing(_)) {
                        ui.status = Status::Installing(words);
                    }
                }
                Message::Installed(Ok(restart)) => {
                    self.restart = Some(restart);
                    let v = self.offered.as_ref().map_or_else(String::new, Release::version_label);
                    ui.status = Status::Ready(v);
                    ui.prompt = true;
                }
                Message::Installed(Err(why)) => {
                    ui.status = Status::Failed(why);
                    ui.prompt = true;
                }
            }
        }
        if matches!(ui.status, Status::Checking | Status::Installing(_)) {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
        close
    }

    /// Start the helper of an installed update. Call once the exit is certain.
    pub fn finish(&mut self) {
        if let Some(restart) = self.restart.take()
            && let Err(e) = restart.start()
        {
            log::warn!("the updated version could not be started: {e}");
        }
    }

    fn check(&self, ctx: &egui::Context, asked: bool) {
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let source = self.source;
        let spawned = std::thread::Builder::new().name("update-check".into()).spawn(move || {
            let found = http(&source).and_then(|http| jcj_upstream::update::check_for_update(&source.app(), &http, source.version));
            let _ = tx.send(Message::Checked { found, asked });
            ctx.request_repaint();
        });
        if let Err(e) = spawned {
            let _ = self.tx.send(Message::Checked { found: Err(format!("could not start the check ({e})")), asked });
        }
    }

    fn checked(&mut self, ui: &mut Updates, found: Result<Option<Release>, String>, asked: bool) {
        let app = self.source.app();
        match found {
            Ok(Some(release)) => {
                let installs = self.source.installs_in_place && self_update::is_self_updating(&app) && self_update::release_installs(&app, &release);
                let page = if release.html_url.starts_with("https://github.com/") { release.html_url.clone() } else { app.releases_url() };
                ui.status = Status::Available(Offer { version: release.version_label(), page, installs });
                self.offered = Some(release);
                ui.prompt = true;
            }
            Ok(None) => {
                ui.status = Status::Current;
                ui.prompt |= asked;
            }
            // A check at start that fails says nothing: the user didn't ask, and may be offline.
            Err(why) if asked => {
                ui.status = Status::Failed(why);
                ui.prompt = true;
            }
            Err(_) => ui.status = Status::Idle,
        }
    }

    fn install(&mut self, ui: &mut Updates, ctx: &egui::Context) {
        let Some(release) = self.offered.clone() else {
            ui.status = Status::Failed("there is no update to install; check again".into());
            return;
        };
        let installed = if self.source.installs_in_place { self_update::current(&self.source.app()) } else { None };
        let Some(installed) = installed else {
            ui.status = Status::Failed("this copy can't replace itself; download the new version from the release page".into());
            return;
        };
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let source = self.source;
        let spawned = std::thread::Builder::new().name("update-install".into()).spawn(move || {
            let result = install(&source, &installed, release, &tx, &ctx);
            let _ = tx.send(Message::Installed(result));
            ctx.request_repaint();
        });
        if let Err(e) = spawned {
            ui.status = Status::Failed(format!("could not start the update ({e})"));
        }
    }
}

fn http(source: &ReleaseSource) -> Result<Http, String> {
    // The product and its version, nothing about the user.
    Http::new(&format!("{}/{}", source.name.replace(' ', ""), source.version))
}

fn install(
    source: &ReleaseSource,
    installed: &self_update::Install,
    mut release: Release,
    tx: &Sender<Message>,
    ctx: &egui::Context,
) -> Result<Restart, String> {
    let app = source.app();
    let http = http(source)?;
    // A release found by the list may come without its files; read it again in full.
    if release.assets.is_empty()
        && let Some(full) = jcj_upstream::update::check_for_update(&app, &http, source.version)?
    {
        release = full;
    }
    let paths = Paths::for_app(&app)?;
    let platform = Platform::current().ok_or("updates aren't available on this platform")?;
    let mut phase = |p: JobPhase| {
        let _ = tx.send(Message::Phase(describe(&p)));
        ctx.request_repaint();
    };
    self_update::prepare_update(&app, installed, &release, platform, &http, &paths, &Stop::default(), &mut phase, &|| Ok(()), Some(source.version))
}

fn describe(phase: &JobPhase) -> String {
    let mb = |b: u64| b as f64 / 1_048_576.0;
    match phase {
        JobPhase::Downloading { done, total } if *total > 0 => {
            let (done, total) = (format!("{:.1}", mb(*done)), format!("{:.1}", mb(*total)));
            jcj_i18n::tr_with("Downloading… {done} of {total} MB", &[("done", &done), ("total", &total)])
        }
        JobPhase::Downloading { .. } => jcj_i18n::tr("Downloading…"),
        JobPhase::Reconnecting { .. } => jcj_i18n::tr("The connection dropped; trying again…"),
        JobPhase::Verifying => jcj_i18n::tr("Checking the download…"),
        JobPhase::Waiting => jcj_i18n::tr("Waiting for the system installer…"),
        JobPhase::Installing => jcj_i18n::tr("Installing…"),
        JobPhase::Removing => jcj_i18n::tr("Removing the old version…"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: ReleaseSource = ReleaseSource {
        id: "sample-app",
        name: "Sample App",
        tagline: "A test app",
        repo: "example-org/sample-app",
        developer: "the sample contributors",
        version: "1.0.0",
        installs_in_place: false,
    };

    #[test]
    fn releases_come_from_the_app_s_repository() {
        assert_eq!(SOURCE.releases_url(), "https://github.com/example-org/sample-app/releases");
        assert_eq!(SOURCE.app().developer, "the sample contributors");
    }

    #[test]
    fn a_copy_that_cannot_install_is_sent_to_the_release_page() {
        let mut u = Updater::new(SOURCE);
        let mut ui = Updates::default();
        let release = Release { tag: "v99.0.0".into(), version: jcj_upstream::update::parse_version("99.0.0"), ..Default::default() };
        u.checked(&mut ui, Ok(Some(release)), true);
        let Status::Available(offer) = ui.status.clone() else { panic!("{:?}", ui.status) };
        assert_eq!(offer.version, "99.0.0");
        assert!(!offer.installs);
        assert!(offer.page.starts_with("https://github.com/example-org/sample-app/releases"));
        u.install(&mut ui, &egui::Context::default());
        assert!(matches!(ui.status, Status::Failed(_)));
    }

    #[test]
    fn a_check_at_start_that_fails_stays_quiet_and_an_asked_one_says_why() {
        let mut u = Updater::new(SOURCE);
        let mut ui = Updates::default();
        u.checked(&mut ui, Err("offline".into()), false);
        assert_eq!((ui.status.clone(), ui.prompt), (Status::Idle, false));
        u.checked(&mut ui, Err("offline".into()), true);
        assert_eq!((ui.status.clone(), ui.prompt), (Status::Failed("offline".into()), true));
        u.checked(&mut ui, Ok(None), false);
        assert_eq!(ui.status, Status::Current);
    }
}
