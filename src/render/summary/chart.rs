//! Separated daily bars and a dotted mean of the previous seven complete days.

use std::fmt::Write as _;

use crate::{cli::Units, domain::summaries::DaySummary, theme::Theme};

use super::columns::Column;

/// Chronological values preserve missing days as empty bar slots.
pub(super) fn values(rows: &[DaySummary], column: &Column, units: Units) -> Vec<Option<f64>> {
    rows.iter()
        .rev()
        .map(|row| column.value(row, units).filter(|value| value.is_finite()))
        .collect()
}

/// Rows include today first; fix the preceding seven-day window before excluding gaps.
fn average(rows: &[DaySummary], column: &Column, units: Units) -> Option<(f64, usize)> {
    let values: Vec<_> = rows
        .iter()
        .skip(1)
        .take(7)
        .filter(|row| !row.partial)
        .filter(|row| row.has_data)
        .filter_map(|row| column.value(row, units))
        .filter(|value| value.is_finite())
        .collect();
    (!values.is_empty()).then(|| {
        (
            values.iter().sum::<f64>() / values.len() as f64,
            values.len(),
        )
    })
}

pub(super) fn lines(
    rows: &[DaySummary],
    column: &Column,
    units: Units,
    size: (u16, u16),
    theme: Theme,
) -> Vec<String> {
    let values = values(rows, column, units);
    let average = average(rows, column, units);
    let height = usize::from(size.1.max(1));
    let mut lines = vec![theme.heading(&column.title(units))];
    let plot_width = usize::from(size.0).saturating_sub(8);
    if values.iter().all(Option::is_none) {
        lines.push(theme.muted("No recorded values for this column"));
    } else if height < 3 || plot_width < rows.len() * 2 {
        lines.push(theme.muted("Enlarge the panel or show fewer days for spaced bars"));
    } else {
        let plot_height = height.saturating_sub(if height >= 5 { 3 } else { 2 });
        lines.extend(plot(
            &values,
            average.map(|mean| mean.0),
            plot_width,
            plot_height,
            theme,
        ));
        lines.push(theme.muted(&day_labels(rows, plot_width)));
    }
    if height >= 5 {
        let legend = average.map_or_else(
            || "· 7-day avg unavailable: no recorded complete days".to_owned(),
            |(mean, count)| format!("· 7-day avg: {mean:.1} ({count}/7 days; today excluded)"),
        );
        lines.push(theme.muted(&legend));
    }
    lines.resize(height, String::new());
    lines
}

fn plot(
    values: &[Option<f64>],
    average: Option<f64>,
    width: usize,
    height: usize,
    theme: Theme,
) -> Vec<String> {
    let maximum = values
        .iter()
        .flatten()
        .copied()
        .fold(0.0, f64::max)
        .max(0.1);
    let average_row = average
        .map(|mean| (((1.0 - mean / maximum) * height as f64).floor() as usize).min(height - 1));
    (0..height)
        .map(|row| {
            let mut cells = bar_row(values, maximum, width, height, row);
            if average_row == Some(row) {
                for column in (0..width).step_by(2) {
                    cells[column] = '·';
                }
            }
            let label = if row == 0 {
                format!("{maximum:.1}")
            } else if row == height - 1 {
                "0".to_owned()
            } else {
                String::new()
            };
            theme.accent(&format!("{label:>7}│{}", cells.iter().collect::<String>()))
        })
        .collect()
}

fn bar_row(
    values: &[Option<f64>],
    maximum: f64,
    width: usize,
    height: usize,
    row: usize,
) -> Vec<char> {
    let mut cells = vec![' '; width];
    for (index, value) in values.iter().enumerate() {
        let Some(value) = value else { continue };
        let filled = (value / maximum * height as f64 - (height - row - 1) as f64).clamp(0.0, 1.0);
        let eighths = (filled * 8.0).round() as usize;
        let symbol = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'][eighths];
        let start = index * width / values.len();
        let end = (index + 1) * width / values.len() - 1;
        cells[start..end].fill(symbol);
    }
    cells
}

fn day_labels(rows: &[DaySummary], width: usize) -> String {
    let mut labels = String::from("        ");
    for (index, row) in rows.iter().rev().enumerate() {
        let slot = (index + 1) * width / rows.len() - index * width / rows.len();
        let format = if slot >= 7 { "%b %d" } else { "%d" };
        let label = row.day.strftime(format).to_string();
        let _ = write!(labels, "{label:^slot$}");
    }
    labels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn average_dots_align_with_bars_of_the_same_height_even_near_zero() {
        let lines = plot(
            &[Some(100.0), Some(900.0)],
            Some(100.0),
            20,
            6,
            Theme::dark(false),
        );
        let dotted_row = lines.iter().position(|line| line.contains('·')).unwrap();
        assert_eq!(
            dotted_row, 5,
            "100/900 fills only the bottom row of a six-row plot"
        );
        assert!(lines[dotted_row].contains('▅') || lines[dotted_row].contains('▆'));
    }
    #[test]
    fn unavailable_days_never_pull_older_values_into_the_seven_day_average() {
        use crate::domain::{
            Calendar,
            fixtures::{AFTERNOON, bottle, dataset},
            summaries,
            today::DayRule,
        };
        let calendar = Calendar::utc();
        let mut data = dataset();
        data.feeds = vec![
            bottle(AFTERNOON - 86400.0, 60.0),
            bottle(AFTERNOON - 8.0 * 86400.0, 900.0),
        ];
        let mut rows = summaries::build(&data, &calendar, DayRule::default(), AFTERNOON, 10);
        assert_eq!(
            average(&rows, &super::super::columns::COLUMNS[1], Units::Ml),
            Some((60.0, 1))
        );
        rows[1].partial = true;
        assert_eq!(
            average(&rows, &super::super::columns::COLUMNS[1], Units::Ml),
            None
        );
    }
}
