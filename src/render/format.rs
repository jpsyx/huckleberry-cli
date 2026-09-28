//! Turning values into the text a person reads.
//!
//! Everything here is pure: a value in, a string out, no clock and no
//! terminal. That is what lets the screens be asserted line by line in a test
//! rather than looked at.
//!
//! Two conventions run through it. A quantity nobody recorded prints as a
//! dash, never as a zero: `0 ml` is a claim about the baby and `—` is a claim
//! about the record. And every bar is drawn out of Unicode blocks at eighth
//! resolution, so a difference of a few percent is visible in a character cell
//! instead of rounding away.

use jiff::civil::Date;

use crate::cli::Units;
use crate::domain::time::Calendar;

/// What is printed where a number was never recorded.
pub const MISSING: &str = "—";

/// A volume, in the units the reader asked for.
#[must_use]
pub fn volume(millilitres: Option<f64>, units: Units) -> String {
    let Some(amount) = millilitres else {
        return MISSING.to_owned();
    };
    match units {
        Units::Ml => format!("{amount:.0} ml"),
        Units::Oz => format!(
            "{:.1} oz",
            amount / huckleberry_api::models::feed::MILLILITRES_PER_OUNCE
        ),
    }
}

/// A volume with no unit on it, for a column that has one in its heading.
#[must_use]
pub fn volume_bare(millilitres: Option<f64>, units: Units) -> String {
    let Some(amount) = millilitres else {
        return MISSING.to_owned();
    };
    match units {
        Units::Ml => format!("{amount:.0}"),
        Units::Oz => format!(
            "{:.1}",
            amount / huckleberry_api::models::feed::MILLILITRES_PER_OUNCE
        ),
    }
}

/// An amount already in the units it is being written in, as that unit is
/// read: millilitres whole, ounces to two places with nothing trailing.
///
/// Rounding ounces the way millilitres are rounded is how a bottle of 1.25 oz
/// reads back as "1 oz", which is a different bottle. [`volume`] is the one to
/// reach for when the number is in millilitres and needs converting;
/// this one is for a number a person typed in the units they typed it in.
#[must_use]
pub fn amount_in(amount: f64, units: Units) -> String {
    match units {
        Units::Ml => format!("{amount:.0}"),
        Units::Oz => {
            let text = format!("{amount:.2}");
            text.trim_end_matches('0').trim_end_matches('.').to_owned()
        }
    }
}

/// The same amount in other units.
#[must_use]
pub fn convert(amount: f64, from: Units, to: Units) -> f64 {
    to.to_api()
        .from_millilitres(from.to_api().to_millilitres(amount))
}

/// A count, or a dash when there is nothing to count.
#[must_use]
pub fn count(value: usize) -> String {
    if value == 0 {
        MISSING.to_owned()
    } else {
        value.to_string()
    }
}

/// A duration, or a dash when there was none.
#[must_use]
pub fn duration(seconds: f64) -> String {
    if seconds <= 0.0 {
        return MISSING.to_owned();
    }
    crate::domain::time::format_duration(seconds)
}

/// Hours with one decimal, for a column of daily sleep totals where `14.2`
/// compares more easily than `14h 12m`.
#[must_use]
pub fn hours(seconds: f64) -> String {
    if seconds <= 0.0 {
        return MISSING.to_owned();
    }
    format!("{:.1}", seconds / 3600.0)
}

/// A wall-clock time, as a person reads one: `8:17 am`.
#[must_use]
pub fn clock(at: f64, calendar: &Calendar) -> String {
    let zoned = calendar.zoned(at);
    let hour = zoned.hour();
    let (display, suffix) = match hour {
        0 => (12, "am"),
        1..=11 => (hour, "am"),
        12 => (12, "pm"),
        _ => (hour - 12, "pm"),
    };
    format!("{display}:{:02} {suffix}", zoned.minute())
}

/// A day, short: `Mon 22 Sep`.
#[must_use]
pub fn day_short(day: Date) -> String {
    day.strftime("%a %d %b").to_string()
}

/// A day, as the machine-readable key: `2025-09-22`.
#[must_use]
pub fn day_key(day: Date) -> String {
    day.to_string()
}

/// The eighth-resolution blocks a bar is drawn from.
const EIGHTHS: [char; 8] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];

/// A horizontal bar `width` cells wide, filled in proportion to `value/max`.
///
/// Partial cells are drawn at eighth resolution so two days that differ by a
/// few percent look different, which a whole-cell bar at this width would not
/// manage. A value of zero draws nothing at all rather than a stub, because a
/// stub reads as a small amount.
#[must_use]
pub fn bar(value: f64, max: f64, width: usize) -> String {
    let positive = |number: f64| number.partial_cmp(&0.0) == Some(core::cmp::Ordering::Greater);
    if !positive(value) || !positive(max) || width == 0 {
        return String::new();
    }
    let filled = (value / max).clamp(0.0, 1.0) * width as f64;
    let whole = filled.floor() as usize;
    let remainder = filled - whole as f64;
    let mut drawn: String = EIGHTHS[7].to_string().repeat(whole.min(width));
    if whole < width && remainder > 0.0 {
        let eighth = ((remainder * 8.0).ceil() as usize).clamp(1, 8);
        drawn.push(EIGHTHS[eighth - 1]);
    }
    drawn
}

/// The blocks a sparkline is drawn from, lowest to highest.
const SPARKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// A one-line chart of a series.
///
/// Scaled from zero rather than from the minimum, because a series of daily
/// totals is a quantity and not a deviation: scaling from the minimum would
/// make 11, 12 and 13 wet diapers look like a crisis and a recovery.
#[must_use]
pub fn sparkline(values: &[f64]) -> String {
    let max = values.iter().copied().fold(0.0_f64, f64::max);
    if max <= 0.0 {
        return SPARKS[0].to_string().repeat(values.len());
    }
    values
        .iter()
        .map(|value| {
            let step = ((value / max) * 7.0).round().clamp(0.0, 7.0) as usize;
            SPARKS[step]
        })
        .collect()
}

/// Pads text to a width, counting characters rather than bytes so an accented
/// name or a block character does not throw a column out.
#[must_use]
pub fn pad(text: &str, width: usize) -> String {
    let length = text.chars().count();
    if length >= width {
        return text.to_owned();
    }
    format!("{text}{}", " ".repeat(width - length))
}

/// Pads text to a width on the left, for a column of numbers.
#[must_use]
pub fn pad_left(text: &str, width: usize) -> String {
    let length = text.chars().count();
    if length >= width {
        return text.to_owned();
    }
    format!("{}{text}", " ".repeat(width - length))
}

/// Cuts text to a width, with an ellipsis when it had to.
#[must_use]
pub fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    if width <= 1 {
        return "…".to_owned();
    }
    let kept: String = text.chars().take(width - 1).collect();
    format!("{kept}…")
}

#[cfg(test)]
mod volumes {
    use super::*;

    #[test]
    fn a_volume_is_shown_in_the_unit_the_reader_asked_for() {
        assert_eq!(volume(Some(90.0), Units::Ml), "90 ml");
        assert_eq!(volume(Some(88.72), Units::Oz), "3.0 oz");
    }

    #[test]
    fn an_amount_nobody_recorded_is_a_dash_and_never_a_zero() {
        assert_eq!(volume(None, Units::Ml), MISSING);
        assert_eq!(volume_bare(None, Units::Oz), MISSING);
        assert_eq!(count(0), MISSING);
        assert_eq!(duration(0.0), MISSING);
        assert_eq!(hours(0.0), MISSING);
    }

    #[test]
    fn a_real_zero_and_an_absent_value_look_different_where_it_matters() {
        assert_eq!(volume(Some(0.0), Units::Ml), "0 ml");
        assert_eq!(volume(None, Units::Ml), MISSING);
    }
}

#[cfg(test)]
mod clocks {
    use super::*;

    fn new_york() -> Calendar {
        Calendar::new("America/New_York").expect("a real timezone")
    }

    #[test]
    fn a_time_is_shown_on_a_twelve_hour_clock() {
        // 2025-09-22T18:00:00Z is 2pm in New York.
        assert_eq!(clock(1_758_564_000.0, &new_york()), "2:00 pm");
    }

    #[test]
    fn midnight_and_noon_are_both_twelve() {
        let calendar = new_york();
        let midnight = calendar.at("2025-09-22".parse().expect("a date"), 0, 0);
        let noon = calendar.at("2025-09-22".parse().expect("a date"), 12, 0);
        assert_eq!(clock(midnight, &calendar), "12:00 am");
        assert_eq!(clock(noon, &calendar), "12:00 pm");
    }

    #[test]
    fn a_minute_under_ten_keeps_its_leading_zero() {
        let calendar = new_york();
        let early = calendar.at("2025-09-22".parse().expect("a date"), 8, 7);
        assert_eq!(clock(early, &calendar), "8:07 am");
    }

    #[test]
    fn a_day_is_shown_with_its_weekday() {
        let day: Date = "2025-09-22".parse().expect("a date");
        assert_eq!(day_short(day), "Mon 22 Sep");
        assert_eq!(day_key(day), "2025-09-22");
    }
}

#[cfg(test)]
mod charts {
    use super::*;

    #[test]
    fn a_full_bar_fills_the_width() {
        assert_eq!(bar(10.0, 10.0, 5).chars().count(), 5);
        assert!(bar(10.0, 10.0, 5).chars().all(|cell| cell == '█'));
    }

    #[test]
    fn a_half_bar_fills_half_the_width() {
        assert_eq!(
            bar(5.0, 10.0, 10)
                .chars()
                .filter(|cell| *cell == '█')
                .count(),
            5
        );
    }

    #[test]
    fn nothing_draws_nothing_rather_than_a_stub_that_reads_as_a_little() {
        assert_eq!(bar(0.0, 10.0, 10), "");
        assert_eq!(bar(5.0, 0.0, 10), "");
        assert_eq!(bar(5.0, 10.0, 0), "");
    }

    #[test]
    fn a_small_value_still_shows_as_a_partial_cell() {
        let sliver = bar(1.0, 100.0, 10);
        assert_eq!(sliver.chars().count(), 1, "one partial cell: {sliver}");
        assert_ne!(sliver, "█");
    }

    #[test]
    fn a_bar_never_runs_past_its_width() {
        assert_eq!(bar(1000.0, 10.0, 8).chars().count(), 8);
    }

    #[test]
    fn a_sparkline_has_one_cell_per_value() {
        assert_eq!(sparkline(&[1.0, 2.0, 3.0]).chars().count(), 3);
    }

    #[test]
    fn a_sparkline_is_scaled_from_zero_not_from_its_own_minimum() {
        // Three values a few percent apart should look alike, not like a
        // cliff, because a daily total is a quantity and not a deviation.
        let drawn = sparkline(&[11.0, 12.0, 13.0]);
        let steps: Vec<char> = drawn.chars().collect();
        assert_eq!(steps[2], '█');
        assert!(steps[0] >= '▅', "the lowest should still be high: {drawn}");
    }

    #[test]
    fn a_flat_zero_series_draws_a_floor_rather_than_nothing() {
        assert_eq!(sparkline(&[0.0, 0.0, 0.0]), "▁▁▁");
    }
}

#[cfg(test)]
mod columns {
    use super::*;

    #[test]
    fn padding_counts_characters_and_not_bytes() {
        assert_eq!(pad("café", 6), "café  ");
        assert_eq!(pad_left("9", 3), "  9");
    }

    #[test]
    fn text_already_wide_enough_is_left_alone() {
        assert_eq!(pad("wide", 2), "wide");
        assert_eq!(pad_left("wide", 2), "wide");
    }

    #[test]
    fn truncation_says_that_it_truncated() {
        assert_eq!(truncate("a long description", 8), "a long …");
        assert_eq!(truncate("short", 8), "short");
        assert_eq!(truncate("anything", 1), "…");
    }
}
