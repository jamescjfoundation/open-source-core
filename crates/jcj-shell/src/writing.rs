// SPDX-License-Identifier: MIT OR Apache-2.0
//! Writing statistics, session targets and the sprint timer, the same in every writing app.
//!
//! The app reports its word count ([`Writing::observe`]); this module keeps the session's start,
//! the words written per day ([`History`], kept by the app wherever it keeps settings, as JSON),
//! a session target, a timed sprint, and the words per writing day a deadline needs
//! ([`words_per_day`]). [`progress_bar`] draws a small bar for the status bar.
//!
//! Words deleted count against the session and the day (net words), so the numbers never reward
//! padding; a big jump at once (a paste or an import) can be left out with
//! [`Writing::rebase`]. An app whose writers count what they wrote, whatever they later cut,
//! turns on [`Counting::UpOnly`] ([`Writing::counting`]).
//!
//! A writer's statistics kept between runs, with pages, writing time, goals and a window to see
//! them, are in [`stats`]; an app opts in by using them.

pub mod stats;

use std::collections::BTreeMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use egui::{Rect, Sense, Ui, pos2, vec2};
use jcj_theme::{Theme, regular};
use serde::{Deserialize, Serialize};

/// A civil date (proleptic Gregorian, UTC unless the app passes local days).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Day {
    /// The year.
    pub year: i32,
    /// 1 to 12.
    pub month: u8,
    /// 1 to 31.
    pub day: u8,
}

/// Days since 1970-01-01 to a civil date (H. Hinnant's public-domain algorithm).
pub fn civil_from_days(z: i64) -> Day {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u8;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
    Day { year: (if m <= 2 { y + 1 } else { y }) as i32, month: m, day: d }
}

/// A civil date to days since 1970-01-01.
pub fn days_from_civil(d: Day) -> i64 {
    let y = i64::from(d.year) - i64::from(d.month <= 2);
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(d.month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d.day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

impl Day {
    /// The day of a moment, shifted by `offset_secs` (the local zone's offset from UTC).
    pub fn of(at: SystemTime, offset_secs: i64) -> Day {
        let secs = match at.duration_since(UNIX_EPOCH) {
            Ok(d) => i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
            Err(e) => -i64::try_from(e.duration().as_secs()).unwrap_or(i64::MAX),
        };
        civil_from_days((secs + offset_secs).div_euclid(86_400))
    }

    /// `YYYY-MM-DD`.
    pub fn iso(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// Read `YYYY-MM-DD`.
    pub fn parse(s: &str) -> Option<Day> {
        let mut it = s.trim().splitn(3, '-');
        let year = it.next()?.parse().ok()?;
        let month: u8 = it.next()?.parse().ok()?;
        let day: u8 = it.next()?.parse().ok()?;
        let d = Day { year, month, day };
        ((1..=12).contains(&month) && (1..=31).contains(&day) && civil_from_days(days_from_civil(d)) == d).then_some(d)
    }

    /// 0 for Monday to 6 for Sunday.
    pub fn weekday(&self) -> u8 {
        (days_from_civil(*self) + 3).rem_euclid(7) as u8
    }

    /// The day `n` days later.
    pub fn plus(&self, n: i64) -> Day {
        civil_from_days(days_from_civil(*self) + n)
    }
}

/// Net words written per day.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct History {
    /// Words by `YYYY-MM-DD`.
    pub days: BTreeMap<String, i64>,
}

/// The most days a history keeps (older days are dropped).
pub const MAX_DAYS: usize = 3660;

impl History {
    /// Add `words` (negative for deleted words) to a day.
    pub fn add(&mut self, day: Day, words: i64) {
        if words == 0 {
            return;
        }
        *self.days.entry(day.iso()).or_insert(0) += words;
        while self.days.len() > MAX_DAYS {
            let Some(first) = self.days.keys().next().cloned() else { break };
            self.days.remove(&first);
        }
    }

    /// Words written on a day.
    pub fn on(&self, day: Day) -> i64 {
        self.days.get(&day.iso()).copied().unwrap_or(0)
    }

    /// Consecutive days up to `today` with more than zero words (today counts once it has some).
    pub fn streak(&self, today: Day) -> u32 {
        let mut n = 0;
        let mut d = if self.on(today) > 0 { today } else { today.plus(-1) };
        while self.on(d) > 0 && n < 100_000 {
            n += 1;
            d = d.plus(-1);
        }
        n
    }
}

/// A timed writing sprint.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sprint {
    /// When it started.
    pub started: Instant,
    /// How long it runs.
    pub length: Duration,
    /// The word count when it started.
    pub start_words: usize,
}

impl Sprint {
    /// Time left at `now` (zero once over).
    pub fn remaining(&self, now: Instant) -> Duration {
        self.length.saturating_sub(now.saturating_duration_since(self.started))
    }

    /// Whether the sprint is over.
    pub fn finished(&self, now: Instant) -> bool {
        self.remaining(now).is_zero()
    }
}

/// How a session and its days count words.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Counting {
    /// Net words: deleting takes words away (the default).
    #[default]
    Net,
    /// Only words added count; deleting takes nothing away.
    UpOnly,
}

/// One writing session: words since it started, its target, a sprint, and the day's history.
#[derive(Clone, Debug)]
pub struct Writing {
    /// The word count when the session started (or was last rebased).
    pub start_words: usize,
    /// The latest word count observed.
    pub words: usize,
    /// Net words written before the last rebase in this session.
    carried: i64,
    /// The session's target, if any.
    pub target: Option<usize>,
    /// The running sprint, if any.
    pub sprint: Option<Sprint>,
    /// Words per day.
    pub history: History,
    /// How words count.
    counting: Counting,
    /// Words added this session ([`Counting::UpOnly`]).
    written: i64,
    /// Words added when the running sprint started ([`Counting::UpOnly`]).
    sprint_written: i64,
    /// The page count when the running sprint started, when the app gave one.
    sprint_pages: Option<usize>,
}

impl Writing {
    /// A session starting at `words`.
    pub fn new(words: usize) -> Writing {
        Writing {
            start_words: words,
            words,
            carried: 0,
            target: None,
            sprint: None,
            history: History::default(),
            counting: Counting::Net,
            written: 0,
            sprint_written: 0,
            sprint_pages: None,
        }
    }

    /// The same session counting words as `counting` says ([`Counting::Net`] by default).
    pub fn counting(mut self, counting: Counting) -> Writing {
        self.counting = counting;
        self
    }

    /// How this session counts words.
    pub fn counts(&self) -> Counting {
        self.counting
    }

    /// The same session with a target.
    pub fn with_target(mut self, target: Option<usize>) -> Writing {
        self.target = target;
        self
    }

    /// Report the current word count on `day`; the change goes into the day's history (only a
    /// rise, with [`Counting::UpOnly`]).
    pub fn observe(&mut self, words: usize, day: Day) {
        let delta = words as i64 - self.words as i64;
        match self.counting {
            Counting::Net => self.history.add(day, delta),
            Counting::UpOnly if delta > 0 => {
                self.history.add(day, delta);
                self.written += delta;
            }
            Counting::UpOnly => {}
        }
        self.words = words;
    }

    /// Take the current count as the new base without counting the jump (an import, a paste the
    /// app does not want counted). The day's history is not changed by the jump.
    pub fn rebase(&mut self, words: usize) {
        self.carried += self.words as i64 - self.start_words as i64;
        self.start_words = words;
        self.words = words;
    }

    /// Words written this session: net words, or with [`Counting::UpOnly`] the words added.
    pub fn session_words(&self) -> i64 {
        match self.counting {
            Counting::Net => self.carried + self.words as i64 - self.start_words as i64,
            Counting::UpOnly => self.written,
        }
    }

    /// Progress toward the session target, 0 to 1 (`None` without a target).
    pub fn progress(&self) -> Option<f32> {
        let t = self.target.filter(|t| *t > 0)?;
        Some((self.session_words().max(0) as f32 / t as f32).clamp(0.0, 1.0))
    }

    /// Start a sprint of `length` now.
    pub fn start_sprint(&mut self, length: Duration, now: Instant) {
        self.sprint = Some(Sprint { started: now, length, start_words: self.words });
        self.sprint_written = self.written;
        self.sprint_pages = None;
    }

    /// Start a sprint of `length` now, with the document's page count, so its result is given in
    /// pages too ([`Writing::sprint_pages`]).
    pub fn start_sprint_with_pages(&mut self, length: Duration, now: Instant, pages: usize) {
        self.start_sprint(length, now);
        self.sprint_pages = Some(pages);
    }

    /// Words written in the running sprint (words added, with [`Counting::UpOnly`]).
    pub fn sprint_words(&self) -> i64 {
        match self.counting {
            Counting::Net => self.sprint.map_or(0, |s| self.words as i64 - s.start_words as i64),
            Counting::UpOnly => self.sprint.map_or(0, |_| self.written - self.sprint_written),
        }
    }

    /// Pages added in the running sprint, given the page count now: `None` with no sprint, or one
    /// started without a page count.
    pub fn sprint_pages(&self, pages: usize) -> Option<i64> {
        self.sprint.and(self.sprint_pages).map(|start| pages as i64 - start as i64)
    }

    /// Start the session again from the current count.
    pub fn reset(&mut self) {
        self.start_words = self.words;
        self.carried = 0;
        self.written = 0;
    }
}

/// Words per writing day needed to write `remaining` words by `deadline` (inclusive), counting
/// from `today` (inclusive) and only the weekdays in `writing_days` (0 Monday .. 6 Sunday; empty
/// means every day). `None` when no writing day is left.
pub fn words_per_day(remaining: usize, today: Day, deadline: Day, writing_days: &[u8]) -> Option<usize> {
    let span = days_from_civil(deadline) - days_from_civil(today);
    if span < 0 {
        return None;
    }
    let days = (0..=span.min(36_600)).filter(|n| writing_days.is_empty() || writing_days.contains(&today.plus(*n).weekday())).count();
    (days > 0).then(|| remaining.div_ceil(days))
}

/// A small progress bar with a label, for the status bar.
pub fn progress_bar(ui: &mut Ui, label: &str, fraction: f32, width: f32) -> egui::Response {
    let t = Theme::get(ui.ctx());
    let galley = ui.painter().layout_no_wrap(label.to_owned(), regular(t.type_sizes.caption), t.text_dim);
    let (rect, resp) = ui.allocate_exact_size(vec2(width + 6.0 + galley.size().x, 16.0), Sense::hover());
    let bar = Rect::from_min_size(pos2(rect.min.x, rect.center().y - 3.0), vec2(width, 6.0));
    ui.painter().rect_filled(bar, 3.0, t.border);
    let done = Rect::from_min_size(bar.min, vec2(width * fraction.clamp(0.0, 1.0), 6.0));
    ui.painter().rect_filled(done, 3.0, t.accent);
    ui.painter().galley(pos2(bar.max.x + 6.0, rect.center().y - galley.size().y / 2.0), galley, t.text_dim);
    let label = label.to_owned();
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::ProgressIndicator, true, &label));
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates_round_trip() {
        for n in [-800_000i64, -1, 0, 1, 19_000, 20_454, 2_932_896] {
            assert_eq!(days_from_civil(civil_from_days(n)), n);
        }
        assert_eq!(civil_from_days(0), Day { year: 1970, month: 1, day: 1 });
        assert_eq!(Day::parse("2026-11-24").map(|d| d.weekday()), Some(1), "a Tuesday");
        assert_eq!(Day::parse("2026-02-30"), None);
        assert_eq!(Day::parse("2026-02-28").unwrap().plus(1).iso(), "2026-03-01");
    }

    #[test]
    fn sessions_count_net_words_and_rebase_skips_jumps() {
        let day = Day { year: 2026, month: 11, day: 24 };
        let mut w = Writing::new(1000);
        w.target = Some(500);
        w.observe(1250, day);
        w.observe(1200, day);
        assert_eq!(w.session_words(), 200);
        assert_eq!(w.progress(), Some(0.4));
        w.rebase(5200);
        assert_eq!(w.session_words(), 200);
        w.observe(5300, day);
        assert_eq!(w.session_words(), 300);
        assert_eq!(w.history.on(day), 300, "the imported jump is not in the day's words");
        w.reset();
        assert_eq!(w.session_words(), 0);
    }

    #[test]
    fn sprints_time_out_and_count_their_words() {
        let now = Instant::now();
        let mut w = Writing::new(10);
        w.start_sprint(Duration::from_secs(600), now);
        w.observe(70, Day { year: 2026, month: 1, day: 1 });
        assert_eq!(w.sprint_words(), 60);
        let s = w.sprint.unwrap();
        assert!(!s.finished(now + Duration::from_secs(599)));
        assert!(s.finished(now + Duration::from_secs(600)));
    }

    #[test]
    fn counting_up_only_keeps_what_was_written() {
        let day = Day { year: 2026, month: 11, day: 24 };
        let mut w = Writing::new(1000).counting(Counting::UpOnly).with_target(Some(500));
        assert_eq!(w.counts(), Counting::UpOnly);
        assert_eq!(Writing::new(0).counts(), Counting::Net, "net words unless the app asks");
        w.observe(1250, day);
        w.observe(1200, day);
        assert_eq!(w.session_words(), 250, "the 50 cut take nothing away");
        assert_eq!(w.history.on(day), 250);
        assert_eq!(w.progress(), Some(0.5));
        w.rebase(5200);
        w.observe(5300, day);
        assert_eq!((w.session_words(), w.history.on(day)), (350, 350), "the rebased jump is left out");
        let now = Instant::now();
        w.start_sprint_with_pages(Duration::from_secs(600), now, 20);
        w.observe(5280, day);
        w.observe(5330, day);
        assert_eq!(w.sprint_words(), 50);
        assert_eq!(w.sprint_pages(22), Some(2));
        w.start_sprint(Duration::from_secs(600), now);
        assert_eq!(w.sprint_pages(22), None, "a sprint started without pages has no page result");
        w.reset();
        assert_eq!(w.session_words(), 0);
    }

    #[test]
    fn streaks_and_deadlines() {
        let mut h = History::default();
        let today = Day { year: 2026, month: 11, day: 24 };
        for n in 1..=3 {
            h.add(today.plus(-n), 100);
        }
        assert_eq!(h.streak(today), 3);
        h.add(today, 5);
        assert_eq!(h.streak(today), 4);
        // Tuesday to the next Monday, weekdays only: 5 writing days.
        let deadline = today.plus(6);
        assert_eq!(words_per_day(10_000, today, deadline, &[0, 1, 2, 3, 4]), Some(2000));
        assert_eq!(words_per_day(10_000, today, deadline, &[]), Some(1429));
        assert_eq!(words_per_day(10, today, today.plus(-1), &[]), None);
        assert_eq!(words_per_day(10, today, today, &[6]), None);
    }
}
