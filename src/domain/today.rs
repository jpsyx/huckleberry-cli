//! What "today" means to this family, and what it adds up to.
//!
//! There is no one answer, which is why this is a setting rather than a
//! constant. A newborn's day has no shape: feeds and sleeps are scattered
//! round the clock, and the only honest window is the last twenty-four hours.
//! An older baby has a day with a beginning, and the question "how much has
//! she eaten today" means since she woke, not since midnight.
//!
//! Midnight itself is the one answer that is wrong for everybody. Nobody with
//! a baby is awake at midnight thinking of it as a boundary, and a 4am feed
//! filed under a fresh day is a 4am feed nobody can find.
//!
//! Everything here is pure and takes `now` as an argument, like the rest of
//! `domain`.

use super::time::Calendar;
use super::types::{Child, Dataset, FeedEvent};

/// How a family counts a day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayMode {
    /// A rolling twenty-four hours, ending now.
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
            Self::Discrete => "from a set hour each morning",
        }
    }
}

/// How this family counts a day, and where its night sits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DayRule {
    /// Rolling, or from an hour.
    pub mode: DayMode,
    /// When a day begins, as an hour fraction. Only read when discrete.
    pub day_start_hour: Option<f64>,
    /// When night begins, when the family said rather than Huckleberry.
    pub night_start_hour: Option<f64>,
}

/// Where a day begins when a discrete family has not said.
///
/// Midnight, which is wrong for everybody, and is why setup asks.
const MIDNIGHT: f64 = 0.0;

impl DayRule {
    /// A rolling twenty-four hours, with the profile's night left alone.
    #[must_use]
    pub const fn continuous() -> Self {
        Self {
            mode: DayMode::Continuous,
            day_start_hour: None,
            night_start_hour: None,
        }
    }

    /// Days from an hour, with the night ending at that same hour.
    #[must_use]
    pub const fn discrete(day_start_hour: f64, night_start_hour: Option<f64>) -> Self {
        Self {
            mode: DayMode::Discrete,
            day_start_hour: Some(day_start_hour),
            night_start_hour,
        }
    }

    /// Discrete days with nobody having said when one begins.
    #[must_use]
    pub const fn discrete_default() -> Self {
        Self {
            mode: DayMode::Discrete,
            day_start_hour: None,
            night_start_hour: None,
        }
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
            DayMode::Discrete => {
                let hour = self.day_start_hour.unwrap_or(MIDNIGHT);
                Window {
                    start: most_recent_day_start(calendar, hour, now),
                    end: now,
                    mode: DayMode::Discrete,
                    began_at_hour: self.day_start_hour,
                }
            }
        }
    }

    /// Replaces the child's night with the one the family configured.
    ///
    /// Applied to the dataset once, where it is read, so every screen that
    /// asks the child about its night gets the same answer without each of
    /// them having to know this setting exists. The night ends where the day
    /// begins, because a family that has said both and had them disagree would
    /// have an hour belonging to neither.
    pub fn apply_to(&self, child: &mut Child) {
        if let Some(night) = self.night_start_hour {
            child.night_start_hour = night;
        }
        if self.mode == DayMode::Discrete
            && let Some(start) = self.day_start_hour
        {
            child.morning_cutoff_hour = start;
        }
    }
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
    /// The hour it began at, when a discrete family named one.
    pub began_at_hour: Option<f64>,
}

impl Window {
    /// How long the window is, in seconds.
    #[must_use]
    pub fn seconds(&self) -> f64 {
        (self.end - self.start).max(0.0)
    }
}

/// The most recent occurrence of an hour, at or before `now`.
fn most_recent_day_start(calendar: &Calendar, hour: f64, now: f64) -> f64 {
    let today = calendar.day_of(now);
    let began = calendar.at_hour_fraction(today, hour);
    if began <= now {
        return began;
    }
    calendar.at_hour_fraction(calendar.offset_day(today, -1), hour)
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

    #[test]
    fn a_continuous_day_is_the_last_twenty_four_hours_whenever_you_ask() {
        let rule = DayRule::continuous();
        for at in [AFTERNOON, THREE_AM] {
            let window = rule.window(&calendar(), at);
            assert!((window.end - at).abs() < f64::EPSILON);
            assert!((window.start - (at - 86_400.0)).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn a_discrete_day_begins_at_the_hour_the_family_chose() {
        let rule = DayRule::discrete(6.0, None);
        let calendar = calendar();
        // 2pm: today began at 6am today.
        let afternoon = rule.window(&calendar, AFTERNOON);
        assert_eq!(calendar.day_of(afternoon.start), calendar.day_of(AFTERNOON));
        assert!((afternoon.start - calendar.at(calendar.day_of(AFTERNOON), 6, 0)).abs() < 1.0);
    }

    #[test]
    fn a_four_in_the_morning_feed_still_belongs_to_the_day_before() {
        let calendar = calendar();
        let rule = DayRule::discrete(6.0, None);
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
    fn a_discrete_day_with_no_hour_chosen_falls_back_to_midnight() {
        let calendar = calendar();
        let window = DayRule::discrete_default().window(&calendar, AFTERNOON);
        assert!((window.start - calendar.start_of(calendar.day_of(AFTERNOON))).abs() < 1.0);
    }

    #[test]
    fn the_window_says_which_rule_drew_it_so_a_label_never_has_to_guess() {
        let calendar = calendar();
        assert_eq!(
            DayRule::continuous().window(&calendar, AFTERNOON).mode,
            DayMode::Continuous
        );
        let discrete = DayRule::discrete(6.0, None).window(&calendar, AFTERNOON);
        assert_eq!(discrete.mode, DayMode::Discrete);
        assert_eq!(discrete.began_at_hour, Some(6.0));
    }

    #[test]
    fn a_configured_night_replaces_the_one_on_the_profile() {
        let mut data = dataset();
        DayRule::discrete(6.5, Some(19.0)).apply_to(&mut data.child);
        assert!((data.child.night_start_hour - 19.0).abs() < f64::EPSILON);
        assert!(
            (data.child.morning_cutoff_hour - 6.5).abs() < f64::EPSILON,
            "the night ends where the day begins, or the two disagree"
        );
    }

    #[test]
    fn a_continuous_family_keeps_the_night_huckleberry_gave_them() {
        let mut data = dataset();
        let before = (data.child.night_start_hour, data.child.morning_cutoff_hour);
        DayRule::continuous().apply_to(&mut data.child);
        assert_eq!(
            (data.child.night_start_hour, data.child.morning_cutoff_hour),
            before
        );
    }

    #[test]
    fn a_night_start_on_its_own_still_takes_effect() {
        let mut data = dataset();
        let rule = DayRule {
            mode: DayMode::Continuous,
            day_start_hour: None,
            night_start_hour: Some(21.0),
        };
        rule.apply_to(&mut data.child);
        assert!((data.child.night_start_hour - 21.0).abs() < f64::EPSILON);
        assert!((data.child.morning_cutoff_hour - 7.0).abs() < f64::EPSILON);
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
