//! Separated daily bars and a dotted mean of the previous seven complete days.

use std::fmt::Write as _;

use crate::{
    cli::Units,
    domain::summaries::DaySummary,
    theme::{Theme, Tone},
};

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
            || "··· 7-day avg unavailable: no recorded complete days".to_owned(),
            |(mean, _)| format!("··· 7-day avg: {mean:.1}"),
        );
        lines.push(theme.paint(Tone::Average, &legend));
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
            let cells = bar_row(values, maximum, width, height, row);
            let painted_cells = paint_cells(&cells, average_row == Some(row), theme);
            let scale_label = if row == 0 {
                format!("{maximum:.1}")
            } else if row == height - 1 {
                "0".to_owned()
            } else {
                String::new()
            };
            let label = average.filter(|_| average_row == Some(row)).map_or_else(
                || theme.accent(&format!("{scale_label:>7}")),
                |mean| theme.paint(Tone::Average, &format!("{mean:>7.1}")),
            );
            format!("{label}{}{painted_cells}", theme.accent("│"))
        })
        .collect()
}

/// Paint dots independently so their background ends at the bar's edge.
fn paint_cells(cells: &[char], is_average_row: bool, theme: Theme) -> String {
    cells
        .iter()
        .map(|symbol| {
            if is_average_row {
                let tone = if *symbol == ' ' {
                    Tone::Average
                } else {
                    Tone::AverageOverBar
                };
                theme.paint(tone, "·")
            } else if *symbol != ' ' {
                theme.paint(Tone::SummaryBar, &symbol.to_string())
            } else {
                theme.accent(&symbol.to_string())
            }
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

    fn styled_cells(line: &str) -> Vec<(char, ratatui::style::Style)> {
        crate::tui::draw::painted(line)
            .spans
            .iter()
            .flat_map(|span| span.content.chars().map(|symbol| (symbol, span.style)))
            .collect()
    }

    #[test]
    fn average_dots_fill_every_plot_column() {
        let lines = plot(
            &[Some(8.0), Some(2.0), None],
            Some(4.0),
            15,
            4,
            Theme::dark(false),
        );
        assert_eq!(lines[2], "    4.0│···············");
    }

    #[test]
    fn average_quantity_matches_the_line_color_even_at_scale_edges() {
        for average in [0.0, 3.9, 8.0] {
            let lines = plot(&[Some(8.0)], Some(average), 10, 4, Theme::dark(true));
            let line = lines.iter().find(|line| line.contains('·')).unwrap();
            let cells = styled_cells(line);
            let label: String = cells[..7].iter().map(|cell| cell.0).collect();
            assert_eq!(label.trim(), format!("{average:.1}"));
            for (_, style) in &cells[..7] {
                assert_eq!(style.fg, cells[8].1.fg);
                assert_eq!(style.bg, None);
            }
        }
    }

    #[test]
    fn average_dots_contrast_with_bars_and_keep_their_background() {
        // The average crosses a full bar, empty space, and a missing day.
        let lines = plot(
            &[Some(8.0), Some(2.0), None],
            Some(4.0),
            15,
            4,
            Theme::dark(true),
        );
        let cells = styled_cells(&lines[2]);
        let (dot, over_bar) = cells[8];
        let (block, bar) = styled_cells(&lines[1])[9];
        let (gap_dot, gap) = cells[12];
        let (missing_dot, missing) = cells[18];
        assert_eq!((dot, block, gap_dot, missing_dot), ('·', '█', '·', '·'));
        assert_ne!(over_bar.fg, bar.fg, "dots must contrast with the bars");
        assert_eq!(over_bar.bg, bar.fg, "dots must not punch holes in bars");
        assert_eq!(gap.fg, over_bar.fg);
        assert_eq!(missing.fg, over_bar.fg);
        assert_eq!(gap.bg, None, "gaps must retain the terminal background");
        assert_eq!(missing.bg, None);
        assert_eq!(
            bar.bg, bar.fg,
            "bar cells must fill the same background as dots"
        );
    }

    #[test]
    fn average_dots_keep_the_background_of_partial_bar_cells() {
        let lines = plot(&[Some(1.0), Some(9.0)], Some(1.0), 10, 6, Theme::dark(true));
        let cells = styled_cells(&lines[5]);
        assert_eq!(cells[8].0, '·');
        assert_eq!(cells[9].0, '·');
        assert_eq!(
            cells[8].1.bg,
            Some(crate::tui::draw::tone(Tone::SummaryBar))
        );
        assert_eq!(cells[9].1.bg, cells[8].1.bg, "the bar top must stay level");
    }

    #[test]
    fn bars_fill_their_cells_even_without_an_average_line() {
        let lines = plot(
            &[Some(1.0), Some(9.0), None],
            None,
            15,
            6,
            Theme::dark(true),
        );
        let cells = styled_cells(&lines[5]);
        for column in [8, 9, 10, 11, 13, 14, 15, 16] {
            assert_eq!(cells[column].1.bg, cells[column].1.fg);
        }
        for column in [12, 17, 18, 19, 20, 21, 22] {
            assert_eq!(cells[column].0, ' ');
            assert_eq!(cells[column].1.bg, None, "empty cells must stay empty");
        }
        assert_eq!(styled_cells(&lines[0])[8].1.bg, None);
    }

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
        assert_eq!(
            lines[dotted_row]
                .chars()
                .filter(|symbol| *symbol == '·')
                .count(),
            20
        );
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
