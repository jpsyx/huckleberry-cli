//! The day table: one row per day, the numbers a pediatrician asks for.
//!
//! Plain columns, no box drawing. The table is meant to be read on a phone
//! over SSH at 3am and to survive being pasted into a message, and a
//! hand-aligned grid of box characters survives neither.
//!
//! Typical ranges appear as a grey line under the table, never as a colour on
//! a row. Nothing here is ever painted red: a number outside a band is
//! visibly outside it, and what to do about that is the reader's call.

use crate::cli::Units;
use crate::domain::reference::{self, Metric};
use crate::domain::summaries::{self, DaySummary};
use crate::domain::time::Calendar;
use crate::domain::types::Dataset;
use crate::theme::Theme;

use super::format;

/// One column of the table.
struct Column {
    heading: &'static str,
    width: usize,
    value: fn(&DaySummary, Units) -> String,
}

const COLUMNS: &[Column] = &[
    Column {
        heading: "day",
        width: 11,
        value: |row, _| format::day_short(row.day),
    },
    Column {
        heading: "feeds",
        width: 5,
        value: |row, _| format::count(row.feed_count),
    },
    Column {
        heading: "milk",
        width: 6,
        value: |row, units| volume_or_dash(row.total_ml, units),
    },
    Column {
        heading: "formula",
        width: 7,
        value: |row, units| volume_or_dash(row.formula_ml, units),
    },
    Column {
        heading: "breast",
        width: 6,
        value: |row, units| volume_or_dash(row.breast_milk_ml, units),
    },
    Column {
        heading: "nursed",
        width: 6,
        value: |row, _| format::duration(row.nursing_seconds),
    },
    Column {
        heading: "sleep",
        width: 5,
        value: |row, _| format::hours(row.sleep_seconds),
    },
    Column {
        heading: "night",
        width: 5,
        value: |row, _| format::hours(row.night_sleep_seconds),
    },
    Column {
        heading: "longest",
        width: 7,
        value: |row, _| format::duration(row.longest_sleep_seconds),
    },
    Column {
        heading: "wet",
        width: 3,
        value: |row, _| format::count(row.wet_count),
    },
    Column {
        heading: "dirty",
        width: 5,
        value: |row, _| format::count(row.dirty_count),
    },
];

fn volume_or_dash(millilitres: f64, units: Units) -> String {
    format::volume_bare((millilitres > 0.0).then_some(millilitres), units)
}

/// The table, one line per row, ready for stdout.
#[must_use]
pub fn lines(
    rows: &[DaySummary],
    dataset: &Dataset,
    calendar: &Calendar,
    theme: Theme,
    units: Units,
    now: f64,
) -> Vec<String> {
    let mut lines = vec![theme.heading(&heading_row(units))];
    for row in rows {
        let text = data_row(row, units);
        lines.push(if row.partial {
            // Today is still happening, so its numbers are not comparable with
            // the rest. Muted rather than hidden: a parent wants to see it.
            theme.muted(&text)
        } else {
            theme.value(&text)
        });
    }
    lines.push(String::new());
    lines.extend(averages(rows, theme, units));
    lines.extend(bands(rows, dataset, calendar, theme, now));
    lines
}

/// The heading row, with the volume unit named in it.
#[must_use]
pub fn heading_row(units: Units) -> String {
    let unit = match units {
        Units::Ml => "ml",
        Units::Oz => "oz",
    };
    COLUMNS
        .iter()
        .map(|column| {
            let heading = match column.heading {
                "milk" | "formula" | "breast" => format!("{} {unit}", column.heading),
                other => other.to_owned(),
            };
            let width = column.width.max(heading.chars().count());
            if column.heading == "day" {
                format::pad(&heading, width)
            } else {
                format::pad_left(&heading, width)
            }
        })
        .collect::<Vec<_>>()
        .join("  ")
}

/// One day's row.
#[must_use]
pub fn data_row(row: &DaySummary, units: Units) -> String {
    COLUMNS
        .iter()
        .map(|column| {
            let heading_width = match column.heading {
                "milk" | "formula" | "breast" => column.heading.chars().count() + 3,
                other => other.chars().count(),
            };
            let width = column.width.max(heading_width);
            let value = (column.value)(row, units);
            if column.heading == "day" {
                format::pad(&value, width)
            } else {
                format::pad_left(&value, width)
            }
        })
        .collect::<Vec<_>>()
        .join("  ")
}

/// The mean of each figure across the complete days that have data.
fn averages(rows: &[DaySummary], theme: Theme, units: Units) -> Vec<String> {
    let counted = rows
        .iter()
        .filter(|row| !row.partial && row.has_data)
        .count();
    if counted == 0 {
        return vec![theme.muted("no complete days with anything logged yet")];
    }
    let mean = |pick: fn(&DaySummary) -> f64| summaries::average(rows, pick);
    let parts = [
        mean(|row| row.feed_count as f64).map(|value| format!("{value:.1} feeds")),
        mean(|row| row.total_ml)
            .map(|value| format!("{} milk", format::volume(Some(value), units))),
        mean(|row| row.sleep_seconds).map(|value| format!("{:.1}h sleep", value / 3600.0)),
        mean(|row| row.wet_count as f64).map(|value| format!("{value:.1} wet")),
        mean(|row| row.dirty_count as f64).map(|value| format!("{value:.1} dirty")),
    ];
    let joined = parts.into_iter().flatten().collect::<Vec<_>>().join(" · ");
    vec![theme.muted(&format!(
        "average over {counted} complete day{}: {joined}",
        if counted == 1 { "" } else { "s" }
    ))]
}

/// The typical ranges for this baby's age, as grey text under the table.
fn bands(
    rows: &[DaySummary],
    dataset: &Dataset,
    calendar: &Calendar,
    theme: Theme,
    now: f64,
) -> Vec<String> {
    let Some(age) = dataset
        .child
        .birthdate
        .as_deref()
        .and_then(|birthdate| calendar.age_in_days(birthdate, now))
    else {
        return Vec::new();
    };

    let mut lines = vec![theme.muted(&format!("{} is {age} days old", dataset.child.name))];
    for (metric, mean) in [
        (
            Metric::FeedsPerDay,
            summaries::average(rows, |row| row.feed_count as f64),
        ),
        (
            Metric::WetPerDay,
            summaries::average(rows, |row| row.wet_count as f64),
        ),
        (
            Metric::DirtyPerDay,
            summaries::average(rows, |row| row.dirty_count as f64),
        ),
        (
            Metric::SleepPerDay,
            summaries::average(rows, |row| row.sleep_seconds),
        ),
    ] {
        let Some(band) = reference::band_for(metric, Some(age)) else {
            continue;
        };
        let standing = mean.map_or(String::new(), |value| match band.standing(value) {
            reference::Standing::Inside => String::new(),
            reference::Standing::Below => " (this week is under that)".to_owned(),
            reference::Standing::Above => " (this week is over that)".to_owned(),
        });
        lines.push(theme.muted(&format!("  {}{standing}", band.label)));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::{AFTERNOON, bottle, dataset, nappy};

    fn calendar() -> Calendar {
        Calendar::new("America/New_York").expect("a real timezone")
    }

    fn table(data: &Dataset, days: usize) -> String {
        let calendar = calendar();
        let rows = summaries::build(data, &calendar, AFTERNOON, days);
        lines(
            &rows,
            data,
            &calendar,
            Theme::dark(false),
            Units::Ml,
            AFTERNOON,
        )
        .join("\n")
    }

    #[test]
    fn the_heading_names_every_column_and_the_unit() {
        let heading = heading_row(Units::Ml);
        for column in ["day", "feeds", "milk ml", "sleep", "wet", "dirty"] {
            assert!(
                heading.contains(column),
                "`{column}` missing from `{heading}`"
            );
        }
    }

    #[test]
    fn the_unit_in_the_heading_follows_the_setting() {
        assert!(heading_row(Units::Oz).contains("milk oz"));
    }

    #[test]
    fn every_row_lines_up_with_the_heading() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 3_600.0, 90.0)];
        let calendar = calendar();
        let rows = summaries::build(&data, &calendar, AFTERNOON, 3);
        let width = heading_row(Units::Ml).chars().count();
        for row in &rows {
            assert_eq!(
                data_row(row, Units::Ml).chars().count(),
                width,
                "row for {} is a different width from the heading",
                row.day
            );
        }
    }

    #[test]
    fn a_day_with_nothing_logged_shows_dashes_rather_than_zeroes() {
        let rendered = table(&dataset(), 2);
        assert!(rendered.contains('—'), "{rendered}");
        assert!(!rendered.contains(" 0 "), "{rendered}");
    }

    #[test]
    fn with_no_complete_day_the_averages_say_so_instead_of_showing_nothing() {
        assert!(table(&dataset(), 3).contains("no complete days"));
    }

    #[test]
    fn the_average_counts_only_the_complete_days_that_have_data() {
        let mut data = dataset();
        for hour in 0..6 {
            data.diapers.push(nappy(
                AFTERNOON - 86_400.0 - f64::from(hour) * 3_600.0,
                true,
                false,
            ));
        }
        let rendered = table(&data, 3);
        assert!(
            rendered.contains("average over 1 complete day:"),
            "{rendered}"
        );
        assert!(rendered.contains("6.0 wet"), "{rendered}");
    }

    #[test]
    fn the_typical_ranges_are_shown_with_the_babys_age() {
        let mut data = dataset();
        for hour in 0..6 {
            data.diapers.push(nappy(
                AFTERNOON - 86_400.0 - f64::from(hour) * 3_600.0,
                true,
                false,
            ));
        }
        let rendered = table(&data, 3);
        assert!(rendered.contains("is 21 days old"), "{rendered}");
        assert!(rendered.contains("typical"), "{rendered}");
    }

    #[test]
    fn no_typical_range_is_ever_offered_for_milk_volume() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 86_400.0, 90.0)];
        let rendered = table(&data, 3);
        for line in rendered.lines().filter(|line| line.contains("typical")) {
            assert!(
                !line.contains("ml"),
                "a volume band would tell a frightened parent they are underfeeding: {line}"
            );
        }
    }

    #[test]
    fn a_profile_with_no_birthdate_gets_no_bands_rather_than_wrong_ones() {
        let mut data = dataset();
        data.child.birthdate = None;
        assert!(!table(&data, 3).contains("typical"));
    }

    #[test]
    fn nothing_is_ever_painted_as_a_warning_or_an_error() {
        let mut data = dataset();
        data.diapers = vec![nappy(AFTERNOON - 86_400.0, true, false)];
        let calendar = calendar();
        let rows = summaries::build(&data, &calendar, AFTERNOON, 3);
        let painted = lines(
            &rows,
            &data,
            &calendar,
            Theme::dark(true),
            Units::Ml,
            AFTERNOON,
        )
        .join("\n");
        for alarming in [
            crate::theme::Tone::Error.sgr(),
            crate::theme::Tone::Warning.sgr(),
        ] {
            assert!(
                !painted.contains(&format!("\u{1b}[{alarming}m")),
                "nothing about a baby's day is an error or a warning"
            );
        }
    }
}
