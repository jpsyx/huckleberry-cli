//! The geometry of the 24-hour stripe chart.
//!
//! One row per day, and everything on it positioned as a fraction of that
//! day's own length rather than of a constant 86400. A 23-hour or 25-hour day
//! still maps cleanly onto `0.0..=1.0`, which is the whole reason the
//! positions are computed here instead of in the renderer.
//!
//! This is the one picture the dashboard's user said they would actually open:
//! where sleep lands, at a glance, over a week.

use jiff::civil::Date;

use super::time::Calendar;
use super::types::Dataset;

/// A block of sleep on one day's row.
#[derive(Debug, Clone, PartialEq)]
pub struct SleepBlock {
    /// Where it starts, as a fraction of the day.
    pub from: f64,
    /// Where it ends.
    pub to: f64,
    /// Whether it began before this day did.
    pub continued: bool,
}

/// A feed on one day's row.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedTick {
    /// Where it falls, as a fraction of the day.
    pub at: f64,
    /// Millilitres, for a bottle.
    pub amount_ml: Option<f64>,
    /// A few words for a tooltip or a legend.
    pub label: String,
}

/// One day's row of the chart.
#[derive(Debug, Clone, PartialEq)]
pub struct StripeRow {
    /// Which day.
    pub day: Date,
    /// Where sleep was.
    pub sleep_blocks: Vec<SleepBlock>,
    /// Where feeds were.
    pub feed_ticks: Vec<FeedTick>,
    /// Where diapers were.
    pub diaper_ticks: Vec<f64>,
    /// The morning cutoff: everything before this is still night.
    pub night_until: f64,
    /// The night start: everything after this is night again.
    pub night_from: f64,
    /// Whether the day is still going.
    pub partial: bool,
    /// How much of the day has happened, for shading the rest of today.
    pub elapsed: f64,
}

/// One row per day, newest first.
#[must_use]
pub fn build(dataset: &Dataset, calendar: &Calendar, now: f64, days: usize) -> Vec<StripeRow> {
    let today = calendar.day_of(now);
    calendar
        .recent_days(now, days)
        .into_iter()
        .map(|day| row(dataset, calendar, now, day, today))
        .collect()
}

fn row(dataset: &Dataset, calendar: &Calendar, now: f64, day: Date, today: Date) -> StripeRow {
    let (start, end) = calendar.bounds(day);
    let span = (end - start).max(1.0);
    let position = |at: f64| ((at - start) / span).clamp(0.0, 1.0);

    let sleep_blocks = dataset
        .sleep
        .iter()
        .filter(|sleep| sleep.end() > start && sleep.start < end)
        .map(|sleep| SleepBlock {
            from: position(sleep.start),
            to: position(sleep.end()),
            continued: sleep.start < start,
        })
        .collect();

    let feed_ticks = dataset
        .feeds
        .iter()
        .filter(|feed| feed.start() >= start && feed.start() < end)
        .map(|feed| FeedTick {
            at: position(feed.start()),
            amount_ml: feed.millilitres(),
            label: feed_label(feed),
        })
        .collect();

    let diaper_ticks = dataset
        .diapers
        .iter()
        .filter(|diaper| diaper.start >= start && diaper.start < end)
        .map(|diaper| position(diaper.start))
        .collect();

    StripeRow {
        day,
        sleep_blocks,
        feed_ticks,
        diaper_ticks,
        night_until: position(calendar.at_hour_fraction(day, dataset.child.morning_cutoff_hour)),
        night_from: position(calendar.at_hour_fraction(day, dataset.child.night_start_hour)),
        partial: day == today,
        elapsed: if day == today { position(now) } else { 1.0 },
    }
}

fn feed_label(feed: &super::types::FeedEvent) -> String {
    match feed {
        super::types::FeedEvent::Bottle {
            amount_ml,
            bottle_type,
            ..
        } => {
            let amount = amount_ml.map_or_else(|| "?".to_owned(), |value| format!("{value:.0}"));
            let kind = bottle_type.as_deref().unwrap_or("milk");
            format!("{amount} ml {kind}")
        }
        super::types::FeedEvent::Nursing {
            left_seconds,
            right_seconds,
            ..
        } => format!("nursed {:.0}m", (left_seconds + right_seconds) / 60.0),
        super::types::FeedEvent::Solids { foods, .. } => {
            if foods.is_empty() {
                "solids".to_owned()
            } else {
                foods.join(", ")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::fixtures::{AFTERNOON, bottle, dataset, diaper, sleep};
    use super::*;

    fn calendar() -> Calendar {
        Calendar::new("America/New_York").expect("a real timezone")
    }

    fn date(text: &str) -> Date {
        text.parse().expect("a date")
    }

    #[test]
    fn there_is_one_row_per_day_newest_first() {
        let rows = build(&dataset(), &calendar(), AFTERNOON, 7);
        assert_eq!(rows.len(), 7);
        assert_eq!(rows[0].day.to_string(), "2025-09-22");
        assert!(rows[0].partial);
        assert!(!rows[6].partial);
    }

    #[test]
    fn noon_is_halfway_along_an_ordinary_day() {
        let calendar = calendar();
        let mut data = dataset();
        data.feeds = vec![bottle(calendar.at(date("2025-09-21"), 12, 0), 90.0)];
        let rows = build(&data, &calendar, AFTERNOON, 2);
        assert!((rows[1].feed_ticks[0].at - 0.5).abs() < 0.001);
    }

    #[test]
    fn a_position_is_a_fraction_of_that_days_own_length() {
        let calendar = calendar();
        let mut data = dataset();
        // Noon on a 23-hour day is not halfway: the missing hour is behind it.
        data.feeds = vec![bottle(calendar.at(date("2025-03-09"), 12, 0), 90.0)];
        // Read on 2025-03-09 at 18:00Z, which is 2pm local.
        let rows = build(&data, &calendar, 1_741_543_200.0, 1);
        let at = rows[0].feed_ticks[0].at;
        assert!(
            (at - 11.0 / 23.0).abs() < 0.001,
            "noon on a 23-hour day should be at {}, got {at}",
            11.0 / 23.0
        );
    }

    #[test]
    fn a_sleep_over_midnight_appears_on_both_rows_and_the_later_one_says_it_continued() {
        let calendar = calendar();
        let mut data = dataset();
        data.sleep = vec![sleep(calendar.at(date("2025-09-21"), 23, 0), 4.0 * 3600.0)];
        let rows = build(&data, &calendar, AFTERNOON, 2);
        let (today, yesterday) = (&rows[0], &rows[1]);
        assert_eq!(yesterday.sleep_blocks.len(), 1);
        assert!(!yesterday.sleep_blocks[0].continued);
        assert_eq!(today.sleep_blocks.len(), 1);
        assert!(today.sleep_blocks[0].continued);
        assert!(
            today.sleep_blocks[0].from.abs() < f64::EPSILON,
            "it starts at the top"
        );
    }

    #[test]
    fn a_block_is_clamped_to_the_day_it_is_drawn_on() {
        let calendar = calendar();
        let mut data = dataset();
        data.sleep = vec![sleep(
            calendar.at(date("2025-09-20"), 12, 0),
            3.0 * 86_400.0,
        )];
        let rows = build(&data, &calendar, AFTERNOON, 2);
        for row in &rows {
            for block in &row.sleep_blocks {
                assert!((0.0..=1.0).contains(&block.from), "{block:?}");
                assert!((0.0..=1.0).contains(&block.to), "{block:?}");
            }
        }
    }

    #[test]
    fn the_night_band_runs_from_the_evening_to_the_morning_cutoff() {
        let rows = build(&dataset(), &calendar(), AFTERNOON, 1);
        // 7am of 24 hours, and 8pm of 24 hours.
        assert!((rows[0].night_until - 7.0 / 24.0).abs() < 0.001);
        assert!((rows[0].night_from - 20.0 / 24.0).abs() < 0.001);
    }

    #[test]
    fn a_finished_day_is_fully_elapsed_and_today_is_not() {
        let rows = build(&dataset(), &calendar(), AFTERNOON, 2);
        assert!((rows[1].elapsed - 1.0).abs() < f64::EPSILON);
        assert!((rows[0].elapsed - 14.0 / 24.0).abs() < 0.001, "2pm local");
    }

    #[test]
    fn diapers_get_their_own_ticks() {
        let calendar = calendar();
        let mut data = dataset();
        data.diapers = vec![diaper(calendar.at(date("2025-09-21"), 6, 0), true, false)];
        let rows = build(&data, &calendar, AFTERNOON, 2);
        assert_eq!(rows[1].diaper_ticks.len(), 1);
        assert!((rows[1].diaper_ticks[0] - 6.0 / 24.0).abs() < 0.001);
    }

    #[test]
    fn a_bottle_with_no_amount_is_labelled_with_a_question_mark_not_a_zero() {
        let calendar = calendar();
        let mut data = dataset();
        data.feeds = vec![super::super::types::FeedEvent::Bottle {
            at: None,
            id: "b1".to_owned(),
            start: calendar.at(date("2025-09-21"), 12, 0),
            amount_ml: None,
            bottle_type: Some("Formula".to_owned()),
            notes: None,
        }];
        let rows = build(&data, &calendar, AFTERNOON, 2);
        assert_eq!(rows[1].feed_ticks[0].label, "? ml Formula");
    }
}
