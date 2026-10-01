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
use crate::domain::reference::{self, Metric, Standing};
use crate::domain::summaries::{self, DaySummary};
use crate::domain::time::Calendar;
use crate::domain::types::Dataset;
use crate::theme::{Theme, Tone};

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
        width: 8,
        value: |row, _| format::duration(row.nursing_seconds),
    },
    Column {
        heading: "sleep",
        width: 8,
        value: |row, _| format::duration(row.sleep_seconds),
    },
    Column {
        heading: "night",
        width: 8,
        value: |row, _| format::duration(row.night_sleep_seconds),
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
        // Today is the row somebody is looking for, so it is the brightest
        // one rather than the dimmest. The rest stay fully legible.
        lines.push(if row.partial {
            theme.today(&text)
        } else {
            theme.value(&text)
        });
    }
    lines.push(String::new());
    let age = dataset
        .child
        .birthdate
        .as_deref()
        .and_then(|birthdate| calendar.age_in_days(birthdate, now));
    lines.extend(averages(rows, theme, units, age));
    lines.extend(bands(rows, dataset, theme, age));
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

/// One averaged figure, and the band it is judged against.
type Figure = (
    fn(&DaySummary) -> f64,
    fn(f64, Units) -> String,
    Option<Metric>,
);

/// What the average line reports, in the order it reports it.
///
/// Milk has no band, deliberately and permanently: see [`crate::domain::reference`].
const AVERAGED: &[Figure] = &[
    (
        |row| row.feed_count as f64,
        |value, _| format!("{value:.1} feeds"),
        Some(Metric::FeedsPerDay),
    ),
    (
        |row| row.total_ml,
        |value, units| format!("{} milk", format::volume(Some(value), units)),
        None,
    ),
    (
        |row| row.sleep_seconds,
        |value, _| format!("{} sleep", crate::domain::time::format_duration(value)),
        Some(Metric::SleepPerDay),
    ),
    (
        |row| row.wet_count as f64,
        |value, _| format!("{value:.1} wet"),
        Some(Metric::WetPerDay),
    ),
    (
        |row| row.dirty_count as f64,
        |value, _| format!("{value:.1} dirty"),
        Some(Metric::DirtyPerDay),
    ),
];

/// How a figure should be painted, given where it sits.
///
/// A figure with no band to judge it against is not "fine", it is unjudged,
/// and it stays grey. That distinction is the whole reason milk never turns
/// green.
#[must_use]
pub const fn tone_for(standing: Option<Standing>) -> Tone {
    match standing {
        Some(Standing::Inside) => Tone::Good,
        Some(Standing::Below | Standing::Above) => Tone::Attention,
        None => Tone::Muted,
    }
}

/// Where an averaged figure sits, if there is a band and a figure.
fn standing_of(rows: &[DaySummary], figure: &Figure, age: Option<i64>) -> Option<Standing> {
    let (pick, _, metric) = figure;
    let mean = summaries::average(rows, pick)?;
    let band = reference::band_for((*metric)?, age)?;
    Some(band.standing(mean))
}

/// The mean of each figure across the complete days that have data.
fn averages(rows: &[DaySummary], theme: Theme, units: Units, age: Option<i64>) -> Vec<String> {
    let counted = rows
        .iter()
        .filter(|row| !row.partial && row.has_data)
        .count();
    if counted == 0 {
        return vec![theme.muted("no complete days with anything logged yet")];
    }

    let painted: Vec<String> = AVERAGED
        .iter()
        .filter_map(|figure| {
            let (pick, render, _) = figure;
            let mean = summaries::average(rows, pick)?;
            let standing = standing_of(rows, figure, age);
            Some(theme.paint(tone_for(standing), &render(mean, units)))
        })
        .collect();

    vec![format!(
        "{}{}",
        theme.muted(&format!(
            "average over {counted} complete day{}: ",
            if counted == 1 { "" } else { "s" }
        )),
        painted.join(&theme.muted(" · "))
    )]
}

/// The typical ranges for this baby's age, each painted by where the week sits.
///
/// Yellow, never red. A figure outside a typical range is worth a second look
/// and is not an emergency, and this tool is in no position to say which.
fn bands(rows: &[DaySummary], dataset: &Dataset, theme: Theme, age: Option<i64>) -> Vec<String> {
    let Some(age) = age else {
        return Vec::new();
    };

    let mut lines = vec![theme.muted(&format!("{} is {age} days old", dataset.child.name))];
    let mut any_band = false;
    for figure in AVERAGED {
        let (pick, _, metric) = figure;
        let Some(band) = metric.and_then(|metric| reference::band_for(metric, Some(age))) else {
            continue;
        };
        any_band = true;
        let standing = summaries::average(rows, pick).map(|mean| band.standing(mean));
        let aside = match standing {
            Some(Standing::Below) => " (this week is under that)",
            Some(Standing::Above) => " (this week is over that)",
            // Said in words as well as in colour, so the meaning survives a
            // pipe, a screenshot, and colour blindness.
            Some(Standing::Inside) => " (this week is in that range)",
            None => "",
        };
        lines.push(theme.paint(tone_for(standing), &format!("  {}{aside}", band.label)));
    }
    // An age too old for any band leaves nothing for the note to qualify.
    if !any_band {
        return Vec::new();
    }
    lines.push(String::new());
    lines.push(theme.muted(&format!("  {}", reference::PEDIATRICIAN_NOTE)));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::{AFTERNOON, bottle, dataset, diaper};

    /// Days from 7am and nights from 8pm, which is what Huckleberry assumes
    /// and what a configuration nobody has finished falls back to.
    fn rule() -> crate::domain::today::DayRule {
        crate::domain::today::DayRule::default()
    }

    fn calendar() -> Calendar {
        Calendar::new("America/New_York").expect("a real timezone")
    }

    fn table(data: &Dataset, days: usize) -> String {
        let calendar = calendar();
        let rows = summaries::build(data, &calendar, rule(), AFTERNOON, days);
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
    fn sleep_columns_show_hours_and_minutes() {
        let mut rows = summaries::build(&dataset(), &calendar(), rule(), AFTERNOON, 1);
        rows[0].sleep_seconds = 9000.0;
        rows[0].night_sleep_seconds = 9000.0;
        let rendered = data_row(&rows[0], Units::Ml);
        assert_eq!(rendered.matches("2h 30m").count(), 2, "{rendered}");
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
        let rows = summaries::build(&data, &calendar, rule(), AFTERNOON, 3);
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
            data.diapers.push(diaper(
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
            data.diapers.push(diaper(
                AFTERNOON - 86_400.0 - f64::from(hour) * 3_600.0,
                true,
                false,
            ));
        }
        let rendered = table(&data, 3);
        assert!(rendered.contains("is 21 days old"), "{rendered}");
        assert!(rendered.contains("typical"), "{rendered}");
    }

    /// The same note the `now` screen carries, in the same words, because the
    /// pediatrician outranks this table wherever it is printed.
    #[test]
    fn the_typical_ranges_carry_the_pediatrician_note() {
        let rendered = table(&dataset(), 3);
        assert!(
            rendered.contains(
                "typical ranges are not your baby: where your pediatrician disagrees, they are right"
            ),
            "{rendered}"
        );
    }

    #[test]
    fn with_no_bands_there_is_no_pediatrician_note_either() {
        let mut data = dataset();
        data.child.birthdate = None;
        assert!(!table(&data, 3).contains("pediatrician"));
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

    /// A week with exactly six wet diapers a day, which is inside the band
    /// for a three-week-old, and almost no sleep, which is under it.
    fn lopsided_week() -> Dataset {
        let mut data = dataset();
        for back in 1..=6 {
            for hour in 0..6 {
                data.diapers.push(diaper(
                    AFTERNOON - f64::from(back) * 86_400.0 - f64::from(hour) * 3_600.0,
                    true,
                    false,
                ));
            }
        }
        data
    }

    fn painted(data: &Dataset, days: usize) -> String {
        let calendar = calendar();
        let rows = summaries::build(data, &calendar, rule(), AFTERNOON, days);
        lines(
            &rows,
            data,
            &calendar,
            Theme::dark(true),
            Units::Ml,
            AFTERNOON,
        )
        .join("\n")
    }

    fn sequence(tone: Tone) -> String {
        format!("\u{1b}[{}m", tone.sgr())
    }

    #[test]
    fn today_is_the_brightest_row_and_the_rest_stay_legible() {
        let drawn = painted(&lopsided_week(), 3);
        let today: Vec<&str> = drawn
            .lines()
            .filter(|line| line.contains(&format::day_short("2025-09-22".parse().expect("a date"))))
            .collect();
        assert_eq!(today.len(), 1, "one row is today");
        assert!(
            today[0].starts_with(&sequence(Tone::Today)),
            "today should be the brightest row: {:?}",
            today[0]
        );

        let yesterday: Vec<&str> = drawn
            .lines()
            .filter(|line| line.contains(&format::day_short("2025-09-21".parse().expect("a date"))))
            .collect();
        assert!(
            yesterday[0].starts_with(&sequence(Tone::Value)),
            "the other days stay fully legible: {:?}",
            yesterday[0]
        );
    }

    #[test]
    fn a_figure_inside_its_band_is_green_and_one_outside_it_is_yellow() {
        let drawn = painted(&lopsided_week(), 7);
        let wet = drawn
            .lines()
            .find(|line| line.contains("wet diapers a day"))
            .expect("the wet diaper band");
        assert!(wet.starts_with(&sequence(Tone::Good)), "{wet:?}");
        assert!(wet.contains("in that range"), "{wet:?}");

        let sleep = drawn
            .lines()
            .find(|line| line.contains("hours a day"))
            .expect("the sleep band");
        assert!(sleep.starts_with(&sequence(Tone::Attention)), "{sleep:?}");
        assert!(sleep.contains("under that"), "{sleep:?}");
    }

    #[test]
    fn a_figure_with_no_band_to_judge_it_is_grey_rather_than_green() {
        // Milk has no band at any age, so it must never read as approved.
        assert_eq!(tone_for(None), Tone::Muted);
        assert_eq!(tone_for(Some(Standing::Inside)), Tone::Good);
        assert_eq!(tone_for(Some(Standing::Below)), Tone::Attention);
        assert_eq!(tone_for(Some(Standing::Above)), Tone::Attention);
    }

    #[test]
    fn the_average_line_carries_colour_figure_by_figure() {
        let drawn = painted(&lopsided_week(), 7);
        let average = drawn
            .lines()
            .find(|line| line.contains("average over"))
            .expect("the average line");
        assert!(average.contains(&sequence(Tone::Good)), "{average:?}");
        assert!(average.contains(&sequence(Tone::Attention)), "{average:?}");
        assert!(
            average.contains(&sequence(Tone::Muted)),
            "milk has no band, so it stays grey: {average:?}"
        );
    }

    #[test]
    fn the_meaning_survives_a_pipe_with_no_colour_at_all() {
        let plain = table(&lopsided_week(), 7);
        assert!(plain.contains("in that range"), "{plain}");
        assert!(plain.contains("under that"), "{plain}");
        assert!(!plain.contains('\u{1b}'));
    }

    #[test]
    fn nothing_is_ever_painted_as_a_warning_or_an_error() {
        let mut data = dataset();
        data.diapers = vec![diaper(AFTERNOON - 86_400.0, true, false)];
        let calendar = calendar();
        let rows = summaries::build(&data, &calendar, rule(), AFTERNOON, 3);
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
