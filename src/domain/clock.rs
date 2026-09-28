//! Reading a time somebody typed.
//!
//! A person logging a sleep after the fact types what they would say: `3:57am`,
//! `9pm`, `0357`, `21:30`, `7`. This module is the one place that turns any of
//! those into a time of day, and then into an instant, and it does both
//! without reading the clock: `now` is an argument, as everywhere in
//! `domain`.
//!
//! Two rules make a bare time mean something. A time with no day is the
//! **most recent** one: at 3am, `11:30pm` was last night. And an end time is
//! the **first one after the start**, so a sleep from `23:30` to `01:15`
//! crosses midnight without anybody saying so.
//!
//! What cannot be guessed is asked instead. `3:57` on its own is either half
//! of the day and this module says so rather than picking one.

use jiff::civil::Date;

use super::time::Calendar;

/// A time of day, on a 24-hour clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeOfDay {
    /// The hour, from 0 to 23.
    pub hour: i8,
    /// The minute, from 0 to 59.
    pub minute: i8,
}

impl TimeOfDay {
    /// A time, if those numbers are one.
    #[must_use]
    pub const fn new(hour: i8, minute: i8) -> Option<Self> {
        if hour < 0 || hour > 23 || minute < 0 || minute > 59 {
            return None;
        }
        Some(Self { hour, minute })
    }

    /// How it reads on a twelve-hour clock, as this tool writes times.
    #[must_use]
    pub fn label(self) -> String {
        let (hour, half) = match self.hour {
            0 => (12, "am"),
            1..=11 => (self.hour, "am"),
            12 => (12, "pm"),
            _ => (self.hour - 12, "pm"),
        };
        format!("{hour}:{:02} {half}", self.minute)
    }
}

/// What a typed time turned out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Typed {
    /// One time, with no doubt about which half of the day it is in.
    Certain(TimeOfDay),
    /// A twelve-hour time with no `am` or `pm`, which is two times.
    Ambiguous {
        /// The morning reading.
        morning: TimeOfDay,
        /// The afternoon reading.
        afternoon: TimeOfDay,
    },
}

/// Reads a typed time.
///
/// Takes `3:57am`, `3:57 AM`, `3:57`, `357`, `0357`, `03:57`, `21:30`, `9pm`,
/// `9 p.m.` and `7`. Returns `None` for anything that is not a time, which the
/// caller turns into a question rather than a guess.
#[must_use]
pub fn parse(text: &str) -> Option<Typed> {
    let cleaned: String = trim_sentence_ending(text)
        .chars()
        .filter(|character| !character.is_whitespace() && *character != '.')
        .collect::<String>()
        .to_lowercase();
    if cleaned.is_empty() {
        return None;
    }

    let (digits, half) = split_half(&cleaned)?;
    let (hour, minute) = split_digits(digits)?;
    if minute > 59 {
        return None;
    }

    match half {
        // `12am` is midnight and `12pm` is noon, which is the one place a
        // twelve-hour clock disagrees with arithmetic.
        Some(Half::Morning) => TimeOfDay::new(if hour == 12 { 0 } else { hour }, minute)
            .filter(|_| (1..=12).contains(&hour))
            .map(Typed::Certain),
        Some(Half::Afternoon) => TimeOfDay::new(if hour == 12 { 12 } else { hour + 12 }, minute)
            .filter(|_| (1..=12).contains(&hour))
            .map(Typed::Certain),
        None if hour == 0 || hour > 12 || leads_with_zero(digits) => {
            TimeOfDay::new(hour, minute).map(Typed::Certain)
        }
        None => Some(Typed::Ambiguous {
            morning: TimeOfDay::new(if hour == 12 { 0 } else { hour }, minute)?,
            afternoon: TimeOfDay::new(if hour == 12 { 12 } else { hour + 12 }, minute)?,
        }),
    }
}

/// Reads a relative time against the supplied clock.
#[must_use]
pub fn parse_relative(text: &str, now: f64) -> Option<f64> {
    let cleaned = trim_sentence_ending(text).to_ascii_lowercase();
    if matches!(cleaned.as_str(), "now" | "right now") {
        return Some(now);
    }
    let duration = cleaned.strip_suffix("ago")?.trim();
    let digits = ["minutes", "minute", "mins", "min", "m"]
        .iter()
        .find_map(|suffix| duration.strip_suffix(suffix))?
        .trim();
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let minutes: u32 = digits.parse().ok()?;
    Some(now - f64::from(minutes) * 60.0)
}

/// Removes sentence endings without erasing signs, decimal points or time separators.
fn trim_sentence_ending(text: &str) -> &str {
    text.trim_end_matches(|character: char| {
        character.is_whitespace() || matches!(character, '.' | ',' | '!' | '?' | ';' | '…')
    })
    .trim_start()
}

/// Which half of the day was named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Half {
    /// `am`.
    Morning,
    /// `pm`.
    Afternoon,
}

/// Takes the `am` or `pm` off the end, if there is one.
fn split_half(cleaned: &str) -> Option<(&str, Option<Half>)> {
    for (suffix, half) in [
        ("am", Half::Morning),
        ("pm", Half::Afternoon),
        ("a", Half::Morning),
        ("p", Half::Afternoon),
    ] {
        if let Some(digits) = cleaned.strip_suffix(suffix) {
            return (!digits.is_empty()).then_some((digits, Some(half)));
        }
    }
    Some((cleaned, None))
}

/// Reads the digits, with or without a colon.
///
/// `357` is 3:57 and `0357` is 03:57, because a person typing four digits is
/// typing a clock and a person typing three is typing the same clock without
/// the leading zero.
fn split_digits(digits: &str) -> Option<(i8, i8)> {
    if let Some((hours, minutes)) = digits.split_once(':') {
        return Some((hours.parse().ok()?, minutes.parse().ok()?));
    }
    if !digits.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    match digits.len() {
        1 | 2 => Some((digits.parse().ok()?, 0)),
        3 => Some((digits[..1].parse().ok()?, digits[1..].parse().ok()?)),
        4 => Some((digits[..2].parse().ok()?, digits[2..].parse().ok()?)),
        _ => None,
    }
}

/// The part of the digits that is the hour, however they were written.
fn hour_text(digits: &str) -> &str {
    if let Some((hours, _)) = digits.split_once(':') {
        return hours;
    }
    match digits.len() {
        4 => &digits[..2],
        3 => &digits[..1],
        _ => digits,
    }
}

/// Whether the hour was written with a leading zero, which is how somebody
/// writing a 24-hour clock writes the morning.
fn leads_with_zero(digits: &str) -> bool {
    let hours = hour_text(digits);
    hours.len() == 2 && hours.starts_with('0')
}

/// The most recent moment at or before `now` that reads as this time.
#[must_use]
pub fn most_recent(time: TimeOfDay, now: f64, calendar: &Calendar) -> f64 {
    let today = calendar.day_of(now);
    let candidate = calendar.at(today, time.hour, time.minute);
    if candidate <= now {
        return candidate;
    }
    calendar.at(calendar.offset_day(today, -1), time.hour, time.minute)
}

/// The first moment at or after `from` that reads as this time.
///
/// What an end time is: a sleep that started at `23:30` and ended at `01:15`
/// ended the next day, and nobody should have to say so.
#[must_use]
pub fn first_after(time: TimeOfDay, from: f64, calendar: &Calendar) -> f64 {
    let day: Date = calendar.day_of(from);
    let candidate = calendar.at(day, time.hour, time.minute);
    if candidate >= from {
        return candidate;
    }
    calendar.at(calendar.offset_day(day, 1), time.hour, time.minute)
}

/// Whether two stretches of time run into each other.
///
/// Half-open, as every window in this repository: an entry that ends exactly
/// when the running sleep began does not overlap it. The running sleep's end
/// is now, unless it is paused, in which case it is the moment it was paused.
#[must_use]
pub fn overlaps(start: f64, end: f64, other_start: f64, other_end: f64) -> bool {
    end > other_start && start < other_end
}

#[cfg(test)]
mod reading {
    use super::*;

    fn certain(text: &str) -> TimeOfDay {
        match parse(text) {
            Some(Typed::Certain(time)) => time,
            other => panic!("`{text}` should read as one time, got {other:?}"),
        }
    }

    #[test]
    fn a_twelve_hour_time_is_read_whatever_case_the_half_is_in() {
        for text in ["3:57am", "3:57 AM", "3:57 a.m.", "3:57Am", "357am"] {
            assert_eq!(
                certain(text),
                TimeOfDay {
                    hour: 3,
                    minute: 57
                },
                "{text}"
            );
        }
        for text in ["9pm", "9 PM", "9:00 p.m.", "9 p"] {
            assert_eq!(
                certain(text),
                TimeOfDay {
                    hour: 21,
                    minute: 0
                },
                "{text}"
            );
        }
    }

    #[test]
    fn a_twenty_four_hour_time_needs_no_half() {
        assert_eq!(
            certain("21:30"),
            TimeOfDay {
                hour: 21,
                minute: 30
            }
        );
        assert_eq!(
            certain("2130"),
            TimeOfDay {
                hour: 21,
                minute: 30
            }
        );
        assert_eq!(
            certain("00:15"),
            TimeOfDay {
                hour: 0,
                minute: 15
            }
        );
    }

    #[test]
    fn a_leading_zero_is_somebody_writing_the_morning_in_full() {
        assert_eq!(
            certain("0357"),
            TimeOfDay {
                hour: 3,
                minute: 57
            }
        );
        assert_eq!(
            certain("03:57"),
            TimeOfDay {
                hour: 3,
                minute: 57
            }
        );
    }

    #[test]
    fn midnight_and_noon_are_where_a_twelve_hour_clock_stops_being_arithmetic() {
        assert_eq!(certain("12am"), TimeOfDay { hour: 0, minute: 0 });
        assert_eq!(
            certain("12:30am"),
            TimeOfDay {
                hour: 0,
                minute: 30
            }
        );
        assert_eq!(
            certain("12pm"),
            TimeOfDay {
                hour: 12,
                minute: 0
            }
        );
        assert_eq!(
            certain("12:30pm"),
            TimeOfDay {
                hour: 12,
                minute: 30
            }
        );
    }

    #[test]
    fn a_bare_hour_is_on_the_hour() {
        assert_eq!(certain("7am"), TimeOfDay { hour: 7, minute: 0 });
        assert_eq!(
            certain("19"),
            TimeOfDay {
                hour: 19,
                minute: 0
            }
        );
    }

    #[test]
    fn a_bare_twelve_hour_time_is_both_times_rather_than_a_guess() {
        let Some(Typed::Ambiguous { morning, afternoon }) = parse("3:57") else {
            panic!("3:57 is either half of the day");
        };
        assert_eq!(
            morning,
            TimeOfDay {
                hour: 3,
                minute: 57
            }
        );
        assert_eq!(
            afternoon,
            TimeOfDay {
                hour: 15,
                minute: 57
            }
        );

        let Some(Typed::Ambiguous { morning, afternoon }) = parse("12:30") else {
            panic!("12:30 is either half of the day");
        };
        assert_eq!(
            morning,
            TimeOfDay {
                hour: 0,
                minute: 30
            }
        );
        assert_eq!(
            afternoon,
            TimeOfDay {
                hour: 12,
                minute: 30
            }
        );
    }

    #[test]
    fn something_that_is_not_a_time_is_not_read_as_one() {
        for text in [
            "", "   ", "later", "25:00", "3:70", "13pm", "0am", "12345", "3:5x", "-3",
        ] {
            assert!(parse(text).is_none(), "`{text}` should not read as a time");
        }
    }

    #[test]
    fn a_time_reads_back_the_way_this_tool_writes_times() {
        assert_eq!(TimeOfDay { hour: 0, minute: 5 }.label(), "12:05 am");
        assert_eq!(
            TimeOfDay {
                hour: 13,
                minute: 0
            }
            .label(),
            "1:00 pm"
        );
        assert_eq!(
            TimeOfDay {
                hour: 12,
                minute: 0
            }
            .label(),
            "12:00 pm"
        );
    }
}

#[cfg(test)]
mod resolving {
    use super::*;

    /// Monday 22 September 2025, 2pm in New York.
    const AFTERNOON: f64 = 1_758_564_000.0;

    fn calendar() -> Calendar {
        Calendar::new("America/New_York").expect("a real timezone")
    }

    #[test]
    fn a_time_earlier_today_is_today() {
        let calendar = calendar();
        let at = most_recent(TimeOfDay { hour: 9, minute: 0 }, AFTERNOON, &calendar);
        assert!(at < AFTERNOON && AFTERNOON - at < 86_400.0);
        assert_eq!(calendar.day_of(at), calendar.day_of(AFTERNOON));
    }

    #[test]
    fn a_time_still_to_come_today_was_yesterday() {
        // The 3am case: "he went down at 11:30pm" means last night.
        let calendar = calendar();
        let at = most_recent(
            TimeOfDay {
                hour: 23,
                minute: 30,
            },
            AFTERNOON,
            &calendar,
        );
        assert!(at < AFTERNOON, "a time nobody has reached yet is yesterday");
        assert_eq!(
            calendar.day_of(at),
            calendar.offset_day(calendar.day_of(AFTERNOON), -1)
        );
    }

    #[test]
    fn an_end_earlier_on_the_clock_than_the_start_is_the_next_day() {
        let calendar = calendar();
        let start = most_recent(
            TimeOfDay {
                hour: 23,
                minute: 30,
            },
            AFTERNOON,
            &calendar,
        );
        let end = first_after(
            TimeOfDay {
                hour: 1,
                minute: 15,
            },
            start,
            &calendar,
        );
        assert!(end > start, "the sleep crossed midnight");
        assert!(
            (end - start - 6_300.0).abs() < 1.0,
            "an hour and 45 minutes"
        );
    }

    #[test]
    fn an_end_later_on_the_clock_than_the_start_is_the_same_day() {
        let calendar = calendar();
        let start = most_recent(TimeOfDay { hour: 9, minute: 0 }, AFTERNOON, &calendar);
        let end = first_after(
            TimeOfDay {
                hour: 10,
                minute: 30,
            },
            start,
            &calendar,
        );
        assert!((end - start - 5_400.0).abs() < 1.0);
    }

    #[test]
    fn an_entry_that_ends_before_a_running_sleep_began_does_not_touch_it() {
        // The running sleep here is 200 to 400.
        assert!(
            !overlaps(100.0, 200.0, 200.0, 400.0),
            "half-open, as everywhere"
        );
        assert!(!overlaps(100.0, 199.0, 200.0, 400.0));
        assert!(
            !overlaps(400.0, 500.0, 200.0, 400.0),
            "and at the other end"
        );
        assert!(overlaps(100.0, 201.0, 200.0, 400.0));
        assert!(overlaps(250.0, 300.0, 200.0, 400.0), "wholly inside it");
        assert!(overlaps(100.0, 500.0, 200.0, 400.0), "wholly around it");
    }
}

#[cfg(test)]
mod relative_reading {
    use super::*;

    #[test]
    fn relative_minutes_are_subtracted_from_the_supplied_clock() {
        for (text, expected) in [
            ("now", 1800.0),
            ("right now", 1800.0),
            ("10 minutes ago", 1200.0),
            ("20 mins ago", 600.0),
            ("28m ago", 120.0),
            ("1 minute ago", 1740.0),
            ("1 min ago", 1740.0),
            ("  2 MINS AGO  ", 1680.0),
            ("0m ago", 1800.0),
            ("40m ago", -600.0),
        ] {
            assert_eq!(parse_relative(text, 1800.0), Some(expected), "{text}");
        }
    }

    #[test]
    fn invalid_relative_times_are_not_guessed() {
        for text in [
            "",
            "10",
            "10m",
            "-1m ago",
            "1.5m ago",
            "NaN mins ago",
            "in 10 minutes",
            "10 minutes ago extra",
            "999999999999999999999m ago",
        ] {
            assert_eq!(parse_relative(text, 1800.0), None, "{text}");
        }
    }
}
