//! Width-based placement of the Now sections, shared by stdout and the drawer.

use ratatui::style::Style;
use ratatui::text::Span;

use super::{GAP, LABEL, Row};
use crate::listing::Piece;
use crate::theme::Tone;

/// Three cells keep neighboring sections visually independent.
const GUTTER: usize = 3;

/// Available widths after the gutters have been reserved.
pub(super) enum Columns {
    Stacked(Option<usize>),
    Two([usize; 2]),
}

impl Columns {
    /// Each side-by-side section needs at least 46 cells to remain readable.
    pub(super) const fn for_width(width: Option<usize>) -> Self {
        match width {
            Some(width) if width >= 96 => {
                let usable = width - GUTTER;
                Self::Two([usable / 2, usable - usable / 2])
            }
            _ => Self::Stacked(width),
        }
    }

    /// The tables wrap their cells before the sections are joined.
    pub(super) const fn table_width(&self) -> Option<usize> {
        match self {
            Self::Stacked(width) => *width,
            Self::Two(widths) => Some(widths[1]),
        }
    }

    /// Reading order is facts, tables, ranges; two columns place ranges left.
    pub(super) fn arrange(
        self,
        facts: Vec<Row>,
        mut tables: Vec<Row>,
        ranges: Vec<Row>,
        freshness: Vec<Row>,
    ) -> Vec<Row> {
        match self {
            Self::Stacked(width) => {
                let rows = stack(stack(stack(facts, freshness), tables), ranges);
                match width {
                    Some(width) => wrap_rows(rows, width),
                    None => rows,
                }
            }
            Self::Two(widths) => {
                let left = wrap_rows(stack(facts, ranges), widths[0]);
                let freshness = wrap_rows(freshness, widths[1]);
                let footer_start = left.len().saturating_sub(freshness.len());
                tables.resize(footer_start.max(tables.len() + 1), Row::new());
                tables.extend(freshness);
                beside(&[left, tables], &widths)
            }
        }
    }
}

fn stack(mut first: Vec<Row>, second: Vec<Row>) -> Vec<Row> {
    if !second.is_empty() {
        first.push(Row::new());
        first.extend(second);
    }
    first
}

fn beside(columns: &[Vec<Row>], widths: &[usize]) -> Vec<Row> {
    let height = columns.iter().map(Vec::len).max().unwrap_or_default();
    (0..height)
        .map(|index| {
            let mut row = Row::new();
            for (column, rows) in columns.iter().enumerate() {
                let cells = rows.get(index).cloned().unwrap_or_default();
                let occupied = plain_width(&cells);
                row.extend(cells);
                if column + 1 < columns.len() {
                    row.push(Piece::new(
                        " ".repeat(widths[column].saturating_sub(occupied)),
                        Tone::Value,
                    ));
                    row.push(Piece::new(" │ ", Tone::Muted));
                }
            }
            row
        })
        .collect()
}

fn wrap_rows(rows: Vec<Row>, width: usize) -> Vec<Row> {
    rows.into_iter().flat_map(|row| wrap(row, width)).collect()
}

/// Label/value rows keep hanging values; narrow screens put values underneath.
fn wrap(row: Row, width: usize) -> Vec<Row> {
    if width == 0 {
        return Vec::new();
    }
    if plain_width(&row) <= width {
        return vec![row];
    }
    if let [label, value] = row.as_slice() {
        return wrap_fact(label, value, width);
    }
    row.into_iter()
        .flat_map(|piece| {
            wrap_words(&piece.text, width)
                .into_iter()
                .map(move |text| vec![Piece::new(text, piece.tone)])
        })
        .collect()
}

/// Keep complete fact details together whenever they fit the value column.
fn wrap_fact(label: &Piece, value: &Piece, width: usize) -> Vec<Row> {
    let indent = if width >= 40 {
        LABEL + GAP
    } else {
        2.min(width / 2)
    };
    let mut values: Vec<Row> = wrap_details(&value.text, width - indent)
        .into_iter()
        .map(|text| {
            vec![
                Piece::new(" ".repeat(indent), label.tone),
                Piece::new(text, value.tone),
            ]
        })
        .collect();
    if width >= 40 {
        values[0][0] = label.clone();
    } else if !label.text.trim().is_empty() {
        let mut labels = wrap_words(label.text.trim(), width)
            .into_iter()
            .map(|text| vec![Piece::new(text, label.tone)])
            .collect::<Vec<_>>();
        labels.extend(values);
        return labels;
    }
    values
}

fn wrap_details(text: &str, width: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    for detail in text.split(" · ") {
        let line = lines.last_mut().expect("a line exists");
        if !line.is_empty() && text_width(line) + 3 + text_width(detail) > width {
            lines.push(format!("· {detail}"));
        } else {
            if !line.is_empty() {
                line.push_str(" · ");
            }
            line.push_str(detail);
        }
    }
    lines
        .into_iter()
        .flat_map(|line| wrap_words(&line, width))
        .collect()
}

/// Word wrapping also splits an overlong token at whole Unicode graphemes.
pub(super) fn wrap_words(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = vec![String::new()];
    for word in text.split_whitespace() {
        let line = lines.last_mut().expect("a line exists");
        if !line.is_empty() && text_width(line) + 1 + text_width(word) > width {
            lines.push(String::new());
        } else if !line.is_empty() {
            line.push(' ');
        }
        for grapheme in Span::raw(word).styled_graphemes(Style::default()) {
            let line = lines.last_mut().expect("a line exists");
            if !line.is_empty() && text_width(line) + text_width(grapheme.symbol) > width {
                lines.push(grapheme.symbol.to_owned());
            } else {
                line.push_str(grapheme.symbol);
            }
        }
    }
    lines
}

fn plain_width(row: &Row) -> usize {
    row.iter().map(|piece| text_width(&piece.text)).sum()
}

fn text_width(text: &str) -> usize {
    Span::raw(text).width()
}
