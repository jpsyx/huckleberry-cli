//! Day arithmetic in the family's timezone.
//!
//! A "day" in this tool is a local calendar day in the timezone the family's
//! Huckleberry account uses, not in the timezone of the machine running the
//! command. A parent on a laptop abroad should see the same days their phone
//! shows.
//!
//! Two consequences are load-bearing and are why this module exists rather
//! than a handful of divisions by 86400:
//!
//! - **A day is not always 24 hours.** On the day a clock goes forward it is
//!   23, and back it is 25. Every position on the stripe chart is a fraction
//!   of `end - start` for that particular day, never of a constant.
//! - **A sleep crosses midnight.** Totals that attribute the whole of a sleep
//!   to the day it started lose the small hours off every overnight, which for
//!   a newborn is most of the sleep there is. [`Calendar::split_across_days`]
//!   is what stops that, and its parts always sum to the original duration.

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, Unit, Zoned};

use anyhow::{Context, Result};

/// The calendar a family's days are counted in.
#[derive(Debug, Clone)]
pub struct Calendar {
    name: String,
    zone: TimeZone,
}

impl Calendar {
    /// The calendar for an IANA timezone, for example `America/New_York`.
    pub fn new(name: &str) -> Result<Self> {
        let zone = TimeZone::get(name).with_context(|| format!("unknown timezone `{name}`"))?;
        Ok(Self {
            name: name.to_owned(),
            zone,
        })
    }

    /// UTC, for when nothing has said which timezone to use.
    #[must_use]
    pub fn utc() -> Self {
        Self {
            name: "UTC".to_owned(),
            zone: TimeZone::UTC,
        }
    }

    /// The IANA name this calendar was built from.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// An instant as a local date and time.
    #[must_use]
    pub fn zoned(&self, at: f64) -> Zoned {
        let seconds = at as i64;
        let nanoseconds = ((at - seconds as f64) * 1e9) as i32;
        Timestamp::new(seconds, nanoseconds)
            .unwrap_or(Timestamp::UNIX_EPOCH)
            .to_zoned(self.zone.clone())
    }

    /// The local calendar day an instant falls on.
    #[must_use]
    pub fn day_of(&self, at: f64) -> Date {
        self.zoned(at).date()
    }

    /// What the clock on the wall said at that instant.
    ///
    /// The inverse of [`Self::at`] for a known day, and what a dial starts
    /// standing on: an instant is the truth, but the hour and minute are what
    /// somebody turns.
    #[must_use]
    pub fn time_of_day(&self, at: f64) -> crate::domain::clock::TimeOfDay {
        let zoned = self.zoned(at);
        crate::domain::clock::TimeOfDay::new(zoned.hour(), zoned.minute())
            .unwrap_or(crate::domain::clock::TimeOfDay { hour: 0, minute: 0 })
    }

    /// Local midnight to local midnight, in seconds.
    ///
    /// On a day the clocks change this span is 23 or 25 hours, which is
    /// exactly why callers divide by it rather than by 86400.
    #[must_use]
    pub fn bounds(&self, day: Date) -> (f64, f64) {
        (self.start_of(day), self.start_of(self.offset_day(day, 1)))
    }

    /// Local midnight at the start of a day, in seconds.
    ///
    /// On the day a clock springs forward, midnight itself can be a time that
    /// does not exist in a few zones; jiff resolves that forward to the first
    /// instant that does, which is the same thing a calendar app shows.
    #[must_use]
    pub fn start_of(&self, day: Date) -> f64 {
        self.at(day, 0, 0)
    }

    /// A local wall-clock time on a day, in seconds.
    #[must_use]
    pub fn at(&self, day: Date, hour: i8, minute: i8) -> f64 {
        day.at(hour, minute, 0, 0)
            .to_zoned(self.zone.clone())
            .map_or(0.0, |zoned| zoned.timestamp().as_second() as f64)
    }

    /// A local wall-clock time given as an hour with a fraction: `6.75` is
    /// 6:45am. This is how a Huckleberry profile stores its night boundaries.
    #[must_use]
    pub fn at_hour_fraction(&self, day: Date, hour_fraction: f64) -> f64 {
        let (hour, minute) = split_hour(hour_fraction);
        self.at(day, hour, minute)
    }

    /// A day some number of days away from this one.
    #[must_use]
    pub fn offset_day(&self, day: Date, days: i32) -> Date {
        day.checked_add(jiff::Span::new().days(days)).unwrap_or(day)
    }

    /// The `count` days ending at `from`, newest first.
    #[must_use]
    pub fn recent_days(&self, from: f64, count: usize) -> Vec<Date> {
        let today = self.day_of(from);
        (0..count)
            .map(|back| self.offset_day(today, -(i32::try_from(back).unwrap_or(i32::MAX))))
            .collect()
    }

    /// An interval divided into the days it covers.
    ///
    /// The parts sum to the original duration, clock changes included, which
    /// is the property that makes daily sleep totals add up to the month.
    #[must_use]
    pub fn split_across_days(&self, start: f64, duration: f64) -> Vec<(Date, f64)> {
        if duration <= 0.0 {
            return Vec::new();
        }
        let end = start + duration;
        let mut parts = Vec::new();
        let mut cursor = start;
        // Bounded so that a timestamp from a corrupt row cannot spin forever.
        for _ in 0..400 {
            if cursor >= end {
                break;
            }
            let day = self.day_of(cursor);
            let boundary = self.bounds(day).1;
            let segment_end = end.min(boundary);
            parts.push((day, segment_end - cursor));
            cursor = segment_end;
        }
        parts
    }

    /// Whole days between a date of birth and an instant.
    ///
    /// Calendar days rather than elapsed hours: a baby born yesterday evening
    /// is one day old this morning, which is what a parent and a pediatrician
    /// both mean.
    #[must_use]
    pub fn age_in_days(&self, birthdate: &str, at: f64) -> Option<i64> {
        let born: Date = birthdate.trim().parse().ok()?;
        let today = self.day_of(at);
        let days = born.until((Unit::Day, today)).ok()?.get_days();
        Some(i64::from(days).max(0))
    }
}

/// A length of time, as a person says it: `2h 15m`, or `45m` under an hour,
/// or `90s` under a minute. Never negative.
///
/// This lives beside the arithmetic rather than in the renderer because it
/// makes no terminal decisions: no colour, no width, no alignment. It is the
/// same string on a pipe as on a screen.
#[must_use]
pub fn format_duration(seconds: f64) -> String {
    let total = seconds.max(0.0) as i64;
    let minutes = total / 60;
    let hours = minutes / 60;
    if hours > 0 {
        format!("{hours}h {}m", minutes % 60)
    } else if minutes > 0 {
        format!("{minutes}m")
    } else {
        format!("{total}s")
    }
}

/// How long ago something was: `2h 14m ago`.
///
/// Clamped to `just now`, so a clock a few seconds out of step with
/// Huckleberry's never reads as a feed in the future.
#[must_use]
pub fn format_ago(then: f64, now: f64) -> String {
    let seconds = (now - then).max(0.0) as i64;
    if seconds < 60 {
        return "just now".to_owned();
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m ago");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours}h {}m ago", minutes % 60);
    }
    format!("{}d {}h ago", hours / 24, hours % 24)
}

/// An hour with a fraction, as hours and minutes. `6.75` is 6:45.
#[must_use]
pub fn split_hour(hour_fraction: f64) -> (i8, i8) {
    let hour = hour_fraction.floor();
    let minute = ((hour_fraction - hour) * 60.0).round();
    (hour as i8, minute as i8)
}

#[cfg(test)]
mod days {
    use super::*;

    /// The hour and minute a dial starts standing on.
    #[test]
    fn an_instant_reads_back_as_the_clock_on_the_wall() {
        let calendar = Calendar::new("America/New_York").expect("a real timezone");
        let afternoon = calendar.at("2025-09-22".parse().expect("a date"), 13, 44);
        let time = calendar.time_of_day(afternoon);
        assert_eq!((time.hour, time.minute), (13, 44));
        let round_trip = calendar.at(calendar.day_of(afternoon), time.hour, time.minute);
        assert!(
            (round_trip - afternoon).abs() < f64::EPSILON,
            "and goes back to the instant it came from"
        );
    }

    fn new_york() -> Calendar {
        Calendar::new("America/New_York").expect("a real timezone")
    }

    fn date(text: &str) -> Date {
        text.parse().expect("a date")
    }

    #[test]
    fn an_instant_lands_on_the_local_day_not_the_utc_one() {
        // 2025-09-23T02:00:00Z is still the 22nd in New York.
        assert_eq!(new_york().day_of(1_758_592_800.0).to_string(), "2025-09-22");
    }

    #[test]
    fn an_ordinary_day_is_twenty_four_hours_long() {
        let (start, end) = new_york().bounds(date("2025-09-22"));
        assert!((end - start - 24.0 * 3600.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_spring_forward_day_is_twenty_three_hours_long() {
        let (start, end) = new_york().bounds(date("2025-03-09"));
        assert!((end - start - 23.0 * 3600.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_fall_back_day_is_twenty_five_hours_long() {
        let (start, end) = new_york().bounds(date("2025-11-02"));
        assert!((end - start - 25.0 * 3600.0).abs() < f64::EPSILON);
    }

    #[test]
    fn recent_days_walk_backwards_from_today() {
        let days = new_york().recent_days(1_758_592_800.0, 3);
        let names: Vec<String> = days.iter().map(ToString::to_string).collect();
        assert_eq!(names, vec!["2025-09-22", "2025-09-21", "2025-09-20"]);
    }

    #[test]
    fn a_month_boundary_walks_backwards_correctly() {
        // 2025-10-01T12:00:00Z.
        let days = new_york().recent_days(1_759_320_000.0, 2);
        let names: Vec<String> = days.iter().map(ToString::to_string).collect();
        assert_eq!(names, vec!["2025-10-01", "2025-09-30"]);
    }
}

#[cfg(test)]
mod splitting {
    use super::*;

    fn new_york() -> Calendar {
        Calendar::new("America/New_York").expect("a real timezone")
    }

    #[test]
    fn a_sleep_inside_one_day_is_one_part() {
        let calendar = new_york();
        let start = calendar.at("2025-09-22".parse().expect("a date"), 13, 0);
        let parts = calendar.split_across_days(start, 3600.0);
        assert_eq!(parts.len(), 1);
        assert!((parts[0].1 - 3600.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_sleep_over_midnight_is_split_between_the_two_days() {
        let calendar = new_york();
        let start = calendar.at("2025-09-22".parse().expect("a date"), 23, 46);
        let parts = calendar.split_across_days(start, 8_100.0);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].0.to_string(), "2025-09-22");
        assert_eq!(parts[1].0.to_string(), "2025-09-23");
        assert!(
            (parts[0].1 - 840.0).abs() < 0.001,
            "14 minutes before midnight"
        );
    }

    #[test]
    fn the_parts_always_sum_to_the_whole() {
        let calendar = new_york();
        for (day, hour, duration) in [
            ("2025-09-22", 23, 8_100.0),
            ("2025-03-08", 22, 40_000.0),
            ("2025-11-01", 22, 40_000.0),
            ("2025-09-20", 0, 300_000.0),
        ] {
            let start = calendar.at(day.parse().expect("a date"), hour, 0);
            let total: f64 = calendar
                .split_across_days(start, duration)
                .iter()
                .map(|(_, seconds)| seconds)
                .sum();
            assert!(
                (total - duration).abs() < 0.001,
                "{day} {hour}:00 for {duration}s summed to {total}"
            );
        }
    }

    #[test]
    fn an_interval_with_no_duration_covers_no_days() {
        assert!(
            new_york()
                .split_across_days(1_758_592_800.0, 0.0)
                .is_empty()
        );
        assert!(
            new_york()
                .split_across_days(1_758_592_800.0, -5.0)
                .is_empty()
        );
    }
}

#[cfg(test)]
mod human_readable {
    use super::*;

    #[test]
    fn a_duration_over_an_hour_carries_both_parts() {
        assert_eq!(format_duration(8_100.0), "2h 15m");
    }

    #[test]
    fn a_duration_under_an_hour_is_minutes_alone() {
        assert_eq!(format_duration(2_700.0), "45m");
    }

    #[test]
    fn a_duration_under_a_minute_is_seconds_rather_than_a_bare_zero() {
        assert_eq!(format_duration(42.0), "42s");
        assert_eq!(format_duration(0.0), "0s");
    }

    #[test]
    fn a_negative_duration_reads_as_nothing_rather_than_as_a_minus() {
        assert_eq!(format_duration(-500.0), "0s");
    }

    #[test]
    fn a_whole_number_of_hours_still_says_its_minutes() {
        assert_eq!(format_duration(7_200.0), "2h 0m");
    }

    #[test]
    fn something_that_just_happened_says_so() {
        assert_eq!(format_ago(1_000.0, 1_030.0), "just now");
    }

    #[test]
    fn a_clock_a_moment_ahead_never_reads_as_the_future() {
        assert_eq!(format_ago(1_030.0, 1_000.0), "just now");
    }

    #[test]
    fn hours_and_days_are_both_spelled_out() {
        assert_eq!(format_ago(0.0, 8_040.0), "2h 14m ago");
        assert_eq!(format_ago(0.0, 3_600.0 * 30.0), "1d 6h ago");
        assert_eq!(format_ago(0.0, 900.0), "15m ago");
    }
}

#[cfg(test)]
mod hours_and_ages {
    use super::*;

    #[test]
    fn a_fractional_hour_becomes_hours_and_minutes() {
        assert_eq!(split_hour(6.75), (6, 45));
        assert_eq!(split_hour(20.0), (20, 0));
        assert_eq!(split_hour(19.5), (19, 30));
    }

    #[test]
    fn a_profile_hour_places_a_time_on_a_day() {
        let calendar = Calendar::new("America/New_York").expect("a real timezone");
        let day = "2025-09-22".parse().expect("a date");
        let quarter_to_seven = calendar.at_hour_fraction(day, 6.75);
        assert_eq!(calendar.zoned(quarter_to_seven).hour(), 6);
        assert_eq!(calendar.zoned(quarter_to_seven).minute(), 45);
    }

    #[test]
    fn age_is_counted_in_calendar_days() {
        let calendar = Calendar::new("America/New_York").expect("a real timezone");
        // 2025-09-22T18:00:00Z, which is the 22nd locally.
        assert_eq!(
            calendar.age_in_days("2025-09-01", 1_758_564_000.0),
            Some(21)
        );
    }

    #[test]
    fn a_baby_born_today_is_nought_days_old() {
        let calendar = Calendar::new("America/New_York").expect("a real timezone");
        assert_eq!(calendar.age_in_days("2025-09-22", 1_758_564_000.0), Some(0));
    }

    #[test]
    fn a_birthdate_in_the_future_reads_as_nought_rather_than_negative() {
        let calendar = Calendar::new("America/New_York").expect("a real timezone");
        assert_eq!(calendar.age_in_days("2099-01-01", 1_758_564_000.0), Some(0));
    }

    #[test]
    fn a_birthdate_that_is_not_a_date_has_no_age() {
        let calendar = Calendar::new("America/New_York").expect("a real timezone");
        assert_eq!(
            calendar.age_in_days("sometime in September", 1_758_564_000.0),
            None
        );
    }

    #[test]
    fn a_timezone_the_database_does_not_know_is_refused() {
        assert!(Calendar::new("Mars/Olympus_Mons").is_err());
    }
}
