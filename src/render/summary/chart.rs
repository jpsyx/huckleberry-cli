//! A Ratatui line chart that leaves gaps where nothing was measured.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    symbols::Marker,
    widgets::{Axis, Chart, Dataset, GraphType, Widget},
};

use crate::{cli::Units, domain::summaries::DaySummary, theme::Theme};

use super::columns::Column;

pub(super) fn segments(rows: &[DaySummary], column: &Column, units: Units) -> Vec<Vec<(f64, f64)>> {
    let mut segments = Vec::new();
    let mut segment = Vec::new();
    for (index, row) in rows.iter().rev().enumerate() {
        if let Some(value) = column.value(row, units).filter(|value| value.is_finite()) {
            segment.push((index as f64, value));
        } else if !segment.is_empty() {
            segments.push(std::mem::take(&mut segment));
        }
    }
    if !segment.is_empty() {
        segments.push(segment);
    }
    segments
}

pub(super) fn lines(
    rows: &[DaySummary],
    column: &Column,
    units: Units,
    size: (u16, u16),
    theme: Theme,
) -> Vec<String> {
    let segments = segments(rows, column, units);
    let height = size.1.max(1);
    let mut lines = vec![theme.heading(&column.title(units))];
    if segments.is_empty() || size.0 < 20 || height < 4 {
        lines.push(theme.muted(if segments.is_empty() {
            "No recorded values for this column"
        } else {
            "Enlarge the panel to show the graph"
        }));
    } else {
        lines.extend(plot(rows, &segments, (size.0, height - 1), theme));
    }
    lines.resize(usize::from(height), String::new());
    lines
}

fn plot(
    rows: &[DaySummary],
    segments: &[Vec<(f64, f64)>],
    size: (u16, u16),
    theme: Theme,
) -> Vec<String> {
    let maximum = segments
        .iter()
        .flatten()
        .map(|point| point.1)
        .fold(0.0, f64::max)
        .max(0.1);
    let datasets = segments
        .iter()
        .map(|segment| {
            Dataset::default()
                .marker(Marker::Braille)
                .graph_type(GraphType::Line)
                .data(segment)
        })
        .collect();
    let first = day_label(rows.last());
    let last = day_label(rows.first());
    let chart = Chart::new(datasets)
        .x_axis(
            Axis::default()
                .bounds([0.0, rows.len().saturating_sub(1).max(1) as f64])
                .labels([first, last]),
        )
        .y_axis(Axis::default().bounds([0.0, maximum]).labels([
            "0".to_owned(),
            format!("{:.1}", maximum / 2.0),
            format!("{maximum:.1}"),
        ]));
    let area = Rect::new(0, 0, size.0, size.1);
    let mut buffer = Buffer::empty(area);
    chart.render(area, &mut buffer);
    (0..size.1)
        .map(|row| {
            let line = (0..size.0)
                .map(|column| buffer[(column, row)].symbol())
                .collect::<String>();
            theme.accent(line.trim_end())
        })
        .collect()
}

fn day_label(row: Option<&DaySummary>) -> String {
    row.map_or_else(String::new, |row| row.day.strftime("%b %d").to_string())
}
