// SPDX-License-Identifier: MIT OR Apache-2.0
//! Preferences, kept between runs as JSON in the app's settings folder.
//!
//! The file holds the shell's own preferences ([`ShellPrefs`]: theme, auto-save, recent files,
//! keys, the panels' layout, ...) and the app's own, whatever struct the app keeps them in:
//! `{"shell": {...}, "app": {...}}` ([`Prefs`]). [`PrefsFile`] reads and writes any such
//! struct. The file is written atomically; a missing key takes its default, an unreadable file is
//! moved aside and the defaults are used, and values are bounded when they are applied, so a
//! hand-edited or damaged file cannot ask for something unreasonable.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use jcj_theme::Mode;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::workspace::Layout;

/// Most minutes between auto-saves.
pub const MAX_AUTOSAVE_MINUTES: u32 = 240;
/// Most idle seconds an auto-save waits for.
pub const MAX_IDLE_SECONDS: u32 = 600;
/// Most recent files listed.
pub const MAX_RECENT: usize = 12;

/// Auto-save preferences.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoSavePrefs {
    /// Whether auto-save is on.
    pub enabled: bool,
    /// Minutes between auto-saves (1 to [`MAX_AUTOSAVE_MINUTES`]).
    pub minutes: u32,
    /// Seconds of quiet an auto-save waits for (1 to [`MAX_IDLE_SECONDS`]).
    pub idle_seconds: u32,
}

impl Default for AutoSavePrefs {
    fn default() -> Self {
        AutoSavePrefs { enabled: true, minutes: 15, idle_seconds: 10 }
    }
}

/// The shell's own preferences.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ShellPrefs {
    /// The theme.
    pub theme: Mode,
    /// Auto-save.
    pub autosave: AutoSavePrefs,
    /// Recent documents, newest first.
    pub recent: Vec<String>,
    /// Open the documents that were open when the app last closed.
    pub reopen_last: bool,
    /// The documents open when the app last closed.
    pub last_open: Vec<String>,
    /// The user's name, for notes and tracked changes.
    pub user_name: String,
    /// The user's own keys: shortcut → command id (`""` turns a key off).
    pub keys: BTreeMap<String, String>,
    /// The panels' layout.
    pub layout: Layout,
    /// The ribbon shows its tabs only.
    pub ribbon_collapsed: bool,
    /// The first-run tour has been seen.
    pub tour_seen: bool,
    /// Check for a newer release when the app starts.
    pub check_updates: bool,
}

impl Default for ShellPrefs {
    fn default() -> Self {
        ShellPrefs {
            theme: Mode::Light,
            autosave: AutoSavePrefs::default(),
            recent: Vec::new(),
            reopen_last: false,
            last_open: Vec::new(),
            user_name: String::new(),
            keys: BTreeMap::new(),
            layout: Layout::default(),
            ribbon_collapsed: false,
            tour_seen: false,
            check_updates: true,
        }
    }
}

impl ShellPrefs {
    /// The preferences with every value within bounds.
    pub fn bounded(mut self) -> Self {
        self.autosave.minutes = self.autosave.minutes.clamp(1, MAX_AUTOSAVE_MINUTES);
        self.autosave.idle_seconds = self.autosave.idle_seconds.clamp(1, MAX_IDLE_SECONDS);
        self.recent = clean_paths(&self.recent);
        self.last_open = clean_paths(&self.last_open);
        self.user_name = self.user_name.chars().filter(|c| !c.is_control()).take(80).collect::<String>().trim().to_string();
        self.keys = self.keys.into_iter().filter(|(k, v)| k.len() <= 40 && v.len() <= 120).take(crate::keys::MAX_USER_KEYS).collect();
        self
    }

    /// Auto-save's interval.
    pub fn autosave_every(&self) -> Duration {
        Duration::from_secs(u64::from(self.autosave.minutes.clamp(1, MAX_AUTOSAVE_MINUTES)) * 60)
    }

    /// Auto-save's idle time.
    pub fn autosave_idle(&self) -> Duration {
        Duration::from_secs(u64::from(self.autosave.idle_seconds.clamp(1, MAX_IDLE_SECONDS)))
    }
}

fn clean_paths(paths: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in paths {
        if !p.is_empty() && p.len() < 4_096 && !p.chars().any(char::is_control) && !out.contains(p) {
            out.push(p.clone());
        }
        if out.len() == MAX_RECENT {
            break;
        }
    }
    out
}

/// Put `path` first in a recent list, at most [`MAX_RECENT`] long.
pub fn add_recent(recent: &mut Vec<String>, path: &str) {
    recent.retain(|p| p != path);
    recent.insert(0, path.to_string());
    recent.truncate(MAX_RECENT);
}

/// A preferences file's contents: the shell's preferences and the app's own.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs<A> {
    /// The shell's.
    pub shell: ShellPrefs,
    /// The app's.
    pub app: A,
}

/// What a preferences file holds, for code that works with any app's file: the shell's part and
/// the app's. [`Prefs`] is the usual one. An app whose file has a history of its own (an older
/// shape it still reads, a first start that differs from the defaults) keeps its own struct
/// and says here where the two parts are.
pub trait Contents: Serialize + DeserializeOwned + Default + PartialEq + Clone {
    /// The app's own preferences.
    type App;

    /// The shell's part.
    fn shell(&self) -> &ShellPrefs;

    /// The app's part.
    fn app(&self) -> &Self::App;

    /// A file's contents from its two parts.
    fn of(shell: ShellPrefs, app: Self::App) -> Self;
}

impl<A: Serialize + DeserializeOwned + Default + PartialEq + Clone> Contents for Prefs<A> {
    type App = A;

    fn shell(&self) -> &ShellPrefs {
        &self.shell
    }

    fn app(&self) -> &A {
        &self.app
    }

    fn of(shell: ShellPrefs, app: A) -> Self {
        Prefs { shell, app }
    }
}

/// The preferences of an app that keeps none of its own yet (`"app": {}` in the file).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoPrefs {}

/// Largest preferences file read, bytes.
pub const MAX_PREFS_BYTES: u64 = 1 << 20;
/// Least time between two writes of the file while preferences change (a slider being dragged).
pub const WRITE_INTERVAL: Duration = Duration::from_secs(1);

/// A preferences file and what was last written to it. `P` is the file's contents, usually
/// [`Prefs`] with the app's own struct.
#[derive(Debug)]
pub struct PrefsFile<P> {
    /// The file.
    pub path: PathBuf,
    written: P,
    last_write: Option<Instant>,
}

impl<P: Serialize + DeserializeOwned + Default + PartialEq + Clone> PrefsFile<P> {
    /// The file's name in the settings folder.
    pub const NAME: &'static str = "preferences.json";

    /// Read the preferences in `dir`. Never fails: a missing file gives the defaults; a damaged
    /// one is moved aside (`preferences.json.damaged`) and also gives the defaults, with a message
    /// to show.
    pub fn open(dir: &Path) -> (PrefsFile<P>, P, Option<String>) {
        let path = dir.join(Self::NAME);
        let read = || -> Result<Option<P>, String> {
            let meta = match std::fs::metadata(&path) {
                Ok(m) => m,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(e) => return Err(e.to_string()),
            };
            if meta.len() > MAX_PREFS_BYTES {
                return Err("the file is too large".into());
            }
            let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            serde_json::from_str(&text).map(Some).map_err(|e| e.to_string())
        };
        let (prefs, problem) = match read() {
            Ok(p) => (p.unwrap_or_default(), None),
            Err(e) => {
                let _ = std::fs::rename(&path, dir.join(format!("{}.damaged", Self::NAME)));
                (P::default(), Some(jcj_i18n::tr_with("Preferences could not be read ({why}); the defaults are used.", &[("why", &e.to_string())])))
            }
        };
        (PrefsFile { path, written: prefs.clone(), last_write: None }, prefs, problem)
    }

    /// Write `prefs` if they changed since the last write and the last write was at least
    /// [`WRITE_INTERVAL`] ago (`force` writes now, as on quitting). Returns how long to wait
    /// before calling again when a change is waiting.
    pub fn save(&mut self, prefs: &P, now: Instant, force: bool) -> Result<Option<Duration>, String> {
        if *prefs == self.written {
            return Ok(None);
        }
        if !force && let Some(last) = self.last_write {
            let next = last + WRITE_INTERVAL;
            if now < next {
                return Ok(Some(next - now));
            }
        }
        let text = serde_json::to_string_pretty(prefs).map_err(|e| e.to_string())?;
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        self.last_write = Some(now);
        jcj_upstream::atomic::atomic_write(&self.path, text.as_bytes()).map_err(|e| format!("{}: {e}", self.path.display()))?;
        self.written = prefs.clone();
        Ok(None)
    }
}

/// The settings folder of the app named `name` (`id` is its lowercase, hyphenated form): macOS
/// `~/Library/Application Support/<name>`, Windows `%APPDATA%\<name>`, elsewhere
/// `$XDG_CONFIG_HOME/<id>` or `~/.config/<id>`. The platform rule is the one WordCraft's
/// settings folder follows; a profile folder set for an isolated run is used as it is.
pub fn settings_dir(id: &str, name: &str) -> Option<PathBuf> {
    let upstream = jcj_upstream::control_key::settings_dir()?;
    // WordCraft's own folder: swap its last part for this app's. Anything else is an override
    // (an isolated profile), kept whole.
    let last = upstream.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if !last.eq_ignore_ascii_case("wordcraft") {
        return Some(upstream);
    }
    let own = if cfg!(any(target_os = "macos", windows)) { name } else { id };
    upstream.parent().map(|p| p.join(own))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    struct AppPrefs {
        zoom: f32,
        page_view: bool,
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("jcj-shell-prefs-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn preferences_round_trip_with_the_app_s_own() {
        let dir = temp_dir("round");
        let (mut file, prefs, problem) = PrefsFile::<Prefs<AppPrefs>>::open(&dir);
        assert_eq!((prefs.clone(), problem), (Prefs::default(), None));
        let mut p = prefs;
        p.shell.theme = Mode::ExtraDark;
        p.shell.keys.insert("Mod+K".into(), "edit.find".into());
        add_recent(&mut p.shell.recent, "/docs/a.doc");
        add_recent(&mut p.shell.recent, "/docs/b.doc");
        add_recent(&mut p.shell.recent, "/docs/a.doc");
        p.app = AppPrefs { zoom: 1.5, page_view: true };
        let t0 = Instant::now();
        assert_eq!(file.save(&p, t0, false), Ok(None));
        let (_, back, problem) = PrefsFile::<Prefs<AppPrefs>>::open(&dir);
        assert_eq!(problem, None);
        assert_eq!(back, p);
        assert_eq!(back.shell.recent, ["/docs/a.doc", "/docs/b.doc"]);
        // Another change within a second waits, unless forced.
        p.app.zoom = 2.0;
        assert_eq!(file.save(&p, t0 + Duration::from_millis(300), false), Ok(Some(Duration::from_millis(700))));
        assert_eq!(file.save(&p, t0 + Duration::from_millis(300), true), Ok(None));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_s_two_parts_are_reached_through_contents() {
        let shell = ShellPrefs { user_name: "A. Writer".into(), ..ShellPrefs::default() };
        let whole = <Prefs<AppPrefs> as Contents>::of(shell.clone(), AppPrefs { zoom: 2.0, page_view: false });
        assert_eq!((Contents::shell(&whole), Contents::app(&whole).zoom), (&shell, 2.0));
        // An app with no preferences of its own still has a whole file.
        let none = <Prefs<NoPrefs> as Contents>::of(shell, NoPrefs {});
        let text = serde_json::to_string(&none).unwrap();
        assert!(text.contains(r#""app":{}"#), "{text}");
        assert_eq!(serde_json::from_str::<Prefs<NoPrefs>>(&text).unwrap(), none);
        assert_eq!(serde_json::from_str::<Prefs<NoPrefs>>(r#"{"shell": {}}"#).unwrap(), Prefs::default());
    }

    #[test]
    fn a_damaged_file_is_moved_aside() {
        let dir = temp_dir("damaged");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("preferences.json"), "{ not json").unwrap();
        let (_, prefs, problem) = PrefsFile::<Prefs<AppPrefs>>::open(&dir);
        assert_eq!(prefs, Prefs::default());
        assert!(problem.is_some_and(|m| m.contains("defaults")));
        assert!(dir.join("preferences.json.damaged").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_keys_take_defaults_and_values_are_bounded() {
        let p: Prefs<AppPrefs> = serde_json::from_str(r#"{"shell": {"autosave": {"minutes": 0}, "userName": "  A\u0007B  "}}"#).unwrap();
        assert!(p.shell.check_updates, "missing: the default");
        let s = p.shell.bounded();
        assert_eq!(s.autosave.minutes, 1);
        assert_eq!(s.autosave.idle_seconds, 10);
        assert_eq!(s.user_name, "AB");
        assert_eq!(s.autosave_every(), Duration::from_secs(60));
        let many: Vec<String> = (0..30).map(|i| format!("/f{i}")).chain(["".into(), "/f1".into()]).collect();
        assert_eq!(clean_paths(&many).len(), MAX_RECENT);
    }

    #[test]
    fn the_settings_folder_is_the_app_s_own() {
        let Some(dir) = settings_dir("sample-app", "Sample App") else { return };
        let last = dir.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        // Under an isolated profile the folder is the profile; otherwise it is named for the app.
        assert!(last == "sample-app" || last == "Sample App" || std::env::var_os("WORDCRAFT_PROFILE_DIR").is_some(), "{}", dir.display());
    }
}
