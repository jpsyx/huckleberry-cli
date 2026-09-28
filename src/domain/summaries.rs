//! One row per day: the numbers a pediatrician asks for.
//!
//! One asymmetry is worth knowing about, because it looks like a bug until you
//! see why. Sleep **seconds** are split across midnight, so a sleep beginning
//! at 23:46 contributes to both days. Sleep **counts** and the longest stretch
//! are attributed to the day the sleep began. Splitting the counts would turn
//! one overnight into two sleeps; not splitting the seconds would quietly lose
//! the small hours off every night, which for a newborn is most of the sleep
//! there is.

use jiff::civil::Date;

use super::time::Calendar;
use super::types::{Dataset, FeedEvent};

/// Everything one day amounts to.
#[derive(Debug, Clone, PartialEq)]
pub struct DaySummary {
    /// Which day.
    pub day: Date,
    /// Whether the day is still going, which is why it is left out of averages.
    pub partial: bool,
    /// Milk feeds: bottles and nursing sessions. Meals are counted separately.
    pub feed_count: usize,
    /// How many were bottles.
    pub bottle_count: usize,
    /// How many were nursing sessions.
    pub nursing_count: usize,
    /// How many were meals.
    pub solids_count: usize,
    /// Millilitres taken by bottle.
    pub total_ml: f64,
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
    const fn empty(day: Date, partial: bool) -> Self {
        Self {
            day,
            partial,
            feed_count: 0,
            bottle_count: 0,
            nursing_count: 0,
            solids_count: 0,
            total_ml: 0.0,
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
            pump_count: 0,
            pumped_ml: 0.0,
            milestone_count: 0,
            has_data: false,
        }
    }
}

/// One row per day, newest first, for the last `days` days.
#[must_use]
pub fn build(dataset: &Dataset, calendar: &Calendar, now: f64, days: usize) -> Vec<DaySummary> {
    let today = calendar.day_of(now);
    let wanted = calendar.recent_days(now, days);
    let mut rows: Vec<DaySummary> = wanted
        .iter()
        .map(|day| DaySummary::empty(*day, *day == today))
        .collect();
    let index = |day: Date| wanted.iter().position(|candidate| *candidate == day);

    let mut feed_starts: Vec<Vec<f64>> = vec![Vec::new(); rows.len()];

    for feed in &dataset.feeds {
        let Some(slot) = index(calendar.day_of(feed.start())) else {
            continue;
        };
        add_feed(&mut rows[slot], feed);
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
        let Some(slot) = index(calendar.day_of(diaper.start)) else {
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

    for sleep in &dataset.sleep {
        // Counts and the longest stretch belong to the day the sleep began.
        if let Some(slot) = index(calendar.day_of(sleep.start)) {
            let row = &mut rows[slot];
            row.sleep_count += 1;
            row.longest_sleep_seconds = row.longest_sleep_seconds.max(sleep.duration);
        }
        // Seconds are apportioned to whichever days the sleep actually covered.
        let mut elapsed = 0.0;
        for (day, seconds) in calendar.split_across_days(sleep.start, sleep.duration) {
            let segment_start = sleep.start + elapsed;
            elapsed += seconds;
            let Some(slot) = index(day) else {
                continue;
            };
            let row = &mut rows[slot];
            row.sleep_seconds += seconds;
            // Classified by the midpoint of each segment rather than of the
            // whole sleep, so a sleep straddling the morning cutoff divides.
            let midpoint = segment_start + seconds / 2.0;
            let position = super::now::night_position(
                calendar,
                midpoint,
                dataset.child.night_start_hour,
                dataset.child.morning_cutoff_hour,
            );
            if position.inside {
                row.night_sleep_seconds += seconds;
            } else {
                row.day_sleep_seconds += seconds;
            }
        }
    }

    for session in &dataset.pumps {
        let Some(slot) = index(calendar.day_of(session.start)) else {
            continue;
        };
        rows[slot].pump_count += 1;
        rows[slot].pumped_ml += session.total_ml.unwrap_or(0.0);
    }

    for milestone in &dataset.milestones {
        if let Some(slot) = index(calendar.day_of(milestone.start)) {
            rows[slot].milestone_count += 1;
        }
    }

    for row in &mut rows {
        row.has_data = row.feed_count + row.solids_count + row.diaper_count + row.sleep_count > 0;
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
        let rows = build(&dataset(), &calendar(), AFTERNOON, 3);
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
        let rows = build(&data, &calendar(), AFTERNOON, 1);
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
        let rows = build(&data, &calendar(), AFTERNOON, 1);
        assert_eq!(rows[0].feed_count, 1);
        assert!(rows[0].total_ml.abs() < f64::EPSILON);
    }

    #[test]
    fn nursing_is_counted_in_seconds_and_by_side() {
        let mut data = dataset();
        data.feeds = vec![nursing(AFTERNOON - 3600.0, 300.0, 120.0)];
        let rows = build(&data, &calendar(), AFTERNOON, 1);
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
        let rows = build(&data, &calendar(), AFTERNOON, 1);
        assert_eq!(rows[0].solids_count, 1);
        assert_eq!(rows[0].feed_count, 0, "a meal is not a milk feed");
        assert!(rows[0].has_data, "but it is still data");
    }

    #[test]
    fn feed_gaps_need_two_feeds_to_exist() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 3600.0, 90.0)];
        assert_eq!(
            build(&data, &calendar(), AFTERNOON, 1)[0].longest_feed_gap_seconds,
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
        let rows = build(&data, &calendar(), AFTERNOON, 1);
        // Sorted: -14400, -10800, -3600 gives gaps of 3600 and 7200.
        assert_eq!(rows[0].longest_feed_gap_seconds, Some(7_200.0));
        assert_eq!(rows[0].average_feed_gap_seconds, Some(5_400.0));
    }

    #[test]
    fn a_sleep_over_midnight_gives_seconds_to_both_days_and_a_count_to_one() {
        let calendar = calendar();
        let mut data = dataset();
        // 23:00 on the 21st for four hours, read on the 22nd.
        let start = calendar.at("2025-09-21".parse().expect("a date"), 23, 0);
        data.sleep = vec![sleep(start, 4.0 * 3600.0)];
        let rows = build(&data, &calendar, AFTERNOON, 2);
        let (today, yesterday) = (&rows[0], &rows[1]);
        assert_eq!(
            yesterday.sleep_count, 1,
            "the count belongs to the start day"
        );
        assert_eq!(today.sleep_count, 0);
        assert!((yesterday.sleep_seconds - 3600.0).abs() < 1.0);
        assert!((today.sleep_seconds - 3.0 * 3600.0).abs() < 1.0);
    }

    #[test]
    fn sleep_is_split_between_night_and_day_by_each_segments_midpoint() {
        let calendar = calendar();
        let mut data = dataset();
        // 05:00 on the 22nd for four hours: two hours before the 7am cutoff
        // and two after.
        let start = calendar.at("2025-09-22".parse().expect("a date"), 5, 0);
        data.sleep = vec![sleep(start, 4.0 * 3600.0)];
        let rows = build(&data, &calendar, AFTERNOON, 1);
        // One segment, classified by its midpoint at 07:00, which is day.
        assert!((rows[0].day_sleep_seconds - 4.0 * 3600.0).abs() < 1.0);
        assert!(rows[0].night_sleep_seconds.abs() < 1.0);
    }

    #[test]
    fn diapers_are_counted_by_what_was_in_them() {
        let mut data = dataset();
        data.diapers = vec![
            diaper(AFTERNOON - 3600.0, true, false),
            diaper(AFTERNOON - 7200.0, true, true),
        ];
        let rows = build(&data, &calendar(), AFTERNOON, 1);
        assert_eq!(rows[0].diaper_count, 2);
        assert_eq!(rows[0].wet_count, 2);
        assert_eq!(rows[0].dirty_count, 1);
    }

    #[test]
    fn a_day_with_nothing_logged_is_marked_as_having_no_data() {
        let rows = build(&dataset(), &calendar(), AFTERNOON, 2);
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
        let rows = build(&data, &calendar(), AFTERNOON, 3);
        let mean = average(&rows, |row| row.wet_count as f64).expect("an average");
        assert!(
            (mean - 6.0).abs() < f64::EPSILON,
            "only yesterday should count, got {mean}"
        );
    }

    #[test]
    fn an_average_over_nothing_is_absent_rather_than_zero() {
        let rows = build(&dataset(), &calendar(), AFTERNOON, 3);
        assert_eq!(average(&rows, |row| row.wet_count as f64), None);
    }

    #[test]
    fn events_outside_the_window_are_left_out_rather_than_folded_into_the_edge() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 30.0 * 86_400.0, 90.0)];
        let rows = build(&data, &calendar(), AFTERNOON, 3);
        assert!(rows.iter().all(|row| row.feed_count == 0));
    }
}
