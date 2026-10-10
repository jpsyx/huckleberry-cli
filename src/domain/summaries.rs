//! One row per day: the numbers a pediatrician asks for.
//!
//! One asymmetry is worth knowing about, because it looks like a bug until you
//! see why. Sleep **seconds** are split across family-day boundaries, so a sleep beginning
//! at 23:46 contributes to both days. Sleep **counts** and the longest stretch
//! are attributed to the day the sleep began. Splitting the counts would turn
//! one overnight into two sleeps; not splitting the seconds would quietly lose
//! the small hours off every night, which for a newborn is most of the sleep
//! there is.

mod sleep;

use jiff::civil::Date;

use super::time::Calendar;
use super::today::DayRule;
use super::types::{Dataset, FeedEvent};

/// Everything one day amounts to.
#[derive(Debug, Clone, PartialEq)]
pub struct DaySummary {
    /// Which day.
    pub day: Date,
    /// Whether the day is unfinished or incompletely covered, so it cannot enter averages.
    pub partial: bool,
    /// Milk feeds: bottles and nursing sessions. Meals are counted separately.
    pub feed_count: usize,
    /// How many were bottles.
    pub bottle_count: usize,
    /// Bottles with a recorded amount, the denominator of the milk average.
    pub measured_bottle_count: usize,
    /// How many were nursing sessions.
    pub nursing_count: usize,
    /// How many were meals.
    pub solids_count: usize,
    /// Millilitres taken by bottle.
    pub total_ml: f64,
    /// Milk from feeds starting in the configured daytime.
    pub day_milk_ml: f64,
    /// Milk from feeds starting in the configured night.
    pub night_milk_ml: f64,
    /// Nursing duration from sessions starting in daytime.
    pub day_nursing_seconds: f64,
    /// Nursing duration from sessions starting at night.
    pub night_nursing_seconds: f64,
    /// Of which formula.
    pub formula_ml: f64,
    /// Of which expressed milk.
    pub breast_milk_ml: f64,
    /// Seconds spent nursing.
    pub nursing_seconds: f64,
    /// Of which on the left.
    pub left_seconds: f64,
    /// Of which on the right.
    pub right_seconds: f64,
    /// The mean gap between milk feeds, when there were at least two.
    pub average_feed_gap_seconds: Option<f64>,
    /// The longest gap between milk feeds.
    pub longest_feed_gap_seconds: Option<f64>,
    /// Wet diapers.
    pub wet_count: usize,
    /// Dirty diapers.
    pub dirty_count: usize,
    /// Diapers of any kind.
    pub diaper_count: usize,
    /// Diapers on which a rash was noted.
    pub rash_count: usize,
    /// Total sleep, apportioned to the days it actually covered.
    pub sleep_seconds: f64,
    /// Of which inside the family's night window.
    pub night_sleep_seconds: f64,
    /// Of which outside it.
    pub day_sleep_seconds: f64,
    /// Sleeps that began on this day.
    pub sleep_count: usize,
    /// The longest of them.
    pub longest_sleep_seconds: f64,
    /// Mean full duration of all completed sleeps starting on this day.
    pub average_sleep_seconds: Option<f64>,
    /// Mean full duration of completed sleeps starting at night on this day.
    pub average_night_sleep_seconds: Option<f64>,
    /// Mean duration of completed sleeps starting in daytime on this day.
    pub average_nap_seconds: Option<f64>,
    /// Elapsed day time minus recorded sleep, absent without sleep records.
    pub wake_seconds: Option<f64>,
    /// Elapsed night time minus recorded night sleep.
    pub night_wake_seconds: Option<f64>,
    /// Elapsed daytime minus recorded daytime sleep.
    pub day_wake_seconds: Option<f64>,
    /// Mean complete gap starting at night, attributed to the day waking began.
    pub average_night_wake_seconds: Option<f64>,
    /// Mean complete gap starting in daytime, attributed to the day waking began.
    pub average_day_wake_seconds: Option<f64>,
    /// Mean complete gap between sleeps, attributed to the day waking began.
    pub average_wake_seconds: Option<f64>,
    /// Longest complete gap between sleeps, attributed to the day waking began.
    pub longest_wake_seconds: Option<f64>,
    /// Pumping sessions.
    pub pump_count: usize,
    /// Millilitres expressed.
    pub pumped_ml: f64,
    /// Milestones recorded.
    pub milestone_count: usize,
    /// Whether anything was tracked at all.
    ///
    /// False means a gap in the record, not a quiet day. Pumping and
    /// milestones deliberately do not count towards it: a pumping session
    /// belongs to the parent and a milestone is a memory, so neither makes a
    /// day eligible to be averaged into a feeding or diaper figure.
    pub has_data: bool,
}

impl DaySummary {
    /// Mean milk volume over bottles with amounts, including formula and breast milk.
    #[must_use]
    pub fn average_milk_ml(&self) -> Option<f64> {
        (self.measured_bottle_count > 0).then(|| self.total_ml / self.measured_bottle_count as f64)
    }

    /// Percentage of recorded milk volume taken in daytime, absent with no milk volume.
    #[must_use]
    pub fn day_milk_percent(&self) -> Option<f64> {
        (self.total_ml > 0.0).then(|| self.day_milk_ml / self.total_ml * 100.0)
    }

    /// Mean nursing duration over nursing sessions only.
    #[must_use]
    pub fn average_nursing_seconds(&self) -> Option<f64> {
        (self.nursing_count > 0).then(|| self.nursing_seconds / self.nursing_count as f64)
    }

    const fn empty(day: Date, partial: bool) -> Self {
        Self {
            day,
            partial,
            feed_count: 0,
            bottle_count: 0,
            measured_bottle_count: 0,
            nursing_count: 0,
            solids_count: 0,
            total_ml: 0.0,
            day_milk_ml: 0.0,
            night_milk_ml: 0.0,
            day_nursing_seconds: 0.0,
            night_nursing_seconds: 0.0,
            formula_ml: 0.0,
            breast_milk_ml: 0.0,
            nursing_seconds: 0.0,
            left_seconds: 0.0,
            right_seconds: 0.0,
            average_feed_gap_seconds: None,
            longest_feed_gap_seconds: None,
            wet_count: 0,
            dirty_count: 0,
            diaper_count: 0,
            rash_count: 0,
            sleep_seconds: 0.0,
            night_sleep_seconds: 0.0,
            day_sleep_seconds: 0.0,
            sleep_count: 0,
            longest_sleep_seconds: 0.0,
            average_nap_seconds: None,
            average_sleep_seconds: None,
            average_night_sleep_seconds: None,
            wake_seconds: None,
            night_wake_seconds: None,
            day_wake_seconds: None,
            average_night_wake_seconds: None,
            average_day_wake_seconds: None,
            average_wake_seconds: None,
            longest_wake_seconds: None,
            pump_count: 0,
            pumped_ml: 0.0,
            milestone_count: 0,
            has_data: false,
        }
    }
}

/// Start of the oldest requested complete family day, using local date arithmetic.
#[must_use]
pub fn history_start(calendar: &Calendar, rule: DayRule, now: f64, complete_days: u32) -> f64 {
    let today = rule.day_of(calendar, now);
    let oldest = calendar.offset_day(today, -i32::try_from(complete_days).unwrap_or(i32::MAX));
    rule.day_bounds(calendar, oldest).0
}

/// Marks rows outside known snapshot coverage as partial, excluding their means.
pub fn exclude_incomplete(
    rows: &mut [DaySummary],
    calendar: &Calendar,
    rule: DayRule,
    coverage: (f64, f64),
) {
    for row in rows {
        let (start, end) = rule.day_bounds(calendar, row.day);
        row.partial |= start < coverage.0 || end > coverage.1;
    }
}

/// One row per day, newest first, for the last `days` days.
///
/// A day is this family's day, not a calendar one: under a day starting at
/// 6am, a 4am feed is counted on the row before, which is where the person who
/// gave it will look for it. See [`super::today`].
#[must_use]
pub fn build(
    dataset: &Dataset,
    calendar: &Calendar,
    rule: DayRule,
    now: f64,
    days: usize,
) -> Vec<DaySummary> {
    let today = rule.day_of(calendar, now);
    let wanted = rule.recent_days(calendar, now, days);
    let mut rows: Vec<DaySummary> = wanted
        .iter()
        .map(|day| DaySummary::empty(*day, *day == today))
        .collect();
    let index = |day: Date| wanted.iter().position(|candidate| *candidate == day);

    let mut feed_starts: Vec<Vec<f64>> = vec![Vec::new(); rows.len()];

    for feed in &dataset.feeds {
        let Some(slot) = index(rule.day_of(calendar, feed.start())) else {
            continue;
        };
        let row = &mut rows[slot];
        add_feed(row, feed);
        split_feed(
            row,
            feed,
            feed.start() < rule.night_start(calendar, row.day),
        );
        if feed.is_milk() {
            feed_starts[slot].push(feed.start());
        }
    }

    for (slot, starts) in feed_starts.iter_mut().enumerate() {
        let gaps = gaps_between(starts);
        if gaps.is_empty() {
            continue;
        }
        rows[slot].longest_feed_gap_seconds = gaps.iter().copied().max_by(|left, right| {
            left.partial_cmp(right)
                .unwrap_or(core::cmp::Ordering::Equal)
        });
        rows[slot].average_feed_gap_seconds = Some(gaps.iter().sum::<f64>() / gaps.len() as f64);
    }

    for diaper in &dataset.diapers {
        let Some(slot) = index(rule.day_of(calendar, diaper.start)) else {
            continue;
        };
        let row = &mut rows[slot];
        row.diaper_count += 1;
        if diaper.wet {
            row.wet_count += 1;
        }
        if diaper.dirty {
            row.dirty_count += 1;
        }
        if diaper.rash {
            row.rash_count += 1;
        }
    }

    sleep::fill(&mut rows, dataset, calendar, rule, now);

    for session in &dataset.pumps {
        let Some(slot) = index(rule.day_of(calendar, session.start)) else {
            continue;
        };
        rows[slot].pump_count += 1;
        rows[slot].pumped_ml += session.total_ml.unwrap_or(0.0);
    }

    for milestone in &dataset.milestones {
        if let Some(slot) = index(rule.day_of(calendar, milestone.start)) {
            rows[slot].milestone_count += 1;
        }
    }

    for row in &mut rows {
        row.has_data = row.feed_count + row.solids_count + row.diaper_count + row.sleep_count > 0
            || row.sleep_seconds > 0.0;
    }
    rows
}

/// Adds one feed to a day's row.
fn add_feed(row: &mut DaySummary, feed: &FeedEvent) {
    match feed {
        FeedEvent::Bottle {
            amount_ml,
            bottle_type,
            ..
        } => {
            row.feed_count += 1;
            row.bottle_count += 1;
            // A bottle with no amount counts as a feed and contributes no
            // volume: a row the parent forgot to fill in must not drag the
            // daily total down.
            if let Some(amount) = amount_ml {
                row.measured_bottle_count += 1;
                row.total_ml += amount;
                if bottle_type.as_deref() == Some("Breast Milk") {
                    row.breast_milk_ml += amount;
                } else {
                    row.formula_ml += amount;
                }
            }
        }
        FeedEvent::Nursing {
            left_seconds,
            right_seconds,
            ..
        } => {
            row.feed_count += 1;
            row.nursing_count += 1;
            row.nursing_seconds += left_seconds + right_seconds;
            row.left_seconds += left_seconds;
            row.right_seconds += right_seconds;
        }
        FeedEvent::Solids { .. } => row.solids_count += 1,
    }
}

/// Feeding is attributed by its start, matching the daily totals and counts.
fn split_feed(row: &mut DaySummary, feed: &FeedEvent, is_daytime: bool) {
    let (milk, nursing) = if is_daytime {
        (&mut row.day_milk_ml, &mut row.day_nursing_seconds)
    } else {
        (&mut row.night_milk_ml, &mut row.night_nursing_seconds)
    };
    match feed {
        FeedEvent::Bottle { amount_ml, .. } => *milk += amount_ml.unwrap_or(0.0),
        FeedEvent::Nursing {
            left_seconds,
            right_seconds,
            ..
        } => {
            *nursing += left_seconds + right_seconds;
        }
        FeedEvent::Solids { .. } => {}
    }
}

/// The gaps between consecutive times, in order.
fn gaps_between(starts: &mut [f64]) -> Vec<f64> {
    if starts.len() < 2 {
        return Vec::new();
    }
    starts.sort_by(|left, right| {
        left.partial_cmp(right)
            .unwrap_or(core::cmp::Ordering::Equal)
    });
    starts.windows(2).map(|pair| pair[1] - pair[0]).collect()
}

/// The mean of a figure across the days that can fairly be averaged.
///
/// Today is excluded because a half-finished day drags every average down.
/// Days with nothing logged at all are excluded for a sharper reason: a day
/// before the parents started logging is not a day the baby had no wet
/// diapers, and averaging it in shows a frightened parent a number well under
/// the typical range for no reason but a gap in the record.
#[must_use]
pub fn average(rows: &[DaySummary], pick: impl Fn(&DaySummary) -> f64) -> Option<f64> {
    let usable: Vec<&DaySummary> = rows
        .iter()
        .filter(|row| !row.partial && row.has_data)
        .collect();
    if usable.is_empty() {
        return None;
    }
    Some(usable.iter().map(|row| pick(row)).sum::<f64>() / usable.len() as f64)
}

#[cfg(test)]
mod tests {
    use super::super::fixtures::{AFTERNOON, bottle, dataset, diaper, sleep};
    use super::*;

    /// Days from 7am and nights from 8pm: Huckleberry's own assumption, and
    /// what a configuration nobody has finished falls back to.
    fn rule() -> DayRule {
        DayRule::assumed()
    }

    fn calendar() -> Calendar {
        Calendar::new("America/New_York").expect("a real timezone")
    }

    fn nursing(start: f64, left: f64, right: f64) -> super::super::types::FeedEvent {
        super::super::types::FeedEvent::Nursing {
            at: None,
            id: format!("feed-{start}"),
            start,
            left_seconds: left,
            right_seconds: right,
            last_side: Some("right".to_owned()),
            notes: None,
        }
    }

    #[test]
    fn the_rows_are_newest_first_and_today_is_marked_partial() {
        let rows = build(&dataset(), &calendar(), rule(), AFTERNOON, 3);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].day.to_string(), "2025-09-22");
        assert!(rows[0].partial);
        assert!(!rows[1].partial);
    }

    #[test]
    fn bottles_split_into_formula_and_expressed_milk() {
        let mut data = dataset();
        data.feeds = vec![
            bottle(AFTERNOON - 3600.0, 90.0),
            super::super::types::FeedEvent::Bottle {
                at: None,
                id: "b2".to_owned(),
                start: AFTERNOON - 7200.0,
                amount_ml: Some(60.0),
                bottle_type: Some("Breast Milk".to_owned()),
                notes: None,
            },
        ];
        let rows = build(&data, &calendar(), rule(), AFTERNOON, 1);
        assert!((rows[0].total_ml - 150.0).abs() < f64::EPSILON);
        assert!((rows[0].formula_ml - 90.0).abs() < f64::EPSILON);
        assert!((rows[0].breast_milk_ml - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_bottle_with_no_amount_counts_as_a_feed_and_adds_no_volume() {
        let mut data = dataset();
        data.feeds = vec![super::super::types::FeedEvent::Bottle {
            at: None,
            id: "b1".to_owned(),
            start: AFTERNOON - 3600.0,
            amount_ml: None,
            bottle_type: Some("Formula".to_owned()),
            notes: None,
        }];
        let rows = build(&data, &calendar(), rule(), AFTERNOON, 1);
        assert_eq!(rows[0].feed_count, 1);
        assert!(rows[0].total_ml.abs() < f64::EPSILON);
    }

    #[test]
    fn nursing_is_counted_in_seconds_and_by_side() {
        let mut data = dataset();
        data.feeds = vec![nursing(AFTERNOON - 3600.0, 300.0, 120.0)];
        let rows = build(&data, &calendar(), rule(), AFTERNOON, 1);
        assert_eq!(rows[0].nursing_count, 1);
        assert!((rows[0].nursing_seconds - 420.0).abs() < f64::EPSILON);
        assert!((rows[0].left_seconds - 300.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_meal_is_counted_separately_from_milk() {
        let mut data = dataset();
        data.feeds = vec![super::super::types::FeedEvent::Solids {
            at: None,
            id: "s1".to_owned(),
            start: AFTERNOON - 3600.0,
            foods: vec!["Avocado".to_owned()],
            reaction: None,
            notes: None,
        }];
        let rows = build(&data, &calendar(), rule(), AFTERNOON, 1);
        assert_eq!(rows[0].solids_count, 1);
        assert_eq!(rows[0].feed_count, 0, "a meal is not a milk feed");
        assert!(rows[0].has_data, "but it is still data");
    }

    #[test]
    fn feed_gaps_need_two_feeds_to_exist() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 3600.0, 90.0)];
        assert_eq!(
            build(&data, &calendar(), rule(), AFTERNOON, 1)[0].longest_feed_gap_seconds,
            None
        );
    }

    #[test]
    fn feed_gaps_are_measured_between_consecutive_feeds() {
        let mut data = dataset();
        data.feeds = vec![
            bottle(AFTERNOON - 14_400.0, 90.0),
            bottle(AFTERNOON - 3_600.0, 90.0),
            bottle(AFTERNOON - 10_800.0, 90.0),
        ];
        let rows = build(&data, &calendar(), rule(), AFTERNOON, 1);
        // Sorted: -14400, -10800, -3600 gives gaps of 3600 and 7200.
        assert_eq!(rows[0].longest_feed_gap_seconds, Some(7_200.0));
        assert_eq!(rows[0].average_feed_gap_seconds, Some(5_400.0));
    }

    #[test]
    fn a_night_sleep_is_no_longer_cut_in_two_by_midnight() {
        let calendar = calendar();
        let mut data = dataset();
        // 23:00 on the 21st for four hours, read on the 22nd. Under a day that
        // starts at 7am, all of it is still the 21st's night.
        let start = calendar.at("2025-09-21".parse().expect("a date"), 23, 0);
        data.sleep = vec![sleep(start, 4.0 * 3600.0)];
        let rows = build(&data, &calendar, rule(), AFTERNOON, 2);
        let (today, yesterday) = (&rows[0], &rows[1]);
        assert_eq!(yesterday.sleep_count, 1);
        assert_eq!(today.sleep_count, 0);
        assert!(
            (yesterday.sleep_seconds - 4.0 * 3600.0).abs() < 1.0,
            "the whole night belongs to the day it began on: {}",
            yesterday.sleep_seconds
        );
        assert!(
            today.sleep_seconds.abs() < 1.0,
            "and none of it leaks into the next row"
        );
    }

    #[test]
    fn a_sleep_across_the_hour_the_day_starts_is_divided_there() {
        let calendar = calendar();
        let mut data = dataset();
        // 06:00 on the 22nd for two hours, either side of the 7am day start.
        let start = calendar.at("2025-09-22".parse().expect("a date"), 6, 0);
        data.sleep = vec![sleep(start, 2.0 * 3600.0)];
        let rows = build(&data, &calendar, rule(), AFTERNOON, 2);
        assert!(
            (rows[1].sleep_seconds - 3600.0).abs() < 1.0,
            "the hour before the day started belongs to the day before: {}",
            rows[1].sleep_seconds
        );
        assert!((rows[0].sleep_seconds - 3600.0).abs() < 1.0);
        assert_eq!(rows[1].sleep_count, 1, "the count goes where it began");
    }

    #[test]
    fn sleep_is_split_between_night_and_day_by_each_segments_midpoint() {
        let calendar = calendar();
        let mut data = dataset();
        // 05:00 on the 22nd for four hours: two hours before the 7am day
        // start and two after, which is also where the row divides.
        let start = calendar.at("2025-09-22".parse().expect("a date"), 5, 0);
        data.sleep = vec![sleep(start, 4.0 * 3600.0)];
        let rows = build(&data, &calendar, rule(), AFTERNOON, 2);
        // The later segment sits on today's row, midpoint 08:00, which is day.
        assert!((rows[0].day_sleep_seconds - 2.0 * 3600.0).abs() < 1.0);
        assert!(rows[0].night_sleep_seconds.abs() < 1.0);
        // The earlier one sits on yesterday's, midpoint 06:00, which is night.
        assert!((rows[1].night_sleep_seconds - 2.0 * 3600.0).abs() < 1.0);
        assert!(rows[1].day_sleep_seconds.abs() < 1.0);
    }

    #[test]
    fn diapers_are_counted_by_what_was_in_them() {
        let mut data = dataset();
        data.diapers = vec![
            diaper(AFTERNOON - 3600.0, true, false),
            diaper(AFTERNOON - 7200.0, true, true),
        ];
        let rows = build(&data, &calendar(), rule(), AFTERNOON, 1);
        assert_eq!(rows[0].diaper_count, 2);
        assert_eq!(rows[0].wet_count, 2);
        assert_eq!(rows[0].dirty_count, 1);
    }

    #[test]
    fn a_day_with_nothing_logged_is_marked_as_having_no_data() {
        let rows = build(&dataset(), &calendar(), rule(), AFTERNOON, 2);
        assert!(rows.iter().all(|row| !row.has_data));
    }

    #[test]
    fn averages_skip_today_and_skip_days_with_nothing_logged() {
        let mut data = dataset();
        // Six wet diapers yesterday, two so far today, nothing the day before.
        for hour in 0..6 {
            data.diapers.push(diaper(
                AFTERNOON - 86_400.0 - f64::from(hour) * 3600.0,
                true,
                false,
            ));
        }
        for hour in 0..2 {
            data.diapers
                .push(diaper(AFTERNOON - f64::from(hour) * 3600.0, true, false));
        }
        let rows = build(&data, &calendar(), rule(), AFTERNOON, 3);
        let mean = average(&rows, |row| row.wet_count as f64).expect("an average");
        assert!(
            (mean - 6.0).abs() < f64::EPSILON,
            "only yesterday should count, got {mean}"
        );
    }

    #[test]
    fn an_average_over_nothing_is_absent_rather_than_zero() {
        let rows = build(&dataset(), &calendar(), rule(), AFTERNOON, 3);
        assert_eq!(average(&rows, |row| row.wet_count as f64), None);
    }

    #[test]
    fn events_outside_the_window_are_left_out_rather_than_folded_into_the_edge() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 30.0 * 86_400.0, 90.0)];
        let rows = build(&data, &calendar(), rule(), AFTERNOON, 3);
        assert!(rows.iter().all(|row| row.feed_count == 0));
    }
    #[test]
    fn summary_history_starts_at_a_complete_family_day_across_dst() {
        let calendar = calendar();
        let now = calendar.at("2025-11-03".parse().unwrap(), 4, 0);
        let start = history_start(&calendar, DayRule::discrete(6.0, 19.5), now, 7);
        assert!((start - calendar.at("2025-10-26".parse().unwrap(), 6, 0)).abs() < 1.0);
        assert!((now - start - (8.0 * 86400.0 - 3600.0)).abs() < 1.0);
    }
    #[test]
    fn a_truncated_snapshot_day_is_excluded_from_complete_day_averages() {
        let calendar = calendar();
        let mut rows = build(&dataset(), &calendar, rule(), AFTERNOON, 8);
        exclude_incomplete(
            &mut rows,
            &calendar,
            rule(),
            (AFTERNOON - 7.0 * 86400.0, AFTERNOON),
        );
        assert!(rows[0].partial);
        assert!(
            rows[7].partial,
            "the snapshot starts mid-day on the oldest row"
        );
        assert!(rows[1..7].iter().all(|row| !row.partial));
    }
}
