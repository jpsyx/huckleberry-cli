//! Rasterize exact straight segments before applying series patterns and markers.

use crate::{
    domain::cumulative::{Comparison, DayCurve, Point},
    theme::{Theme, Tone},
};

#[derive(Clone, Copy)]
struct Cell {
    symbol: char,
    tone: Tone,
}

pub(super) struct Plot {
    cells: Vec<Vec<Cell>>,
    width: usize,
    height: usize,
    label_width: usize,
    duration: f64,
    maximum: f64,
    scale: f64,
}

impl Plot {
    pub(super) fn create(comparison: &Comparison, scale: f64, width: usize, height: usize) -> Self {
        let maximum = maximum(comparison, scale);
        let label_width = quantity(maximum).len().max(4) + 1;
        let plot_width = width.saturating_sub(label_width + 1).max(2);
        let duration = std::iter::once(comparison.today.duration)
            .chain(comparison.selected.iter().map(|day| day.duration))
            .chain(comparison.average.last().map(|point| point.seconds))
            .fold(1.0, f64::max);
        let blank = Cell {
            symbol: ' ',
            tone: Tone::Muted,
        };
        let mut result = Self {
            cells: vec![vec![blank; plot_width]; height],
            width: plot_width,
            height,
            label_width,
            duration,
            maximum,
            scale,
        };
        result.draw(comparison);
        result
    }

    fn draw(&mut self, comparison: &Comparison) {
        self.curve(&comparison.average, self.duration, Tone::Average);
        if let Some(day) = comparison.selected.as_ref().filter(|day| day.has_data) {
            self.curve(&day.points, day.observed, Tone::Accent);
        }
        if comparison.today.has_data {
            self.curve(
                &comparison.today.points,
                comparison.today.observed,
                Tone::Today,
            );
        }
        for (day, tone) in comparison
            .selected
            .iter()
            .map(|day| (day, Tone::Accent))
            .chain(std::iter::once((&comparison.today, Tone::Today)))
        {
            self.markers(day, tone);
        }
    }

    pub(super) fn lines(&self, start_hour: f64, is_elapsed: bool, theme: Theme) -> Vec<String> {
        let mut output: Vec<_> = self
            .cells
            .iter()
            .enumerate()
            .map(|(row, cells)| {
                let label = if row == 0 || row == self.height / 2 || row == self.height - 1 {
                    quantity(
                        self.maximum * (self.height - row - 1) as f64 / (self.height - 1) as f64,
                    )
                } else {
                    String::new()
                };
                let prefix = format!("{label:>width$}|", width = self.label_width);
                format!("{}{}", theme.muted(&prefix), paint_cells(cells, theme))
            })
            .collect();
        let prefix = " ".repeat(self.label_width);
        output.push(theme.muted(&format!("{prefix}+{}", "-".repeat(self.width))));
        output.push(theme.muted(&format!(
            "{prefix} {}",
            self.time_labels(start_hour, is_elapsed)
        )));
        output
    }

    fn curve(&mut self, points: &[Point], cutoff: f64, tone: Tone) {
        if let Some(point) = points.first().filter(|point| point.seconds <= cutoff) {
            let (column, row) = self.position(*point);
            self.stroke(column, row, '-', tone);
        }
        for pair in points.windows(2) {
            let start = pair[0];
            if start.seconds > cutoff {
                break;
            }
            let mut end = pair[1];
            if end.seconds > cutoff && end.seconds > start.seconds {
                let fraction = (cutoff - start.seconds) / (end.seconds - start.seconds);
                end.value = start.value + (end.value - start.value) * fraction;
                end.seconds = cutoff;
            }
            self.segment(start, end, tone);
        }
    }

    fn segment(&mut self, start: Point, end: Point, tone: Tone) {
        let (start_column, start_row) = self.position(start);
        let (end_column, end_row) = self.position(end);
        let horizontal = start_column.abs_diff(end_column);
        let vertical = start_row.abs_diff(end_row);
        let steps = horizontal.max(vertical).max(1);
        let symbol = if horizontal == 0 && vertical > 0 {
            '|'
        } else if vertical == 0 {
            '-'
        } else {
            '/'
        };
        for step in 0..=steps {
            let fraction = step as f64 / steps as f64;
            let column = (start_column as f64
                + (end_column as f64 - start_column as f64) * fraction)
                .round() as usize;
            let row = (start_row as f64 + (end_row as f64 - start_row as f64) * fraction).round()
                as usize;
            self.stroke(column, row, symbol, tone);
        }
    }

    fn stroke(&mut self, column: usize, row: usize, symbol: char, tone: Tone) {
        if tone == Tone::Accent && (column + row) % 4 == 3 {
            return;
        }
        let symbol = if tone == Tone::Average { '.' } else { symbol };
        self.cells[row][column] = Cell { symbol, tone };
    }

    fn markers(&mut self, day: &DayCurve, tone: Tone) {
        if !day.has_data {
            return;
        }
        for marker in day
            .markers
            .iter()
            .filter(|marker| marker.point.seconds <= day.observed)
        {
            let (column, row) = self.position(marker.point);
            self.cells[row][column] = Cell {
                symbol: marker.symbol,
                tone,
            };
        }
    }

    fn position(&self, point: Point) -> (usize, usize) {
        let column = (point.seconds / self.duration).clamp(0.0, 1.0) * (self.width - 1) as f64;
        let row = (1.0 - point.value * self.scale / self.maximum).clamp(0.0, 1.0)
            * (self.height - 1) as f64;
        (column.round() as usize, row.round() as usize)
    }

    fn time_labels(&self, start_hour: f64, is_elapsed: bool) -> String {
        let mut cells = vec![' '; self.width];
        let divisions = if self.width >= 48 { 4 } else { 2 };
        for index in 0..=divisions {
            let elapsed = self.duration / 3600.0 * f64::from(index) / f64::from(divisions);
            let label = if is_elapsed {
                format!("+{}h", quantity(elapsed))
            } else {
                super::clock_label(start_hour + elapsed)
            };
            let center = self.width.saturating_sub(1) * index as usize / divisions as usize;
            let start = center
                .saturating_sub(label.len() / 2)
                .min(self.width - label.len());
            for (offset, character) in label.chars().enumerate() {
                cells[start + offset] = character;
            }
        }
        cells.into_iter().collect()
    }
}

fn maximum(comparison: &Comparison, scale: f64) -> f64 {
    let largest = comparison
        .average
        .iter()
        .chain(
            std::iter::once(&comparison.today)
                .chain(comparison.selected.iter())
                .filter(|day| day.has_data)
                .flat_map(|day| {
                    day.points
                        .iter()
                        .filter(|point| point.seconds <= day.observed)
                }),
        )
        .map(|point| point.value * scale)
        .filter(|value| value.is_finite())
        .fold(0.0, f64::max)
        .max(0.1);
    let magnitude = 10.0_f64.powf(largest.log10().floor());
    let factor = [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .find(|factor| largest <= magnitude * factor)
        .unwrap_or(10.0);
    magnitude * factor
}

fn quantity(value: f64) -> String {
    if value.abs() >= 10000.0 {
        format!("{value:.1e}")
    } else if value.fract().abs() < 0.001 {
        format!("{value:.0}")
    } else {
        format!("{value:.1}")
    }
}

fn paint_cells(cells: &[Cell], theme: Theme) -> String {
    let mut output = String::new();
    let mut start = 0;
    while start < cells.len() {
        let tone = cells[start].tone;
        let length = cells[start..]
            .iter()
            .take_while(|cell| cell.tone == tone)
            .count();
        let text: String = cells[start..start + length]
            .iter()
            .map(|cell| cell.symbol)
            .collect();
        output.push_str(&theme.paint(tone, &text));
        start += length;
    }
    output
}
