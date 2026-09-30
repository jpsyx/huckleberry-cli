//! What "today" means to this family, and what it adds up to.
//!
//! There is no one answer, which is why this is configured rather than
//! assumed. Midnight is the one answer that is wrong for everybody: nobody
//! with a baby is awake at midnight thinking of it as a boundary, and a 4am
//! feed filed under a fresh day is a 4am feed nobody can find.
//!
//! Two things are configured, and they are independent.
//!
//! **When a day starts and ends** is always known, because every screen with a
//! day on it needs both: the summary counts its rows between them, the stripe
//! chart draws its rows from one day's end to the next, and the night is the
//! stretch between the end of one day and the start of the next.
//!
//! **How "today" is counted** is separate, and decides only whether the
//! running totals on the 3am screen use those hours or ignore them for a
//! rolling twenty-four. A newborn's day has no shape: feeds and sleeps are
//! scattered round the clock, and the only honest window is the last
//! twenty-four hours. An older baby has a day with a beginning.
//!
//! Everything here is pure and takes `now` as an argument, like the rest of
//! `domain`.

use jiff::civil::Date;

use super::time::Calendar;
use super::types::{Child, Dataset, FeedEvent};

/// How a family counts "today" for the running totals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayMode {
    /// A rolling twenty-four hours, ending now. The day hours are ignored.
    Continuous,
    /// From the hour the family calls the start of the day.
    Discrete,
}

impl DayMode {
    /// The spelling used in the configuration file.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Continuous => "continuous",
            Self::Discrete => "discrete",
        }
    }

    /// The mode a setting names, or `None` when it names neither.
    #[must_use]
    pub fn from_key(key: &str) -> Option<Self> {
        match key {
            "continuous" => Some(Self::Continuous),
            "discrete" => Some(Self::Discrete),
            _ => None,
        }
    }

    /// One line saying what this mode does, for the question that asks.
    #[must_use]
    pub const fn describe(self) -> &'static str {
        match self {
            Self::Continuous => "a rolling 24 hours, which suits a newborn",
            Self::Discrete => "from the hour the day starts",
        }
    }
}

/// How this family counts a day.
///
/// The two hours are always known, because everything with a day on it needs
/// them. [`mode`](Self::mode) decides only whether the running totals use the
/// day hours or a rolling window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DayRule {
    /// Rolling, or from the hour the day starts.
    pub mode: DayMode,
    /// When a day begins.
    pub day_start_hour: f64,
    /// When a day ends and the night begins.
    pub day_end_hour: f64,
}

impl DayRule {
    /// When a day begins when nobody has said.
    ///
    /// Huckleberry's own profile default, so a configuration nobody has
    /// finished draws the same boundaries the app already assumed rather than
    /// a boundary nobody chose.
    pub const DEFAULT_DAY_START: f64 = 7.0;

    /// When a day ends when nobody has said. Huckleberry's default too.
    pub const DEFAULT_DAY_END: f64 = 20.0;

    /// A rule with both hours and a mode.
    #[must_use]
    pub const fn new(mode: DayMode, day_start_hour: f64, day_end_hour: f64) -> Self {
        Self {
            mode,
            day_start_hour,
            day_end_hour,
        }
    }

    /// Totals over a rolling twenty-four hours; the day hours still draw every
    /// other screen.
    #[must_use]
    pub const fn continuous(day_start_hour: f64, day_end_hour: f64) -> Self {
        Self::new(DayMode::Continuous, day_start_hour, day_end_hour)
    }

    /// Totals from the hour the day starts.
    #[must_use]
    pub const fn discrete(day_start_hour: f64, day_end_hour: f64) -> Self {
        Self::new(DayMode::Discrete, day_start_hour, day_end_hour)
    }

    /// The window "today" means, as of `now`.
    #[must_use]
    pub fn window(&self, calendar: &Calendar, now: f64) -> Window {
        match self.mode {
            DayMode::Continuous => Window {
                start: now - 86_400.0,
                end: now,
                mode: DayMode::Continuous,
                began_at_hour: None,
            },
            DayMode::Discrete => Window {
                start: self.day_bounds(calendar, self.day_of(calendar, now)).0,
                end: now,
                mode: DayMode::Discrete,
                began_at_hour: Some(self.day_start_hour),
            },
        }
    }

    /// Replaces the child's night with this family's own.
    ///
    /// Applied to the dataset once, where it is read, so every screen that
    /// asks the child about its night gets the same answer without each of
    /// them having to know these settings exist. The night is simply the
    /// stretch between the end of one day and the start of the next, which is
    /// why there is no third hour to configure.
    pub const fn apply_to(&self, child: &mut Child) {
        child.night_start_hour = self.day_end_hour;
        child.morning_cutoff_hour = self.day_start_hour;
    }

    // -- Days, as this family counts them -----------------------------------

    /// When the day named `day` begins and ends.
    #[must_use]
    pub fn day_bounds(&self, calendar: &Calendar, day: Date) -> (f64, f64) {
        (
            calendar.at_hour_fraction(day, self.day_start_hour),
            calendar.at_hour_fraction(calendar.offset_day(day, 1), self.day_start_hour),
        )
    }

    /// Which day an instant belongs to.
    ///
    /// Under a day starting at 6am, a 4am feed belongs to the day before,
    /// which is where the person who gave it will look for it.
    #[must_use]
    pub fn day_of(&self, calendar: &Calendar, at: f64) -> Date {
        let named = calendar.day_of(at);
        if at >= calendar.at_hour_fraction(named, self.day_start_hour) {
            named
        } else {
            calendar.offset_day(named, -1)
        }
    }

    /// The days ending at `at`, newest first.
    #[must_use]
    pub fn recent_days(&self, calendar: &Calendar, at: f64, count: usize) -> Vec<Date> {
        let today = self.day_of(calendar, at);
        (0..count)
            .map(|back| calendar.offset_day(today, -i32::try_from(back).unwrap_or(i32::MAX)))
            .collect()
    }

    /// How a span divides across the days it covered.
    ///
    /// Sleep seconds are apportioned to the days they actually fell in, which
    /// is what stops a night being lost off one day or counted twice.
    #[must_use]
    pub fn split_across_days(
        &self,
        calendar: &Calendar,
        start: f64,
        duration: f64,
    ) -> Vec<(Date, f64)> {
        split(start, duration, |at| {
            let day = self.day_of(calendar, at);
            (day, self.day_bounds(calendar, day).1)
        })
    }

    // -- Stripe rows, which open on the night ------------------------------

    /// When the stripe row named `day` begins and ends.
    ///
    /// It opens at the previous day's end, so a night appears whole on one row
    /// instead of cut in two by a boundary nobody sleeps through. A row is
    /// therefore "the night leading into this day, and then this day", which
    /// is how the night is talked about anyway.
    #[must_use]
    pub fn stripe_bounds(&self, calendar: &Calendar, day: Date) -> (f64, f64) {
        (
            calendar.at_hour_fraction(calendar.offset_day(day, -1), self.day_end_hour),
            calendar.at_hour_fraction(day, self.day_end_hour),
        )
    }

    /// Which stripe row an instant falls on.
    #[must_use]
    pub fn stripe_day_of(&self, calendar: &Calendar, at: f64) -> Date {
        let named = calendar.day_of(at);
        if at < calendar.at_hour_fraction(named, self.day_end_hour) {
            named
        } else {
            calendar.offset_day(named, 1)
        }
    }

    /// The stripe rows ending at `at`, newest first.
    #[must_use]
    pub fn recent_stripe_days(&self, calendar: &Calendar, at: f64, count: usize) -> Vec<Date> {
        let today = self.stripe_day_of(calendar, at);
        (0..count)
            .map(|back| calendar.offset_day(today, -i32::try_from(back).unwrap_or(i32::MAX)))
            .collect()
    }
}

impl DayRule {
    /// Huckleberry's own hours, for a screen drawn before anybody has said.
    #[must_use]
    pub const fn assumed() -> Self {
        Self::discrete(Self::DEFAULT_DAY_START, Self::DEFAULT_DAY_END)
    }
}

impl Default for DayRule {
    fn default() -> Self {
        Self::assumed()
    }
}

/// Divides a span at whatever boundaries `next` reports.
fn split(start: f64, duration: f64, next: impl Fn(f64) -> (Date, f64)) -> Vec<(Date, f64)> {
    let end = start + duration;
    let mut pieces = Vec::new();
    let mut at = start;
    // Bounded by the window any caller reads, and by the boundary always
    // moving forward; a zero-length span still yields its own day.
    while at < end || pieces.is_empty() {
        let (day, boundary) = next(at);
        let stop = boundary.min(end);
        pieces.push((day, (stop - at).max(0.0)));
        if stop <= at {
            break;
        }
        at = stop;
    }
    pieces
}

/// The stretch of time a screen means by "today".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window {
    /// When it began.
    pub start: f64,
    /// When it ends, which is always now.
    pub end: f64,
    /// Which rule drew it, so a label never has to guess what it means.
    pub mode: DayMode,
    /// The hour it began at, when the family counts discrete days.
    pub began_at_hour: Option<f64>,
}

impl Window {
    /// How long the window is, in seconds.
    #[must_use]
    pub fn seconds(&self) -> f64 {
        (self.end - self.start).max(0.0)
    }
}

/// What a window amounts to.
///
/// Nothing here is ever a guess. A bottle nobody put an amount on is a feed
/// and no millilitres, because `0 ml` is a claim about the baby and an absent
/// amount is a claim about the record.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Totals {
    /// Millilitres taken by bottle.
    pub millilitres: f64,
    /// Seconds spent nursing.
    pub nursing_seconds: f64,
    /// Bottles and nursing sessions together.
    pub milk_feeds: usize,
    /// Meals.
    pub solids: usize,
    /// Sleep, counted only for the part that fell inside the window.
    pub sleep_seconds: f64,
}

impl Totals {
    /// Whether anything at all was recorded in the window.
    ///
    /// False means a gap in the record, not a quiet day, and the difference is
    /// what stops a screen printing a confident zero.
    #[must_use]
    pub fn anything_recorded(&self) -> bool {
        self.milk_feeds > 0 || self.solids > 0 || self.sleep_seconds > 0.0
    }
}

/// Everything recorded between two instants.
#[must_use]
pub fn totals(dataset: &Dataset, start: f64, end: f64) -> Totals {
    let mut totals = Totals::default();
    for feed in &dataset.feeds {
        if !inside(feed.start(), start, end) {
            continue;
        }
        match feed {
            FeedEvent::Bottle { amount_ml, .. } => {
                totals.milk_feeds += 1;
                totals.millilitres += amount_ml.unwrap_or(0.0);
            }
            FeedEvent::Nursing {
                left_seconds,
                right_seconds,
                ..
            } => {
                totals.milk_feeds += 1;
                totals.nursing_seconds += left_seconds + right_seconds;
            }
            FeedEvent::Solids { .. } => totals.solids += 1,
        }
    }
    for asleep in &dataset.sleep {
        totals.sleep_seconds += overlap(asleep.start, asleep.end(), start, end);
    }
    // A sleep in progress has not reached history yet, and a baby asleep right
    // now has slept that time today whatever the intervals collection says.
    if dataset.live.sleep_active
        && let Some(began) = dataset.live.sleep_start
    {
        totals.sleep_seconds += overlap(began, end, start, end);
    }
    totals
}

/// Whether an instant falls in `[start, end)`.
const fn inside(at: f64, start: f64, end: f64) -> bool {
    at >= start && at <= end
}

/// How much of one span falls inside another.
fn overlap(start: f64, end: f64, window_start: f64, window_end: f64) -> f64 {
    (end.min(window_end) - start.max(window_start)).max(0.0)
}

#[cfg(test)]
mod windows {
    use super::*;
    use crate::domain::Calendar;
    use crate::domain::fixtures::{AFTERNOON, THREE_AM, bottle, dataset, nursing, sleep};

    fn calendar() -> Calendar {
        Calendar::new("America/New_York").expect("a real timezone")
    }

    /// Days from 6am, nights from 7:30pm.
    fn rule(mode: DayMode) -> DayRule {
        DayRule::new(mode, 6.0, 19.5)
    }

    #[test]
    fn a_continuous_day_is_the_last_twenty_four_hours_whenever_you_ask() {
        let rule = rule(DayMode::Continuous);
        for at in [AFTERNOON, THREE_AM] {
            let window = rule.window(&calendar(), at);
            assert!((window.end - at).abs() < f64::EPSILON);
            assert!((window.start - (at - 86_400.0)).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn a_discrete_day_begins_at_the_hour_the_family_chose() {
        let rule = rule(DayMode::Discrete);
        let calendar = calendar();
        // 2pm: today began at 6am today.
        let afternoon = rule.window(&calendar, AFTERNOON);
        assert_eq!(calendar.day_of(afternoon.start), calendar.day_of(AFTERNOON));
        assert!((afternoon.start - calendar.at(calendar.day_of(AFTERNOON), 6, 0)).abs() < 1.0);
    }

    #[test]
    fn a_four_in_the_morning_feed_still_belongs_to_the_day_before() {
        let calendar = calendar();
        let rule = rule(DayMode::Discrete);
        // 3am, which is before a 6am day start, so the day began yesterday.
        let window = rule.window(&calendar, THREE_AM);
        let yesterday = calendar.offset_day(calendar.day_of(THREE_AM), -1);
        assert_eq!(calendar.day_of(window.start), yesterday);
        assert!(
            window.start < THREE_AM,
            "the day it began has already begun"
        );
        assert!(
            THREE_AM - window.start < 86_400.0,
            "and it is less than a day old"
        );
    }

    #[test]
    fn with_nobody_having_said_the_day_is_the_one_huckleberry_assumes() {
        let rule = DayRule::default();
        assert!((rule.day_start_hour - 7.0).abs() < f64::EPSILON);
        assert!((rule.day_end_hour - 20.0).abs() < f64::EPSILON);
        assert_eq!(rule.mode, DayMode::Discrete);
    }

    #[test]
    fn the_window_says_which_rule_drew_it_so_a_label_never_has_to_guess() {
        let calendar = calendar();
        assert_eq!(
            rule(DayMode::Continuous).window(&calendar, AFTERNOON).mode,
            DayMode::Continuous
        );
        let discrete = rule(DayMode::Discrete).window(&calendar, AFTERNOON);
        assert_eq!(discrete.mode, DayMode::Discrete);
        assert_eq!(discrete.began_at_hour, Some(6.0));
    }

    #[test]
    fn the_family_night_replaces_the_profile_one_whichever_mode_they_count_in() {
        for mode in [DayMode::Discrete, DayMode::Continuous] {
            let mut data = dataset();
            DayRule::new(mode, 6.5, 19.0).apply_to(&mut data.child);
            assert!(
                (data.child.night_start_hour - 19.0).abs() < f64::EPSILON,
                "{mode:?}"
            );
            assert!(
                (data.child.morning_cutoff_hour - 6.5).abs() < f64::EPSILON,
                "the night ends where the day begins, or the two disagree: {mode:?}"
            );
        }
    }

    #[test]
    fn a_day_runs_from_its_start_to_the_next_ones() {
        let calendar = calendar();
        let rule = rule(DayMode::Discrete);
        let today = rule.day_of(&calendar, AFTERNOON);
        let (start, end) = rule.day_bounds(&calendar, today);
        assert!((start - calendar.at(today, 6, 0)).abs() < 1.0);
        assert!((end - calendar.at(calendar.offset_day(today, 1), 6, 0)).abs() < 1.0);
        assert!(start <= AFTERNOON && AFTERNOON < end);
    }

    #[test]
    fn an_instant_before_the_day_started_belongs_to_the_day_before() {
        let calendar = calendar();
        let rule = rule(DayMode::Discrete);
        // 3am with a 6am day start: still yesterday's day.
        assert_eq!(
            rule.day_of(&calendar, THREE_AM),
            calendar.offset_day(calendar.day_of(THREE_AM), -1)
        );
        // 2pm: today's.
        assert_eq!(
            rule.day_of(&calendar, AFTERNOON),
            calendar.day_of(AFTERNOON)
        );
    }

    #[test]
    fn recent_days_count_back_from_the_day_that_is_running() {
        let calendar = calendar();
        let rule = rule(DayMode::Discrete);
        let days = rule.recent_days(&calendar, THREE_AM, 3);
        assert_eq!(days.len(), 3);
        assert_eq!(days[0], rule.day_of(&calendar, THREE_AM));
        assert_eq!(days[1], calendar.offset_day(days[0], -1));
    }

    #[test]
    fn a_sleep_across_the_day_start_is_divided_at_it_and_nowhere_else() {
        let calendar = calendar();
        let rule = rule(DayMode::Discrete);
        let today = rule.day_of(&calendar, AFTERNOON);
        let boundary = rule.day_bounds(&calendar, today).1;
        // Two hours, straddling the 6am boundary by an hour each side.
        let pieces = rule.split_across_days(&calendar, boundary - 3_600.0, 7_200.0);
        assert_eq!(pieces.len(), 2, "{pieces:?}");
        assert_eq!(pieces[0].0, today);
        assert_eq!(pieces[1].0, calendar.offset_day(today, 1));
        assert!((pieces[0].1 - 3_600.0).abs() < 1.0);
        assert!((pieces[1].1 - 3_600.0).abs() < 1.0);
        assert!(
            (pieces.iter().map(|(_, seconds)| seconds).sum::<f64>() - 7_200.0).abs() < 1.0,
            "nothing is lost or counted twice"
        );
    }

    #[test]
    fn a_sleep_inside_one_day_is_not_divided_at_all() {
        let calendar = calendar();
        let pieces = rule(DayMode::Discrete).split_across_days(&calendar, AFTERNOON, 3_600.0);
        assert_eq!(pieces.len(), 1);
        assert!((pieces[0].1 - 3_600.0).abs() < 1.0);
    }

    #[test]
    fn a_stripe_row_opens_on_the_night_before_the_day_it_names() {
        let calendar = calendar();
        let rule = rule(DayMode::Discrete);
        let day = calendar.day_of(AFTERNOON);
        let (start, end) = rule.stripe_bounds(&calendar, day);
        assert!(
            (start - calendar.at_hour_fraction(calendar.offset_day(day, -1), 19.5)).abs() < 1.0,
            "the row starts at the previous day's end"
        );
        assert!((end - calendar.at_hour_fraction(day, 19.5)).abs() < 1.0);
        assert!(
            (end - start - 86_400.0).abs() < 1.0,
            "and still covers exactly one day"
        );
    }

    #[test]
    fn a_night_lands_whole_on_one_stripe_row_instead_of_split_by_midnight() {
        let calendar = calendar();
        let rule = rule(DayMode::Discrete);
        let evening = calendar.at(calendar.day_of(AFTERNOON), 22, 0);
        let small_hours = calendar.at(calendar.offset_day(calendar.day_of(AFTERNOON), 1), 3, 0);
        assert_eq!(
            rule.stripe_day_of(&calendar, evening),
            rule.stripe_day_of(&calendar, small_hours),
            "10pm and 3am are the same night, so they belong on the same row"
        );
    }

    #[test]
    fn a_stripe_row_is_named_for_the_day_its_daytime_falls_in() {
        let calendar = calendar();
        let rule = rule(DayMode::Discrete);
        assert_eq!(
            rule.stripe_day_of(&calendar, AFTERNOON),
            calendar.day_of(AFTERNOON),
            "2pm is on the row named for today"
        );
        let rows = rule.recent_stripe_days(&calendar, AFTERNOON, 2);
        assert_eq!(rows[0], calendar.day_of(AFTERNOON));
        assert_eq!(rows[1], calendar.offset_day(rows[0], -1));
    }

    #[test]
    fn totals_add_up_the_bottles_the_nursing_and_the_meals_inside_the_window() {
        let mut data = dataset();
        data.feeds = vec![
            bottle(AFTERNOON - 7_200.0, 90.0),
            bottle(AFTERNOON - 1_800.0, 60.0),
            // Older than the window, so it is not counted.
            bottle(AFTERNOON - 20_000.0, 500.0),
            nursing(AFTERNOON - 3_600.0, 600.0, 300.0),
        ];
        let totals = totals(&data, AFTERNOON - 10_800.0, AFTERNOON);
        assert!((totals.millilitres - 150.0).abs() < f64::EPSILON);
        assert!((totals.nursing_seconds - 900.0).abs() < f64::EPSILON);
        assert_eq!(totals.milk_feeds, 3);
    }

    #[test]
    fn a_bottle_with_no_amount_is_a_feed_and_no_millilitres() {
        let mut data = dataset();
        data.feeds = vec![crate::domain::types::FeedEvent::Bottle {
            at: None,
            id: "f".into(),
            start: AFTERNOON - 600.0,
            amount_ml: None,
            bottle_type: None,
            notes: None,
        }];
        let totals = totals(&data, AFTERNOON - 10_800.0, AFTERNOON);
        assert_eq!(totals.milk_feeds, 1);
        assert!(
            totals.millilitres.abs() < f64::EPSILON,
            "nobody recorded an amount, so there is no amount"
        );
    }

    #[test]
    fn sleep_is_counted_only_for_the_part_inside_the_window() {
        let mut data = dataset();
        // Three hours of sleep, of which the last hour is inside the window.
        data.sleep = vec![sleep(AFTERNOON - 14_400.0, 10_800.0)];
        let totals = totals(&data, AFTERNOON - 7_200.0, AFTERNOON);
        assert!(
            (totals.sleep_seconds - 3_600.0).abs() < 1.0,
            "{}",
            totals.sleep_seconds
        );
    }

    #[test]
    fn a_sleep_running_right_now_counts_towards_today() {
        let mut data = dataset();
        data.live.sleep_active = true;
        data.live.sleep_start = Some(AFTERNOON - 1_800.0);
        let totals = totals(&data, AFTERNOON - 86_400.0, AFTERNOON);
        assert!(
            (totals.sleep_seconds - 1_800.0).abs() < 1.0,
            "history cannot know about a sleep in progress, so this has to: {}",
            totals.sleep_seconds
        );
    }

    #[test]
    fn nothing_recorded_adds_up_to_nothing_rather_than_to_a_guess() {
        let totals = totals(&dataset(), AFTERNOON - 86_400.0, AFTERNOON);
        assert_eq!(totals.milk_feeds, 0);
        assert_eq!(totals.solids, 0);
        assert!(totals.millilitres.abs() < f64::EPSILON);
        assert!(totals.sleep_seconds.abs() < f64::EPSILON);
        assert!(!totals.anything_recorded());
    }
}
