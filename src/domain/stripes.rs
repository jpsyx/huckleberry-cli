//! The geometry of the 24-hour stripe chart.
//!
//! One row per day, and everything on it positioned as a fraction of that
//! row's own length rather than of a constant 86400. A 23-hour or 25-hour day
//! still maps cleanly onto `0.0..=1.0`, which is the whole reason the
//! positions are computed here instead of in the renderer.
//!
//! A row **opens at the previous day's end**, not at midnight. This is a chart
//! about where sleep lands, and midnight falls in the middle of the longest
//! sleep there is: cutting the row there puts half a night at the right-hand
//! end of one row and half at the left-hand end of the next, which is the one
//! thing this picture exists not to do. Opening on the night makes a row read
//! as "the night leading into this day, and then this day", which is how a
//! night gets talked about anyway.
//!
//! This is the one picture the dashboard's user said they would actually open:
//! where sleep lands, at a glance, over a week.

use jiff::civil::Date;

use super::time::Calendar;
use super::today::DayRule;
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
    /// Where the night that opened this row gives way to the day.
    pub night_until: f64,
    /// Where night begins again, which is the end of the row unless the row
    /// is drawn some other way.
    pub night_from: f64,
    /// Whether the row is still being lived.
    pub partial: bool,
    /// How much of the row has happened, for shading the rest of today.
    pub elapsed: f64,
    /// The hour the row opens at, so the ruler above it can be labelled in
    /// wall-clock time rather than in hours since the row began.
    pub starts_at_hour: f64,
}

/// One row per day, newest first.
#[must_use]
pub fn build(
    dataset: &Dataset,
    calendar: &Calendar,
    rule: DayRule,
    now: f64,
    days: usize,
) -> Vec<StripeRow> {
    let today = rule.stripe_day_of(calendar, now);
    rule.recent_stripe_days(calendar, now, days)
        .into_iter()
        .map(|day| row(dataset, calendar, rule, now, day, today))
        .collect()
}

fn row(
    dataset: &Dataset,
    calendar: &Calendar,
    rule: DayRule,
    now: f64,
    day: Date,
    today: Date,
) -> StripeRow {
    let (start, end) = rule.stripe_bounds(calendar, day);
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
        // The row opens inside the night, so the night is one block at the
        // left and there is none left over at the right.
        night_until: position(calendar.at_hour_fraction(day, rule.day_start_hour)),
        night_from: 1.0,
        partial: day == today,
        elapsed: if day == today { position(now) } else { 1.0 },
        starts_at_hour: rule.day_end_hour,
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

    /// Days from 7am and nights from 8pm: Huckleberry's own assumption, and
    /// what a configuration nobody has finished falls back to.
    fn rule() -> DayRule {
        DayRule::assumed()
    }

    fn calendar() -> Calendar {
        Calendar::new("America/New_York").expect("a real timezone")
    }

    fn date(text: &str) -> Date {
        text.parse().expect("a date")
    }

    #[test]
    fn there_is_one_row_per_day_newest_first() {
        let rows = build(&dataset(), &calendar(), rule(), AFTERNOON, 7);
        assert_eq!(rows.len(), 7);
        assert_eq!(rows[0].day.to_string(), "2025-09-22");
        assert!(rows[0].partial);
        assert!(!rows[6].partial);
    }

    #[test]
    fn a_row_opens_at_the_previous_days_end_so_noon_sits_two_thirds_along() {
        let calendar = calendar();
        let mut data = dataset();
        data.feeds = vec![bottle(calendar.at(date("2025-09-21"), 12, 0), 90.0)];
        let rows = build(&data, &calendar, rule(), AFTERNOON, 2);
        // The row opens at 8pm, so noon is sixteen of its twenty-four hours in.
        assert!(
            (rows[1].feed_ticks[0].at - 16.0 / 24.0).abs() < 0.001,
            "{}",
            rows[1].feed_ticks[0].at
        );
        assert!((rows[1].starts_at_hour - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn midnight_is_no_longer_an_edge_but_a_point_inside_the_row() {
        let calendar = calendar();
        let mut data = dataset();
        data.feeds = vec![bottle(calendar.at(date("2025-09-21"), 0, 0), 90.0)];
        let rows = build(&data, &calendar, rule(), AFTERNOON, 2);
        assert!(
            (rows[1].feed_ticks[0].at - 4.0 / 24.0).abs() < 0.001,
            "midnight is four hours after an 8pm row opens: {}",
            rows[1].feed_ticks[0].at
        );
    }

    #[test]
    fn a_position_is_a_fraction_of_that_rows_own_length() {
        let calendar = calendar();
        let mut data = dataset();
        // The row runs 8pm on the 8th to 8pm on the 9th, and the clocks go
        // forward inside it, so it is twenty-three hours long. Noon is fifteen
        // of them in rather than sixteen: the missing hour is behind it.
        data.feeds = vec![bottle(calendar.at(date("2025-03-09"), 12, 0), 90.0)];
        // Read on 2025-03-09 at 18:00Z, which is 2pm local.
        let rows = build(&data, &calendar, rule(), 1_741_543_200.0, 1);
        let at = rows[0].feed_ticks[0].at;
        assert!(
            (at - 15.0 / 23.0).abs() < 0.001,
            "noon on a 23-hour row should be at {}, got {at}",
            15.0 / 23.0
        );
    }

    #[test]
    fn a_night_lands_whole_on_one_row_which_is_the_point_of_the_chart() {
        let calendar = calendar();
        let mut data = dataset();
        // 11pm to 3am: the longest sleep there is, and the one midnight used
        // to cut in half.
        data.sleep = vec![sleep(calendar.at(date("2025-09-21"), 23, 0), 4.0 * 3600.0)];
        let rows = build(&data, &calendar, rule(), AFTERNOON, 2);
        let (today, yesterday) = (&rows[0], &rows[1]);
        assert_eq!(today.sleep_blocks.len(), 1, "one block, not two");
        assert!(
            !today.sleep_blocks[0].continued,
            "and it began on this row rather than before it"
        );
        assert!(
            yesterday.sleep_blocks.is_empty(),
            "nothing is left over on the row before"
        );
        let block = &today.sleep_blocks[0];
        assert!((block.from - 3.0 / 24.0).abs() < 0.001, "{}", block.from);
        assert!((block.to - 7.0 / 24.0).abs() < 0.001, "{}", block.to);
    }

    #[test]
    fn a_sleep_that_began_before_the_row_opened_still_says_it_continued() {
        let calendar = calendar();
        let mut data = dataset();
        // 7pm to 9pm, straddling the 8pm the row opens at.
        data.sleep = vec![sleep(calendar.at(date("2025-09-21"), 19, 0), 2.0 * 3600.0)];
        let rows = build(&data, &calendar, rule(), AFTERNOON, 2);
        assert!(rows[0].sleep_blocks[0].continued);
        assert!(rows[0].sleep_blocks[0].from.abs() < f64::EPSILON);
    }

    #[test]
    fn a_block_is_clamped_to_the_day_it_is_drawn_on() {
        let calendar = calendar();
        let mut data = dataset();
        data.sleep = vec![sleep(
            calendar.at(date("2025-09-20"), 12, 0),
            3.0 * 86_400.0,
        )];
        let rows = build(&data, &calendar, rule(), AFTERNOON, 2);
        for row in &rows {
            for block in &row.sleep_blocks {
                assert!((0.0..=1.0).contains(&block.from), "{block:?}");
                assert!((0.0..=1.0).contains(&block.to), "{block:?}");
            }
        }
    }

    #[test]
    fn the_night_is_one_band_at_the_start_of_the_row_rather_than_two_at_its_ends() {
        let rows = build(&dataset(), &calendar(), rule(), AFTERNOON, 1);
        // The row opens at 8pm inside the night, and the night gives way to
        // the day at 7am, eleven of the row's twenty-four hours in.
        assert!((rows[0].night_until - 11.0 / 24.0).abs() < 0.001);
        assert!(
            (rows[0].night_from - 1.0).abs() < f64::EPSILON,
            "the row ends where the next night begins, so none is left over"
        );
    }

    #[test]
    fn a_finished_day_is_fully_elapsed_and_today_is_not() {
        let rows = build(&dataset(), &calendar(), rule(), AFTERNOON, 2);
        assert!((rows[1].elapsed - 1.0).abs() < f64::EPSILON);
        // 2pm local is eighteen hours into a row that opened at 8pm.
        assert!((rows[0].elapsed - 18.0 / 24.0).abs() < 0.001, "2pm local");
    }

    #[test]
    fn diapers_get_their_own_ticks() {
        let calendar = calendar();
        let mut data = dataset();
        data.diapers = vec![diaper(calendar.at(date("2025-09-21"), 6, 0), true, false)];
        let rows = build(&data, &calendar, rule(), AFTERNOON, 2);
        assert_eq!(rows[1].diaper_ticks.len(), 1);
        // 6am is ten hours into a row that opened at 8pm the evening before.
        assert!((rows[1].diaper_ticks[0] - 10.0 / 24.0).abs() < 0.001);
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
        let rows = build(&data, &calendar, rule(), AFTERNOON, 2);
        assert_eq!(rows[1].feed_ticks[0].label, "? ml Formula");
    }
}
