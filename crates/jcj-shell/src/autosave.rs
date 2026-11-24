// SPDX-License-Identifier: MIT OR Apache-2.0
//! When to auto-save: on by default, every 15 minutes, after about 10 seconds idle.
//!
//! The clock starts at the first change after a save. A save is due once that change is at least
//! the interval old and nothing has been typed for the idle time, so an auto-save never lands in
//! the middle of a sentence. Only a document with a file of its own is auto-saved; an untitled
//! one is kept by crash recovery until the user saves it. This module only schedules: the app
//! supplies the save itself (the file work is `jcj-project`'s). Times are passed in, so tests run
//! on a made-up clock.

use std::time::{Duration, Instant};

/// Auto-save settings and state.
#[derive(Clone, Debug)]
pub struct AutoSave {
    /// Whether auto-save is on.
    pub enabled: bool,
    /// How old the first unsaved change must be.
    pub every: Duration,
    /// How long typing must have paused.
    pub idle: Duration,
    /// The first change since the last save.
    dirty_since: Option<Instant>,
    /// The last change.
    last_edit: Option<Instant>,
    /// The revision last seen by [`AutoSave::observe`].
    seen_revision: Option<u64>,
}

impl Default for AutoSave {
    fn default() -> Self {
        AutoSave {
            enabled: true,
            every: Duration::from_secs(15 * 60),
            idle: Duration::from_secs(10),
            dirty_since: None,
            last_edit: None,
            seen_revision: None,
        }
    }
}

/// What [`AutoSave::tick`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tick {
    /// Nothing is due.
    Idle,
    /// A save is waiting; it is due at this time (for a repaint then).
    Waiting(Instant),
    /// The save ran and succeeded.
    Saved,
    /// The save ran and failed; the clock keeps running, so it is tried again after the idle time.
    Failed(String),
}

impl AutoSave {
    /// The document changed at `now`.
    pub fn edited(&mut self, now: Instant) {
        self.dirty_since.get_or_insert(now);
        self.last_edit = Some(now);
    }

    /// Note the document's revision at `now`: a new one counts as a change. The first revision
    /// seen is the starting point, not a change.
    pub fn observe(&mut self, revision: u64, now: Instant) {
        if let Some(seen) = self.seen_revision
            && seen != revision
        {
            self.edited(now);
        }
        self.seen_revision = Some(revision);
    }

    /// The document was saved (by hand or automatically): the clock starts again at the next
    /// change.
    pub fn saved(&mut self) {
        self.dirty_since = None;
    }

    /// When a save becomes due, if one is waiting.
    pub fn due_at(&self, dirty: bool, has_file: bool) -> Option<Instant> {
        if !self.enabled || !dirty || !has_file {
            return None;
        }
        let since = self.dirty_since?;
        let last = self.last_edit.unwrap_or(since);
        Some((since + self.every).max(last + self.idle))
    }

    /// Whether to save now.
    pub fn due(&self, now: Instant, dirty: bool, has_file: bool) -> bool {
        self.due_at(dirty, has_file).is_some_and(|at| now >= at)
    }

    /// Run `save` if a save is due at `now`. A document that is no longer dirty (saved by hand)
    /// restarts the clock.
    pub fn tick(&mut self, now: Instant, dirty: bool, has_file: bool, save: impl FnOnce() -> Result<(), String>) -> Tick {
        if !dirty {
            self.saved();
            return Tick::Idle;
        }
        match self.due_at(dirty, has_file) {
            None => Tick::Idle,
            Some(at) if now < at => Tick::Waiting(at),
            Some(_) => match save() {
                Ok(()) => {
                    self.saved();
                    Tick::Saved
                }
                Err(why) => {
                    // Try again once typing pauses again, not every frame.
                    self.last_edit = Some(now);
                    Tick::Failed(why)
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: Duration = Duration::from_secs(60);
    const SEC: Duration = Duration::from_secs(1);

    #[test]
    fn saves_after_the_interval_once_typing_pauses() {
        let t0 = Instant::now();
        let mut a = AutoSave::default();
        assert!(!a.due(t0 + 60 * MIN, true, true), "nothing changed yet");
        a.edited(t0);
        assert!(!a.due(t0 + 14 * MIN, true, true));
        // Still typing at 15 minutes: wait for 10 seconds of quiet.
        a.edited(t0 + 15 * MIN - SEC);
        assert!(!a.due(t0 + 15 * MIN, true, true));
        assert!(!a.due(t0 + 15 * MIN + 8 * SEC, true, true));
        assert!(a.due(t0 + 15 * MIN + 9 * SEC, true, true));
        assert_eq!(a.due_at(true, true), Some(t0 + 15 * MIN + 9 * SEC));
        a.saved();
        assert!(!a.due(t0 + 30 * MIN, true, true), "the clock restarts at the next change");
        a.edited(t0 + 31 * MIN);
        assert!(a.due(t0 + 46 * MIN, true, true));
    }

    #[test]
    fn never_without_a_file_changes_or_when_switched_off() {
        let t0 = Instant::now();
        let mut a = AutoSave::default();
        a.edited(t0);
        let later = t0 + 60 * MIN;
        assert!(!a.due(later, true, false), "an untitled document is kept by crash recovery");
        assert!(!a.due(later, false, true), "nothing unsaved");
        a.enabled = false;
        assert!(!a.due(later, true, true));
    }

    #[test]
    fn tick_runs_the_app_s_save_on_a_fake_clock() {
        let t0 = Instant::now();
        let mut a = AutoSave { every: 5 * MIN, idle: 10 * SEC, ..AutoSave::default() };
        let mut saves = 0;
        // Revisions stand for the app's edits.
        a.observe(7, t0);
        assert_eq!(a.tick(t0 + 10 * MIN, true, true, || panic!("no change yet")), Tick::Idle);
        a.observe(8, t0 + MIN);
        assert_eq!(a.tick(t0 + 2 * MIN, true, true, || panic!("not due")), Tick::Waiting(t0 + 6 * MIN));
        let saved = a.tick(t0 + 6 * MIN, true, true, || {
            saves += 1;
            Ok(())
        });
        assert_eq!((saved, saves), (Tick::Saved, 1));
        assert_eq!(a.tick(t0 + 20 * MIN, true, true, || panic!("saved; no change since")), Tick::Idle);
        // A failed save is tried again after the idle time, not at once.
        a.observe(9, t0 + 21 * MIN);
        assert_eq!(a.tick(t0 + 26 * MIN, true, true, || Err("disk full".into())), Tick::Failed("disk full".into()));
        assert_eq!(a.tick(t0 + 26 * MIN + SEC, true, true, || panic!("too soon")), Tick::Waiting(t0 + 26 * MIN + 10 * SEC));
        // Saved by hand in between: nothing to do, and the clock restarts.
        assert_eq!(a.tick(t0 + 27 * MIN, false, true, || panic!("not dirty")), Tick::Idle);
        assert_eq!(a.due_at(true, true), None);
    }
}
