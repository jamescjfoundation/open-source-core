// SPDX-License-Identifier: MIT OR Apache-2.0
//! A writer's statistics kept between runs: words, pages and writing time per day, counted only
//! upward, goals, and a window that shows them.
//!
//! Nothing here runs unless an app uses it; [`Writing`](super::Writing) and its net words per
//! day are unchanged. An app that keeps statistics:
//!
//! - opens them from its settings folder with [`Statistics::open`] and writes them back with
//!   [`Statistics::save`] (the file [`FILE`], JSON);
//! - keeps a [`Tracker`] beside them, calls [`Tracker::count`] with its word count (and page
//!   count, if it has pages) after each edit, and [`Tracker::tick`] once a frame, so the time
//!   spent writing is counted while input keeps coming;
//! - shows [`statistics_window`] when the writer asks for it, with a [`StatsView`] that says
//!   which unit the app's goal is in ([`Unit`]).
//!
//! **Counting only goes up.** Words and pages written count; deleting takes nothing away, so a
//! day's numbers say what was written that day. A jump the app does not want counted (an
//! import, a paste) is left out with [`Tracker::forget`] before it is counted.
//!
//! **Writing time** counts while input came within the idle time the writer sets
//! ([`Statistics::idle_minutes`]), and stops after it. It is kept by day and by hour of the day.
//!
//! **The days are the app's.** The day and hour a count belongs to are given by the app
//! ([`Moment`]): local time, or UTC; the file keeps no time zone, and the window names the one
//! the app says ([`StatsView::zone`]).
//!
//! **The file.** `{"enabled", "idle_minutes", "sprint_minutes", "goals": {"pages_per_day",
//! "days_per_week", "minutes_per_day", "words_per_day"}, "days": {"YYYY-MM-DD": {"words",
//! "pages", "seconds", "hours": [24 numbers]}}}`. A key that is missing takes its default and one
//! that is not known is ignored, so a file an app wrote in this shape before it used the core
//! reads as it is. Everything read is bounded ([`Statistics::repaired`]).

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use egui::{Rect, RichText, Sense, Stroke, Ui, pos2, vec2};
use jcj_i18n::{tr, tr_with, trn};
use jcj_theme::{Theme, regular};
use serde::{Deserialize, Serialize};

use super::{Day, days_from_civil};

/// The statistics file's name in the settings folder.
pub const FILE: &str = "stats.json";
/// Largest statistics file read, bytes.
pub const MAX_FILE_BYTES: u64 = 8 << 20;
/// Most days kept (about ten years); older days are dropped.
pub const MAX_STAT_DAYS: usize = 3_700;
/// The idle minutes after which writing time stops counting, by default.
pub const IDLE_MINUTES: u32 = 3;
/// Most idle minutes that can be set.
pub const MAX_IDLE_MINUTES: u32 = 120;
/// A sprint's length, minutes, by default.
pub const SPRINT_MINUTES: u32 = 25;
/// Most minutes a sprint can be set to.
pub const MAX_SPRINT_MINUTES: u32 = 240;
/// Most pages a day a goal can ask for.
pub const MAX_PAGES_GOAL: f32 = 100.0;
/// Most words a day a goal can ask for.
pub const MAX_WORDS_GOAL: u32 = 100_000;
/// Most time a single tick adds: a frame after the app was asleep counts no more.
pub const MAX_TICK: Duration = Duration::from_secs(5);

/// One day's writing.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DayStats {
    /// Words written (deleting takes none away).
    pub words: u64,
    /// Whole pages added.
    pub pages: u64,
    /// Seconds spent writing.
    pub seconds: u64,
    /// Seconds spent writing, by hour of the day (24 numbers, the first from midnight).
    pub hours: Vec<u64>,
}

impl DayStats {
    /// Whether the day counts toward a streak: words written, or a minute of writing.
    pub fn wrote(&self) -> bool {
        self.words > 0 || self.seconds >= 60
    }

    /// The day's amount of `metric`: words, pages, or minutes.
    pub fn value(&self, metric: Metric) -> f32 {
        match metric {
            Metric::Words => self.words as f32,
            Metric::Pages => self.pages as f32,
            Metric::Minutes => self.seconds as f32 / 60.0,
        }
    }
}

/// Goals: an amount in the app's unit on so many days a week, and minutes a day. Zero is no goal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Goals {
    /// Pages a day, for an app that counts pages (0 to [`MAX_PAGES_GOAL`]).
    pub pages_per_day: f32,
    /// The days a week the page or word goal is for (1 to 7).
    pub days_per_week: u32,
    /// Minutes of writing a day (0 to a whole day).
    pub minutes_per_day: u32,
    /// Words a day, for an app that counts words (0 to [`MAX_WORDS_GOAL`]).
    pub words_per_day: u32,
}

impl Default for Goals {
    fn default() -> Self {
        Goals { pages_per_day: 0.0, days_per_week: 7, minutes_per_day: 0, words_per_day: 0 }
    }
}

impl Goals {
    /// The goal for `metric` on an average day: the page or word goal spread over the week.
    pub fn per_day(&self, metric: Metric) -> f32 {
        let week = self.days_per_week.clamp(1, 7) as f32 / 7.0;
        match metric {
            Metric::Words => self.words_per_day as f32 * week,
            Metric::Pages => self.pages_per_day * week,
            Metric::Minutes => self.minutes_per_day as f32,
        }
    }
}

/// A writer's statistics: the settings and the days. Kept as JSON in the settings folder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Statistics {
    /// Whether anything is counted.
    pub enabled: bool,
    /// Writing time stops counting after this many minutes without input (1 to
    /// [`MAX_IDLE_MINUTES`]).
    pub idle_minutes: u32,
    /// The length of a sprint the writer starts, minutes (1 to [`MAX_SPRINT_MINUTES`]).
    pub sprint_minutes: u32,
    /// The writer's goals.
    pub goals: Goals,
    /// The days, by `YYYY-MM-DD`.
    pub days: BTreeMap<String, DayStats>,
}

impl Default for Statistics {
    fn default() -> Self {
        Statistics { enabled: true, idle_minutes: IDLE_MINUTES, sprint_minutes: SPRINT_MINUTES, goals: Goals::default(), days: BTreeMap::new() }
    }
}

impl Statistics {
    /// The same statistics with every value within its range: settings clamped, days that are
    /// not dates dropped, at most [`MAX_STAT_DAYS`] days (the newest), 24 hours a day.
    pub fn repaired(mut self) -> Self {
        self.idle_minutes = self.idle_minutes.clamp(1, MAX_IDLE_MINUTES);
        self.sprint_minutes = self.sprint_minutes.clamp(1, MAX_SPRINT_MINUTES);
        let g = &mut self.goals;
        g.pages_per_day = if g.pages_per_day.is_finite() { g.pages_per_day.clamp(0.0, MAX_PAGES_GOAL) } else { 0.0 };
        g.days_per_week = g.days_per_week.clamp(1, 7);
        g.minutes_per_day = g.minutes_per_day.min(24 * 60);
        g.words_per_day = g.words_per_day.min(MAX_WORDS_GOAL);
        self.days.retain(|k, _| Day::parse(k).is_some_and(|d| d.iso() == *k));
        self.trim();
        for d in self.days.values_mut() {
            d.hours.resize(24, 0);
        }
        self
    }

    fn trim(&mut self) {
        while self.days.len() > MAX_STAT_DAYS {
            let Some(first) = self.days.keys().next().cloned() else { break };
            self.days.remove(&first);
        }
    }

    /// Read statistics from JSON, bounded ([`Statistics::repaired`]). `None` when the text is
    /// longer than [`MAX_FILE_BYTES`] or is not statistics.
    pub fn from_json(text: &str) -> Option<Statistics> {
        if text.len() as u64 > MAX_FILE_BYTES {
            return None;
        }
        serde_json::from_str::<Statistics>(text).ok().map(Statistics::repaired)
    }

    /// The statistics as JSON, as [`Statistics::save`] writes them.
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Read the statistics in the settings folder `dir` ([`FILE`]). Never fails: with no file,
    /// `fresh` is used (the app's own starting goals); a file that cannot be read is moved aside
    /// (`stats.json.damaged`) and `fresh` is used too, with a message to show.
    pub fn open(dir: &Path, fresh: Statistics) -> (Statistics, Option<String>) {
        let path = dir.join(FILE);
        let read = || -> Result<Option<Statistics>, String> {
            let meta = match std::fs::metadata(&path) {
                Ok(m) => m,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(e) => return Err(e.to_string()),
            };
            if meta.len() > MAX_FILE_BYTES {
                return Err("the file is too large".into());
            }
            let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            serde_json::from_str::<Statistics>(&text).map(|s| Some(s.repaired())).map_err(|e| e.to_string())
        };
        match read() {
            Ok(found) => (found.unwrap_or_else(|| fresh.repaired()), None),
            Err(why) => {
                let _ = std::fs::rename(&path, dir.join(format!("{FILE}.damaged")));
                (fresh.repaired(), Some(tr_with("Writing statistics could not be read ({why}); they start again.", &[("why", &why)])))
            }
        }
    }

    /// Write the statistics into the settings folder `dir` ([`FILE`]), atomically.
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = dir.join(FILE);
        jcj_upstream::atomic::atomic_write(&path, self.to_json().as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// A day's writing (an empty day when nothing was written).
    pub fn on(&self, day: Day) -> DayStats {
        self.days.get(&day.iso()).cloned().unwrap_or_default()
    }

    fn day_mut(&mut self, day: Day) -> &mut DayStats {
        let key = day.iso();
        if !self.days.contains_key(&key) {
            self.days.insert(key.clone(), DayStats { hours: vec![0; 24], ..DayStats::default() });
            self.trim();
        }
        let d = self.days.entry(key).or_default();
        d.hours.resize(24, 0);
        d
    }

    /// The days that were written on, by day number (days since 1970-01-01).
    fn by_number(&self) -> BTreeMap<i64, &DayStats> {
        self.days.iter().filter_map(|(k, d)| Day::parse(k).map(|day| (days_from_civil(day), d))).collect()
    }

    /// Streaks, the best weekday for `metric` and the best hour, as of `today`.
    pub fn summary(&self, today: Day, metric: Metric) -> Summary {
        let days = self.by_number();
        let wrote = |n: i64| days.get(&n).is_some_and(|d| d.wrote());
        let today_n = days_from_civil(today);
        // Today counts once written on; until then the streak runs to yesterday.
        let mut n = if wrote(today_n) { today_n } else { today_n - 1 };
        let mut current = 0;
        while wrote(n) && current < 100_000 {
            current += 1;
            n -= 1;
        }
        let (mut longest, mut run, mut last) = (0, 0, None);
        let mut weekdays = [0f32; 7];
        let mut hours = [0u64; 24];
        for (&k, d) in &days {
            if d.wrote() {
                run = if last == Some(k - 1) { run + 1 } else { 1 };
                longest = longest.max(run);
                last = Some(k);
            }
            // 1970-01-01 was a Thursday; 0 is Monday.
            weekdays[(k + 3).rem_euclid(7) as usize] += d.value(metric);
            for (h, s) in d.hours.iter().enumerate().take(24) {
                hours[h] = hours[h].saturating_add(*s);
            }
        }
        let best_weekday = weekdays.iter().enumerate().filter(|(_, v)| **v > 0.0).max_by(|a, b| a.1.total_cmp(b.1)).map(|(i, _)| i as u8);
        let best_hour = hours.iter().enumerate().filter(|(_, s)| **s > 0).max_by_key(|(_, s)| **s).map(|(h, _)| h as u8);
        Summary { current_streak: current, longest_streak: longest, best_weekday, best_hour }
    }

    /// The bars of the window: `metric` summed over each span of `range` (oldest first), the
    /// last ending on `today`.
    pub fn bars(&self, today: Day, metric: Metric, range: Range) -> Vec<f32> {
        let days = self.by_number();
        let (span, count) = range.span_and_bars();
        let today_n = days_from_civil(today);
        (0..count)
            .rev()
            .map(|b| {
                let end = today_n - b * span;
                (end - span + 1..=end).map(|k| days.get(&k).map_or(0.0, |d| d.value(metric))).sum()
            })
            .collect()
    }

    /// The goal for one bar of `range` in `metric` (0: no goal).
    pub fn goal(&self, metric: Metric, range: Range) -> f32 {
        self.goals.per_day(metric) * range.span_and_bars().0 as f32
    }
}

/// What the statistics window sums up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// Days in a row written on, up to today (today counts once written on).
    pub current_streak: u32,
    /// The most days in a row ever written on.
    pub longest_streak: u32,
    /// The weekday with the most of the metric (0 Monday to 6 Sunday); `None` before any.
    pub best_weekday: Option<u8>,
    /// The hour of the day with the most writing time (0 to 23); `None` before any.
    pub best_hour: Option<u8>,
}

/// What is counted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Metric {
    /// Words written.
    #[default]
    Words,
    /// Pages added.
    Pages,
    /// Minutes of writing.
    Minutes,
}

/// The unit an app's writing is measured in: the one its goal is set in and its window shows
/// first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Unit {
    /// Words: the window offers words and minutes, and a goal in words.
    #[default]
    Words,
    /// Pages: the window offers words, pages and minutes, and a goal in pages.
    Pages,
}

impl Unit {
    /// The metric of the unit.
    pub fn metric(self) -> Metric {
        match self {
            Unit::Words => Metric::Words,
            Unit::Pages => Metric::Pages,
        }
    }
}

/// How much one bar of the window holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Range {
    /// A day a bar, the last 14.
    Days,
    /// Seven days a bar, the last 12.
    #[default]
    Weeks,
    /// Thirty days a bar, the last 12.
    Months,
    /// 365 days a bar, the last 5.
    Years,
}

impl Range {
    /// Days a bar, and how many bars.
    pub fn span_and_bars(self) -> (i64, i64) {
        match self {
            Range::Days => (1, 14),
            Range::Weeks => (7, 12),
            Range::Months => (30, 12),
            Range::Years => (365, 5),
        }
    }
}

/// The day and the hour of the day a count belongs to, as the app's clock gives them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Moment {
    /// The day.
    pub day: Day,
    /// The hour of the day, 0 to 23.
    pub hour: u8,
}

impl Moment {
    /// The day and hour of `at`, shifted by `offset_secs` (the zone's offset from UTC; 0 for UTC).
    pub fn of(at: SystemTime, offset_secs: i64) -> Moment {
        let secs = match at.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(d) => i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
            Err(e) => -i64::try_from(e.duration().as_secs()).unwrap_or(i64::MAX),
        };
        let local = secs.saturating_add(offset_secs);
        Moment { day: Day::of(at, offset_secs), hour: (local.rem_euclid(86_400) / 3_600) as u8 }
    }
}

/// Counts writing into [`Statistics`]: words and pages added, and time spent while input keeps
/// coming. One a window; it keeps only what it last counted.
#[derive(Clone, Debug, Default)]
pub struct Tracker {
    /// Words and pages at the last count.
    last: Option<(usize, Option<usize>)>,
    last_input: Option<Instant>,
    last_tick: Option<Instant>,
    /// Writing time not yet a whole second, milliseconds.
    carry_ms: u64,
}

impl Tracker {
    /// A tracker that has counted nothing yet: its first count is the base.
    pub fn new() -> Tracker {
        Tracker::default()
    }

    /// The document changed: `words` and `pages` (`None` for an app without pages) are its
    /// counts now. What grew since the last count is added to the day of `at`; what shrank
    /// takes nothing away. The change counts as input for the writing time. The first count, and
    /// the first after [`Tracker::forget`], is only the base. Returns whether `stats` changed.
    pub fn count(&mut self, stats: &mut Statistics, words: usize, pages: Option<usize>, at: Moment, now: Instant) -> bool {
        self.last_input = Some(now);
        let before = self.last.replace((words, pages));
        let Some((w0, p0)) = before.filter(|_| stats.enabled) else { return false };
        let added_words = words.saturating_sub(w0) as u64;
        let added_pages = match (p0, pages) {
            (Some(p0), Some(p)) => p.saturating_sub(p0) as u64,
            _ => 0,
        };
        if added_words == 0 && added_pages == 0 {
            return false;
        }
        let day = stats.day_mut(at.day);
        day.words = day.words.saturating_add(added_words);
        day.pages = day.pages.saturating_add(added_pages);
        true
    }

    /// Input that changed no count (the caret moved, a format was set): writing time goes on.
    pub fn input(&mut self, now: Instant) {
        self.last_input = Some(now);
    }

    /// Forget the last counts, so the next [`Tracker::count`] is the base: another document came
    /// into view, or the app is about to make a change it does not want counted.
    pub fn forget(&mut self) {
        self.last = None;
    }

    /// Once a frame: the time since the last tick (at most [`MAX_TICK`]) is writing time when
    /// input came within the idle time, and is added to the day and hour of `at` in whole
    /// seconds. Returns whether `stats` changed.
    pub fn tick(&mut self, stats: &mut Statistics, at: Moment, now: Instant) -> bool {
        let since = self.last_tick.map_or(Duration::ZERO, |t| now.saturating_duration_since(t)).min(MAX_TICK);
        self.last_tick = Some(now);
        let idle = Duration::from_secs(u64::from(stats.idle_minutes.clamp(1, MAX_IDLE_MINUTES)) * 60);
        let active = self.last_input.is_some_and(|t| now.saturating_duration_since(t) < idle);
        if !stats.enabled || !active || since.is_zero() {
            return false;
        }
        self.carry_ms += since.as_millis() as u64;
        let whole = self.carry_ms / 1_000;
        if whole == 0 {
            return false;
        }
        self.carry_ms %= 1_000;
        let day = stats.day_mut(at.day);
        day.seconds = day.seconds.saturating_add(whole);
        if let Some(h) = day.hours.get_mut(usize::from(at.hour)) {
            *h = h.saturating_add(whole);
        }
        true
    }

    /// Whether input came within the idle time at `now`: writing time is being counted.
    pub fn is_writing(&self, stats: &Statistics, now: Instant) -> bool {
        let idle = Duration::from_secs(u64::from(stats.idle_minutes.clamp(1, MAX_IDLE_MINUTES)) * 60);
        stats.enabled && self.last_input.is_some_and(|t| now.saturating_duration_since(t) < idle)
    }
}

/// What the statistics window shows, and whether it is open.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StatsView {
    /// Whether the window is open.
    pub open: bool,
    /// The app's unit: the metrics and the goal offered.
    pub unit: Unit,
    /// The metric shown.
    pub metric: Metric,
    /// How much a bar holds.
    pub range: Range,
    /// Said after the best hours (`"UTC"` when the app counts its hours in UTC); empty for none.
    pub zone: String,
}

impl StatsView {
    /// A closed window for an app measured in `unit`, showing that unit by weeks.
    pub fn new(unit: Unit) -> StatsView {
        StatsView { open: false, unit, metric: unit.metric(), range: Range::Weeks, zone: String::new() }
    }

    /// The metrics the window offers.
    pub fn metrics(&self) -> &'static [Metric] {
        match self.unit {
            Unit::Words => &[Metric::Words, Metric::Minutes],
            Unit::Pages => &[Metric::Words, Metric::Pages, Metric::Minutes],
        }
    }
}

/// The weekday's name, 0 Monday to 6 Sunday.
fn weekday_name(n: u8) -> String {
    match n {
        0 => tr("Monday"),
        1 => tr("Tuesday"),
        2 => tr("Wednesday"),
        3 => tr("Thursday"),
        4 => tr("Friday"),
        5 => tr("Saturday"),
        _ => tr("Sunday"),
    }
}

/// The statistics window, while `view.open`: the streaks, the best weekday and hours, the bars
/// of the metric with the goal as a line, and the goals and settings to change. Returns whether
/// the writer changed `stats` (the app saves them).
pub fn statistics_window(ctx: &egui::Context, stats: &mut Statistics, view: &mut StatsView, today: Day) -> bool {
    if !view.open {
        return false;
    }
    let mut open = true;
    let mut changed = false;
    egui::Window::new(tr("Writing Statistics")).id(egui::Id::unique("jcj-writing-statistics")).open(&mut open).default_width(520.0).show(ctx, |ui| {
        changed = statistics_ui(ui, stats, view, today);
    });
    view.open = open;
    changed
}

/// The contents of the statistics window, drawn into `ui` (an app that shows them elsewhere: a
/// panel, a File page). Returns whether the writer changed `stats`.
pub fn statistics_ui(ui: &mut Ui, stats: &mut Statistics, view: &mut StatsView, today: Day) -> bool {
    let t = Theme::get(ui.ctx());
    if !view.metrics().contains(&view.metric) {
        view.metric = view.unit.metric();
    }
    let summary = stats.summary(today, view.metric);
    let best_day = summary.best_weekday.map_or_else(|| "—".to_owned(), weekday_name);
    let best_hours = summary.best_hour.map_or_else(
        || "—".to_owned(),
        |h| {
            let hours = tr_with("{from}:00 to {to}:00", &[("from", &format!("{h:02}")), ("to", &format!("{:02}", (h + 1) % 24))]);
            if view.zone.is_empty() { hours } else { format!("{hours} {}", view.zone) }
        },
    );
    ui.horizontal_wrapped(|ui| {
        for (label, value) in [
            (tr("Current streak"), trn("{n} day", "{n} days", u64::from(summary.current_streak), &[])),
            (tr("Longest streak"), trn("{n} day", "{n} days", u64::from(summary.longest_streak), &[])),
            (tr("Best day"), best_day),
            (tr("Best hours"), best_hours),
        ] {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label(RichText::new(label).font(regular(t.type_sizes.caption)).color(t.text_dim));
                    ui.strong(value);
                });
            });
        }
    });
    ui.horizontal_wrapped(|ui| {
        for &m in view.metrics() {
            let label = match m {
                Metric::Words => tr("Words"),
                Metric::Pages => tr("Pages"),
                Metric::Minutes => tr("Minutes"),
            };
            if ui.selectable_label(view.metric == m, label).clicked() {
                view.metric = m;
            }
        }
        ui.separator();
        for (r, label) in [(Range::Days, tr("Days")), (Range::Weeks, tr("Weeks")), (Range::Months, tr("Months")), (Range::Years, tr("Years"))] {
            if ui.selectable_label(view.range == r, label).clicked() {
                view.range = r;
            }
        }
    });
    let bars = stats.bars(today, view.metric, view.range);
    let goal = stats.goal(view.metric, view.range);
    let span = view.range.span_and_bars().0;
    let average = bars.last().copied().unwrap_or(0.0) / span as f32;
    ui.label(trn(
        "Average a day, the last day: {average}",
        "Average a day, the last {n} days: {average}",
        span as u64,
        &[("average", &format!("{average:.1}"))],
    ));
    bar_chart(ui, &t, &bars, goal);
    ui.separator();
    let mut changed = false;
    let g = &mut stats.goals;
    ui.horizontal_wrapped(|ui| {
        ui.label(tr("Goals:"));
        changed |= match view.unit {
            Unit::Pages => ui.add(egui::DragValue::new(&mut g.pages_per_day).range(0.0..=MAX_PAGES_GOAL).speed(0.25).suffix(tr(" pages"))).changed(),
            Unit::Words => ui.add(egui::DragValue::new(&mut g.words_per_day).range(0..=MAX_WORDS_GOAL).speed(10.0).suffix(tr(" words"))).changed(),
        };
        ui.label(tr("on"));
        changed |= ui.add(egui::DragValue::new(&mut g.days_per_week).range(1..=7).suffix(tr(" days a week"))).changed();
        changed |= ui.add(egui::DragValue::new(&mut g.minutes_per_day).range(0..=24 * 60).suffix(tr(" minutes a day"))).changed();
    });
    ui.horizontal_wrapped(|ui| {
        changed |= ui.checkbox(&mut stats.enabled, tr("Keep statistics")).changed();
        ui.label(tr("Stop counting time after"));
        changed |= ui.add(egui::DragValue::new(&mut stats.idle_minutes).range(1..=MAX_IDLE_MINUTES).suffix(tr(" idle minutes"))).changed();
    });
    changed
}

/// The bars, scaled to the highest bar or the goal, with the goal as a line across them.
fn bar_chart(ui: &mut Ui, t: &Theme, bars: &[f32], goal: f32) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width().min(500.0), 160.0), Sense::hover());
    let painter = ui.painter_at(rect);
    let top = bars.iter().copied().fold(goal.max(1.0), f32::max);
    let w = rect.width() / bars.len().max(1) as f32;
    for (i, v) in bars.iter().enumerate() {
        let h = rect.height() * (v / top).clamp(0.0, 1.0);
        let r = Rect::from_min_max(pos2(rect.min.x + i as f32 * w + 2.0, rect.max.y - h), pos2(rect.min.x + (i + 1) as f32 * w - 2.0, rect.max.y));
        painter.rect_filled(r, 2.0, t.accent);
    }
    painter.line_segment([pos2(rect.min.x, rect.max.y), pos2(rect.max.x, rect.max.y)], Stroke::new(1.0, t.border));
    if goal > 0.0 {
        let y = rect.max.y - rect.height() * (goal / top);
        painter.line_segment([pos2(rect.min.x, y), pos2(rect.max.x, y)], Stroke::new(1.5, t.text));
    }
    let label = tr("Writing by period");
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, &label));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> Day {
        Day::parse(s).unwrap()
    }

    fn at(s: &str, hour: u8) -> Moment {
        Moment { day: day(s), hour }
    }

    /// A statistics file in the shape an app kept before it used the core: written for this
    /// test, with the field names and meanings the core reads.
    const EARLIER_FILE: &str = r#"{
        "enabled": true,
        "idle_minutes": 5,
        "sprint_minutes": 30,
        "goals": {"pages_per_day": 1.5, "days_per_week": 5, "minutes_per_day": 45},
        "days": {
            "2026-11-20": {"words": 812, "pages": 3, "seconds": 3600, "hours": [0,0,0,0,0,0,0,0,0,1800,1800,0,0,0,0,0,0,0,0,0,0,0,0,0]},
            "2026-11-21": {"words": 40, "pages": 0, "seconds": 59, "hours": [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,59]}
        }
    }"#;

    #[test]
    fn an_earlier_file_reads_whole_and_writes_back_the_same() {
        let s = Statistics::from_json(EARLIER_FILE).unwrap();
        assert_eq!((s.enabled, s.idle_minutes, s.sprint_minutes), (true, 5, 30));
        assert_eq!(s.goals, Goals { pages_per_day: 1.5, days_per_week: 5, minutes_per_day: 45, words_per_day: 0 });
        let d = s.on(day("2026-11-20"));
        assert_eq!((d.words, d.pages, d.seconds, d.hours[9], d.hours[10]), (812, 3, 3600, 1800, 1800));
        assert_eq!(s.on(day("2026-11-21")).hours[23], 59);
        // Written back and read again, nothing is lost; the earlier fields are all there.
        let again = Statistics::from_json(&s.to_json()).unwrap();
        assert_eq!(again, s);
        let earlier: serde_json::Value = serde_json::from_str(EARLIER_FILE).unwrap();
        let now: serde_json::Value = serde_json::from_str(&s.to_json()).unwrap();
        assert_eq!(now["days"], earlier["days"]);
        for key in ["pages_per_day", "days_per_week", "minutes_per_day"] {
            assert_eq!(now["goals"][key], earlier["goals"][key], "{key}");
        }
    }

    #[test]
    fn hostile_statistics_are_bounded() {
        let mut s = Statistics { idle_minutes: 0, sprint_minutes: 10_000, ..Statistics::default() };
        s.goals = Goals { pages_per_day: f32::NAN, days_per_week: 0, minutes_per_day: 100_000, words_per_day: u32::MAX };
        for i in 0..4_000 {
            s.days.insert(Day { year: 1990, month: 1, day: 1 }.plus(i).iso(), DayStats { hours: vec![1; 99], ..DayStats::default() });
        }
        s.days.insert("not a day".into(), DayStats::default());
        s.days.insert("2026-02-30".into(), DayStats::default());
        let s = s.repaired();
        assert_eq!((s.idle_minutes, s.sprint_minutes), (1, MAX_SPRINT_MINUTES));
        assert_eq!(s.goals, Goals { pages_per_day: 0.0, days_per_week: 1, minutes_per_day: 24 * 60, words_per_day: MAX_WORDS_GOAL });
        assert_eq!(s.days.len(), MAX_STAT_DAYS);
        assert!(s.days.values().all(|d| d.hours.len() == 24));
        assert!(s.days.keys().next().is_some_and(|k| k.as_str() > "1990-01-01"), "the oldest days go");
        assert_eq!(Statistics::from_json("[1, 2]"), None);
        assert_eq!(Statistics::from_json(&" ".repeat(MAX_FILE_BYTES as usize + 1)), None);
        assert_eq!(Statistics::from_json("{}"), Some(Statistics::default()));
    }

    #[test]
    fn counting_only_goes_up_and_a_forgotten_jump_is_left_out() {
        let mut s = Statistics::default();
        let mut tracker = Tracker::new();
        let now = Instant::now();
        let a = at("2026-11-24", 9);
        assert!(!tracker.count(&mut s, 100, Some(1), a, now), "the first count is the base");
        assert!(tracker.count(&mut s, 150, Some(2), a, now));
        assert!(!tracker.count(&mut s, 120, Some(1), a, now), "deleting takes nothing away");
        assert!(tracker.count(&mut s, 130, Some(1), a, now));
        assert_eq!((s.on(a.day).words, s.on(a.day).pages), (60, 1));
        tracker.forget();
        assert!(!tracker.count(&mut s, 5_130, Some(40), a, now), "an import after forget is the new base");
        assert_eq!(s.on(a.day).words, 60);
        // An app without pages counts words alone.
        let mut words_only = Tracker::new();
        words_only.count(&mut s, 0, None, a, now);
        words_only.count(&mut s, 5, None, a, now);
        assert_eq!((s.on(a.day).words, s.on(a.day).pages), (65, 1));
        // Turned off, nothing counts.
        s.enabled = false;
        assert!(!tracker.count(&mut s, 9_999, Some(99), a, now));
        assert_eq!(s.on(a.day).words, 65);
    }

    #[test]
    fn writing_time_counts_while_input_comes_and_stops_when_idle() {
        let mut s = Statistics { idle_minutes: 1, ..Statistics::default() };
        let mut tracker = Tracker::new();
        let t0 = Instant::now();
        let a = at("2026-11-24", 14);
        assert!(!tracker.tick(&mut s, a, t0), "no input yet");
        tracker.count(&mut s, 0, None, a, t0);
        assert!(tracker.is_writing(&s, t0));
        // Ten seconds of frames, half a second apart.
        for i in 1..=20 {
            tracker.tick(&mut s, a, t0 + Duration::from_millis(500 * i));
        }
        assert_eq!((s.on(a.day).seconds, s.on(a.day).hours[14]), (10, 10));
        // A frame after a long sleep adds no more than MAX_TICK.
        tracker.input(t0 + Duration::from_secs(20));
        tracker.tick(&mut s, a, t0 + Duration::from_secs(50));
        assert_eq!(s.on(a.day).seconds, 10 + MAX_TICK.as_secs());
        // A minute and more after the last input, time stops.
        assert!(!tracker.tick(&mut s, a, t0 + Duration::from_secs(81)));
        assert!(!tracker.is_writing(&s, t0 + Duration::from_secs(81)));
    }

    #[test]
    fn the_summary_and_the_bars() {
        let s = Statistics::from_json(EARLIER_FILE).unwrap();
        let today = day("2026-11-22");
        let sum = s.summary(today, Metric::Words);
        // The 21st has words (but less than a minute); the 22nd has nothing yet.
        assert_eq!((sum.current_streak, sum.longest_streak), (2, 2));
        assert_eq!(sum.best_weekday, Some(4), "2026-11-20 is a Friday");
        assert_eq!(sum.best_hour, Some(9));
        assert_eq!(s.summary(day("2026-11-24"), Metric::Words).current_streak, 0);
        let bars = s.bars(today, Metric::Words, Range::Days);
        assert_eq!(bars.len(), 14);
        assert_eq!(&bars[11..], [812.0, 40.0, 0.0]);
        assert_eq!(s.bars(today, Metric::Pages, Range::Weeks).last(), Some(&3.0));
        assert_eq!(s.bars(today, Metric::Minutes, Range::Weeks).last().map(|m| m.round()), Some(61.0));
        // 1.5 pages on 5 days a week: 7.5 pages a week.
        assert_eq!(s.goal(Metric::Pages, Range::Weeks), 7.5);
        assert_eq!(s.goal(Metric::Minutes, Range::Days), 45.0);
        assert_eq!(s.goal(Metric::Words, Range::Days), 0.0, "no word goal");
        assert_eq!(Statistics::default().summary(today, Metric::Words), Summary::default());
    }

    #[test]
    fn moments_and_the_window_s_metrics() {
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(86_400 + 3 * 3_600 + 59);
        assert_eq!(Moment::of(at, 0), Moment { day: day("1970-01-02"), hour: 3 });
        assert_eq!(Moment::of(at, -4 * 3_600), Moment { day: day("1970-01-01"), hour: 23 });
        assert_eq!(StatsView::new(Unit::Words).metrics(), [Metric::Words, Metric::Minutes]);
        let pages = StatsView::new(Unit::Pages);
        assert_eq!((pages.metric, pages.range, pages.open), (Metric::Pages, Range::Weeks, false));
        assert!(pages.metrics().contains(&Metric::Pages));
    }

    #[test]
    fn statistics_are_kept_in_the_settings_folder() {
        let dir = std::env::temp_dir().join(format!("jcj-shell-stats-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let fresh = Statistics { goals: Goals { minutes_per_day: 25, ..Goals::default() }, ..Statistics::default() };
        let (s, problem) = Statistics::open(&dir, fresh.clone());
        assert_eq!((s.clone(), problem), (fresh, None), "no file: the app's fresh statistics");
        let mut s = Statistics::from_json(EARLIER_FILE).unwrap();
        s.goals.words_per_day = 500;
        s.save(&dir).unwrap();
        assert_eq!(Statistics::open(&dir, Statistics::default()).0, s);
        std::fs::write(dir.join(FILE), "not json").unwrap();
        let (back, problem) = Statistics::open(&dir, Statistics::default());
        assert_eq!(back, Statistics::default());
        assert!(problem.is_some() && dir.join(format!("{FILE}.damaged")).is_file(), "a damaged file is kept aside");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_window_shows_and_changes_the_goal_in_the_app_s_unit() {
        use egui_kittest::Harness;
        use egui_kittest::kittest::Queryable;

        let mut s = Statistics::from_json(EARLIER_FILE).unwrap();
        s.goals.words_per_day = 300;
        let view = StatsView { open: true, zone: "UTC".into(), ..StatsView::new(Unit::Pages) };
        type State = (Statistics, StatsView, bool);
        let mut h = Harness::builder().with_size(vec2(700.0, 600.0)).build_ui_state(
            |ui, st: &mut State| {
                st.2 |= statistics_window(ui.ctx(), &mut st.0, &mut st.1, Day { year: 2026, month: 11, day: 22 });
            },
            (s, view, false),
        );
        h.run();
        assert!(h.query_by_label("Writing Statistics").is_some());
        assert!(h.query_by_label("09:00 to 10:00 UTC").is_some());
        assert!(h.query_by_label("Friday").is_some());
        h.get_by_label("Minutes").click();
        h.run();
        assert_eq!(h.state().1.metric, Metric::Minutes);
        h.get_by_label("Keep statistics").click();
        h.run();
        assert!(!h.state().0.enabled && h.state().2, "a setting changed is reported");
    }
}
