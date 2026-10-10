//! The day table: one row per day, the numbers a pediatrician asks for.
//!
//! Category headings span their numeric columns; the same catalog drives plain
//! output, interactive selection and the Ratatui graph. Typical ranges stay
//! below the table, never as a verdict painted onto a day's row.

use crate::cli::Units;
use crate::domain::reference::{self, Metric, Standing};
use crate::domain::summaries::{self, DaySummary};
use crate::domain::time::Calendar;
use crate::domain::types::Dataset;
use crate::theme::{Theme, Tone};

use super::format;

mod chart;
pub mod columns;
pub mod view;

use columns::{
    COLUMNS, data_cells, group_row, heading_cells, join_cells, join_heading_cells, subgroup_row,
};

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
    let mut lines = vec![
        theme.heading(&group_row(units, 0..COLUMNS.len())),
        theme.heading(&subgroup_row(units, 0..COLUMNS.len())),
        theme.heading(&heading_row(units)),
    ];
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
    if let Some(age) = age_in_days(dataset, calendar, now) {
        lines.push(theme.muted(&format!("{} is {age} days old", dataset.child.name)));
    }
    lines.extend(footer(rows, dataset, calendar, theme, units, now));
    lines.push(theme.muted(&super::now::as_of(dataset, now)));
    lines
}

/// Notes and reference ranges shared by the plain and interactive views.
#[must_use]
pub fn footer(
    rows: &[DaySummary],
    dataset: &Dataset,
    calendar: &Calendar,
    theme: Theme,
    units: Units,
    now: f64,
) -> Vec<String> {
    let age = age_in_days(dataset, calendar, now);
    let mut lines = notes(rows, dataset, theme, units, age);
    lines.extend(bands(rows, theme, age));
    lines
}

fn notes(
    rows: &[DaySummary],
    dataset: &Dataset,
    theme: Theme,
    units: Units,
    age: Option<i64>,
) -> Vec<String> {
    let mut lines = vec![String::new()];
    lines.extend(
        night_note(dataset)
            .into_iter()
            .map(|line| theme.muted(&line)),
    );
    lines.extend(averages(rows, theme, units, age));
    lines
}

/// Age shared by the plain summary and the interactive header.
fn age_in_days(dataset: &Dataset, calendar: &Calendar, now: f64) -> Option<i64> {
    dataset
        .child
        .birthdate
        .as_deref()
        .and_then(|birthdate| calendar.age_in_days(birthdate, now))
}

fn night_note(dataset: &Dataset) -> Vec<String> {
    let start = crate::domain::time::split_hour(dataset.child.night_start_hour);
    let end = crate::domain::time::split_hour(dataset.child.morning_cutoff_hour);
    let label = |(hour, minute)| crate::domain::clock::TimeOfDay { hour, minute }.label();
    vec![format!(
        "* Night: {} to the next {} ({}).",
        label(start),
        label(end),
        dataset.timezone
    )]
}

/// The heading row, with the volume unit named in it.
#[must_use]
pub fn heading_row(units: Units) -> String {
    join_heading_cells(
        "day",
        &heading_cells(units, 0..COLUMNS.len(), None),
        0..COLUMNS.len(),
    )
}

/// A day label that makes incomplete coverage visible without colour.
fn day_label(row: &DaySummary) -> String {
    format!(
        "{}{}",
        format::day_short(row.day),
        if row.partial { "~" } else { " " }
    )
}

/// One day's row, using the same columns as the interactive view.
#[must_use]
pub fn data_row(row: &DaySummary, units: Units) -> String {
    join_cells(
        &day_label(row),
        &data_cells(row, units, 0..COLUMNS.len()),
        0..COLUMNS.len(),
    )
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
fn bands(rows: &[DaySummary], theme: Theme, age: Option<i64>) -> Vec<String> {
    let Some(age) = age else {
        return Vec::new();
    };

    let mut lines = Vec::new();
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
    #[test]
    fn summary_has_grouped_headers_and_displays_type_specific_averages() {
        use crate::domain::fixtures::nursing;
        let mut data = dataset();
        data.feeds = vec![
            bottle(AFTERNOON, 90.0),
            bottle(AFTERNOON - 3600.0, 150.0),
            nursing(AFTERNOON - 1800.0, 600.0, 300.0),
        ];
        let rendered = table(&data, 1);
        let lines: Vec<_> = rendered.lines().collect();
        for group in ["feed", "sleep", "diaper", "wake time"] {
            assert!(lines[0].contains(group), "{rendered}");
        }
        assert!(lines[1].contains("bottle feed"), "{rendered}");
        assert!(lines[1].contains("nursed"), "{rendered}");
        for heading in ["ml/feed", "nurse/feed", "avg sleep", "avg wake window"] {
            assert!(lines[2].contains(heading), "{rendered}");
        }
        assert!(lines[3].contains("120"), "{rendered}");
        assert!(lines[3].contains("15m"), "{rendered}");
    }

    #[test]
    fn summary_feed_delimiters_bound_bottle_columns_in_both_lower_headers() {
        let rendered = table(&dataset(), 1);
        let lines: Vec<_> = rendered.lines().collect();
        let subgroups: Vec<_> = lines[1].split('│').map(str::trim).collect();
        assert_eq!(subgroups, ["", "", "bottle feed", "nursed", "", "", ""]);
        let headings: Vec<_> = lines[2].split('│').map(str::trim).collect();
        assert_eq!(headings[1], "feeds");
        assert!(headings[2].starts_with("milk ml"));
        assert!(headings[2].contains("milk ml/feed"));
        assert!(headings[2].ends_with("% feed daytime"));
        assert!(headings[3].starts_with("nursed"));
        assert!(headings[3].ends_with("nurse/feed"));
        let separators = |line: &str| {
            line.chars()
                .enumerate()
                .filter_map(|(position, character)| (character == '│').then_some(position))
                .collect::<Vec<_>>()
        };
        assert_eq!(separators(lines[1]), separators(lines[2]));
        assert_eq!(separators(lines[0]).len(), 4);
        assert_eq!(separators(lines[3]).len(), 4);
    }

    #[test]
    fn summary_subheaders_span_only_their_feeding_columns_at_every_scroll_position() {
        for start in 0..COLUMNS.len() {
            for end in start + 1..=COLUMNS.len() {
                let headings = heading_row(Units::Ml);
                let subgroup = subgroup_row(Units::Ml, start..end);
                let leaves = join_heading_cells(
                    "day",
                    &heading_cells(Units::Ml, start..end, None),
                    start..end,
                );
                assert_eq!(subgroup.chars().count(), leaves.chars().count());
                if start == 0 && end == COLUMNS.len() {
                    let bottle = subgroup.find("bottle feed").unwrap();
                    let nursing = subgroup.find("nursed").unwrap();
                    assert!(bottle > headings.find("feeds").unwrap());
                    assert!(bottle < headings.find("nursed").unwrap());
                    assert!(nursing > headings.find("milk ml/feed").unwrap());
                    assert!(nursing < headings.find("sleep").unwrap());
                }
            }
        }
    }

    #[test]
    fn summary_split_columns_plot_hours_from_their_own_measurements() {
        use crate::domain::fixtures::sleep;
        let mut data = dataset();
        let calendar = calendar();
        let at = |hour| calendar.at("2025-09-21".parse().unwrap(), hour, 0);
        data.sleep = vec![
            sleep(at(8), 3600.0),
            sleep(at(19), 7200.0),
            sleep(at(23), 10800.0),
        ];
        let rows = summaries::build(&data, &calendar, rule(), AFTERNOON, 2);
        for (heading, hours) in [
            ("day", 2.0),
            ("avg night nap", 3.0),
            ("avg day nap", 1.5),
            ("day wake", 11.0),
            ("avg night wake window", 2.0),
            ("avg day wake window", 10.0),
        ] {
            let column = COLUMNS
                .iter()
                .find(|column| column.heading(Units::Ml) == heading)
                .unwrap();
            assert_eq!(column.value(&rows[1], Units::Ml), Some(hours), "{heading}");
        }
    }

    #[test]
    fn summary_night_note_uses_configured_hours() {
        let mut data = dataset();
        data.child.night_start_hour = 19.5;
        data.child.morning_cutoff_hour = 6.25;
        let rendered = table(&data, 1);
        for value in ["7:30 pm", "6:15 am", "America/New_York"] {
            assert!(rendered.contains(value), "{value} missing: {rendered}");
        }
    }
}
