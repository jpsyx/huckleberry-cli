//! Small, readable command receipts. Machine payloads remain a separate channel.
use super::format;
use crate::{config::Config, domain::Calendar, theme::Theme};

/// A labelled value in a receipt or status table.
pub type Fields = Vec<(&'static str, String)>;

/// Chooses the terminal presentation or the original pipe payload.
#[must_use]
pub fn select(terminal: bool, human: Vec<String>, machine: &[String]) -> Vec<String> {
    if terminal { human } else { machine.to_vec() }
}

/// A short duration that does not round a sub-minute session down to zero.
#[must_use]
pub fn duration(seconds: f64) -> String {
    if seconds <= 0.0 {
        return "0m".into();
    }
    if seconds < 60.0 {
        format!("{:.0}s", seconds.ceil())
    } else {
        crate::domain::time::format_duration(seconds)
    }
}

/// Recorded sleep, including the date on both sides of midnight.
#[must_use]
pub fn sleep_fields(start: f64, seconds: f64, calendar: &Calendar) -> Fields {
    vec![
        ("Started", format::date_time(start, calendar)),
        ("Ended", format::date_time(start + seconds, calendar)),
        ("Duration", duration(seconds)),
    ]
}

/// Start context before stopping a sleep; age includes time spent paused.
#[must_use]
pub fn sleep_start_context(
    timer: Option<&huckleberry_api::models::SleepTimer>,
    now: f64,
    calendar: &Calendar,
) -> Option<String> {
    let started = timer.filter(|timer| timer.active)?.started_at()?;
    Some(format!(
        "Sleep started at {} ({} ago).",
        format::date_time(started, calendar),
        duration((now - started).max(0.0))
    ))
}

/// Sleep status without stale start times from inactive timers.
#[must_use]
pub fn sleep_status_fields(
    timer: Option<&huckleberry_api::models::SleepTimer>,
    last: Option<f64>,
    now: f64,
    calendar: &Calendar,
) -> Fields {
    let Some(timer) = timer else {
        return vec![("Status", "No sleep in progress".into())];
    };
    let state = if !timer.active {
        "Awake"
    } else if timer.paused {
        "Paused"
    } else {
        "Sleeping"
    };
    let mut fields = vec![("Status", state.into())];
    if timer.active
        && let Some(started) = timer.started_at()
    {
        fields.push(("Started", format::date_time(started, calendar)));
    }
    if let Some(seconds) = timer.elapsed(now) {
        fields.push(("Elapsed", duration(seconds)));
    } else if let Some(seconds) = last {
        fields.push(("Last sleep", duration(seconds)));
    }
    fields
}

/// Nursing totals use words for both sides and units on every value.
#[must_use]
pub fn nursing_fields(left: f64, right: f64) -> Fields {
    vec![
        ("Total", duration(left + right)),
        ("Left side", duration(left)),
        ("Right side", duration(right)),
    ]
}

/// Technical enum spellings as words. Use only for enum values, never names or IDs.
#[must_use]
pub fn words(value: &str) -> String {
    let mut spaced = String::new();
    let mut previous_lowercase = false;
    for character in value.chars() {
        if character.is_uppercase() && previous_lowercase {
            spaced.push(' ');
        }
        if character == '_' || character == '-' {
            spaced.push(' ');
        } else {
            spaced.extend(character.to_lowercase());
        }
        previous_lowercase = character.is_lowercase();
    }
    let mut characters = spaced.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + characters.as_str()
    })
}

/// Settings with explicit units and absence labels.
#[must_use]
pub fn settings_fields(config: &Config) -> Fields {
    vec![
        ("Child", config.child().unwrap_or("Not selected").into()),
        ("Timezone", config.timezone.clone()),
        ("History window", format!("{} days", config.days)),
        (
            "Bottle units",
            if config.units == "oz" {
                "Fluid ounces (oz)"
            } else {
                "Millilitres (ml)"
            }
            .into(),
        ),
        (
            "Measurements",
            if config.measurements == "imperial" {
                "Pounds and inches"
            } else {
                "Kilograms and centimetres"
            }
            .into(),
        ),
        ("Refresh interval", format!("{}s", config.refresh)),
        (
            "Detailed output",
            if config.verbose { "On" } else { "Off" }.into(),
        ),
    ]
}

/// A child's profile with dates, age and local night boundaries.
#[must_use]
pub fn profile_fields(
    child: &crate::domain::types::Child,
    calendar: &Calendar,
    now: f64,
) -> Fields {
    let birthday = child
        .birthdate
        .as_deref()
        .and_then(|date| date.parse::<jiff::civil::Date>().ok())
        .map_or_else(
            || "Not recorded".into(),
            |date| date.strftime("%b %d, %Y").to_string(),
        );
    let mut fields = vec![
        ("Name", child.name.clone()),
        ("Child ID", child.cid.clone()),
        ("Birthday", birthday),
    ];
    if let Some(age) = child
        .birthdate
        .as_deref()
        .and_then(|birthdate| calendar.age_in_days(birthdate, now))
    {
        fields.push((
            "Age",
            format!("{} weeks, {} days ({age} days)", age / 7, age % 7),
        ));
    }
    let today = calendar.day_of(now);
    for (label, hour) in [
        ("Night begins", child.night_start_hour),
        ("Morning begins", child.morning_cutoff_hour),
    ] {
        fields.push((
            label,
            format::clock(calendar.at_hour_fraction(today, hour), calendar),
        ));
    }
    fields.push(("Timezone", calendar.name().into()));
    fields
}

/// A weight in the chosen system, labelled with the date it was measured.
#[must_use]
pub fn weight_fields(kilograms: f64, measured: f64, imperial: bool, calendar: &Calendar) -> Fields {
    let weight = if imperial {
        format!("{:.2} lb", kilograms / 0.453_592_37)
    } else {
        format!("{kilograms:.3} kg")
    };
    vec![
        ("Weight", weight),
        ("Weight measured", format::date_time(measured, calendar)),
    ]
}

/// A table measured in terminal cells, including emoji and wide characters.
#[must_use]
pub fn table(title: &str, headers: &[&str], rows: &[Vec<String>], theme: Theme) -> Vec<String> {
    let headings: Vec<String> = headers.iter().map(|text| clean(text)).collect();
    let rows: Vec<Vec<String>> = rows
        .iter()
        .map(|row| row.iter().map(|text| clean(text)).collect())
        .collect();
    let widths: Vec<usize> = headings
        .iter()
        .enumerate()
        .map(|(index, heading)| {
            rows.iter()
                .filter_map(|row| row.get(index))
                .map(|cell| width(cell))
                .fold(width(heading), usize::max)
        })
        .collect();
    let mut lines = vec![
        theme.heading(title),
        String::new(),
        theme.muted(&border(&widths, '┌', '┬', '┐')),
        theme.accent(&row_line(&headings, &widths)),
        theme.muted(&border(&widths, '├', '┼', '┤')),
    ];
    for row in &rows {
        lines.push(theme.value(&row_line(row, &widths)));
    }
    lines.push(theme.muted(&border(&widths, '└', '┴', '┘')));
    if rows.is_empty() {
        lines.push(theme.muted("Nothing to show."));
    }
    lines
}

/// On a narrow terminal, stacks labelled records instead of letting columns wrap.
#[must_use]
pub fn table_with_width(
    title: &str,
    headers: &[&str],
    rows: &[Vec<String>],
    theme: Theme,
    available: usize,
) -> Vec<String> {
    let plain = table(title, headers, rows, Theme::dark(false));
    if plain.iter().all(|line| width(line) <= available) {
        return table(title, headers, rows, theme);
    }
    let mut lines: Vec<String> = wrap(&clean(title), available)
        .into_iter()
        .map(|line| theme.heading(&line))
        .collect();
    for row in rows {
        lines.push(String::new());
        let labelled: Vec<String> = if headers == ["Detail", "Value"] && row.len() == 2 {
            vec![format!("{}: {}", clean(&row[0]), clean(&row[1]))]
        } else {
            headers
                .iter()
                .zip(row)
                .map(|(header, cell)| format!("{}: {}", clean(header), clean(cell)))
                .collect()
        };
        for text in labelled {
            lines.extend(
                wrap(&text, available)
                    .into_iter()
                    .map(|line| theme.value(&line)),
            );
        }
    }
    if rows.is_empty() {
        lines.push(theme.muted("Nothing to show."));
    }
    lines
}

fn wrap(text: &str, available: usize) -> Vec<String> {
    let available = available.max(2);
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && width(&line) + 1 + width(word) > available {
            lines.push(core::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        for character in word.chars() {
            let candidate = format!("{line}{character}");
            if width(&candidate) > available {
                lines.push(core::mem::take(&mut line));
            }
            line.push(character);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn clean(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}

fn width(text: &str) -> usize {
    ratatui::text::Line::raw(text).width()
}

fn border(widths: &[usize], left: char, middle: char, right: char) -> String {
    let parts: Vec<String> = widths.iter().map(|width| "─".repeat(width + 2)).collect();
    format!("{left}{}{right}", parts.join(&middle.to_string()))
}

fn row_line(cells: &[String], widths: &[usize]) -> String {
    let parts: Vec<String> = widths
        .iter()
        .enumerate()
        .map(|(index, &column_width)| {
            let cell = cells.get(index).map_or("", String::as_str);
            format!(
                " {cell}{} ",
                " ".repeat(column_width.saturating_sub(width(cell)))
            )
        })
        .collect();
    format!("│{}│", parts.join("│"))
}
