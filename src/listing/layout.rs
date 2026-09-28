//! Turning rows into lines, at whatever width there is.
//!
//! One pass over every filtered row decides the column widths, so the rows
//! under one heading line up with the rows under every other. When the
//! terminal is too narrow for that, the columns share out what there is:
//! narrow columns keep their natural width and the greedy ones split the rest,
//! so one very long description cannot push the time and the kind off the
//! screen.
//!
//! Everything here is pure, and a line is kept as its pieces rather than as a
//! painted string, so the same layout serves the plain output and the
//! full-screen one.

use crate::theme::{Theme, Tone};

use super::model::{Column, Group, Row};

/// The two spaces every line starts with, so the cursor's marker has somewhere
/// to go without shifting the columns.
pub const GUTTER: &str = "  ";
/// What replaces the gutter on the row under the cursor.
pub const MARKER: &str = "› ";
/// The gap between two columns.
const GAP: &str = "  ";

/// One painted piece of a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    /// The text.
    pub text: String,
    /// What to paint it.
    pub tone: Tone,
}

impl Piece {
    /// A piece of text in one tone.
    #[must_use]
    pub fn new(text: impl Into<String>, tone: Tone) -> Self {
        Self {
            text: text.into(),
            tone,
        }
    }
}

/// One line of a rendered listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// What it is made of.
    pub pieces: Vec<Piece>,
    /// Which row of the filtered list it is, when it is one. A heading and a
    /// note are lines that are not rows.
    pub row: Option<usize>,
}

impl Line {
    /// A line of pieces that is not a row.
    #[must_use]
    pub const fn text(pieces: Vec<Piece>) -> Self {
        Self { pieces, row: None }
    }

    /// The line without any colour, which is what the cursor's bar is drawn
    /// from and what a test reads.
    #[must_use]
    pub fn plain(&self) -> String {
        self.pieces
            .iter()
            .map(|piece| piece.text.as_str())
            .collect()
    }

    /// The line as one painted string.
    #[must_use]
    pub fn painted(&self, theme: Theme) -> String {
        self.pieces
            .iter()
            .map(|piece| theme.paint(piece.tone, &piece.text))
            .collect()
    }
}

/// Shortens text to `max` characters, marking the cut.
///
/// Counts characters rather than bytes, so a cut never lands inside one.
fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut shortened: String = text.chars().take(max.saturating_sub(1)).collect();
    shortened.push('…');
    shortened
}

/// The natural width of each column: its widest cell, heading included.
#[must_use]
pub fn natural_widths(columns: &[Column], rows: &[&Row]) -> Vec<usize> {
    columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            rows.iter()
                .filter_map(|row| row.cells.get(index))
                .map(|cell| cell.chars().count())
                .chain(std::iter::once(column.header.chars().count()))
                .max()
                .unwrap_or(0)
        })
        .collect()
}

/// What each column is allowed, at this width.
///
/// The budget is handed out starting with the narrowest column, so whatever a
/// column does not need is left for the wider ones behind it. A layout that
/// already fits is left exactly as it is, which is the common case.
#[must_use]
pub fn widths(columns: &[Column], rows: &[&Row], width: Option<usize>) -> Vec<usize> {
    let natural = natural_widths(columns, rows);
    let Some(width) = width else {
        return natural;
    };
    let count = columns.len();
    if count == 0 {
        return natural;
    }
    let budget = width.saturating_sub(GUTTER.chars().count() + GAP.chars().count() * (count - 1));
    if natural.iter().sum::<usize>() <= budget {
        return natural;
    }

    let mut order: Vec<usize> = (0..count).collect();
    order.sort_by_key(|&column| natural[column]);
    let mut allowed = vec![0; count];
    let mut left = budget;
    for (given, &column) in order.iter().enumerate() {
        let share = left / (count - given);
        allowed[column] = natural[column].min(share).max(1);
        left = left.saturating_sub(allowed[column]);
    }
    allowed
}

/// One row's cells, padded and shortened to the widths, as pieces.
fn row_pieces(columns: &[Column], row: &Row, widths: &[usize]) -> Vec<Piece> {
    let mut pieces = vec![Piece::new(GUTTER, Tone::Muted)];
    for (index, column) in columns.iter().enumerate() {
        if index > 0 {
            pieces.push(Piece::new(GAP, Tone::Muted));
        }
        let cell = row.cells.get(index).map_or("", String::as_str);
        let cell = shorten(cell, widths[index]);
        // The last column is free-form, so it is never padded: trailing space
        // is invisible on a screen and noise in a pipe.
        let text = if index + 1 == columns.len() {
            cell
        } else {
            format!("{cell:<width$}", width = widths[index])
        };
        pieces.push(Piece::new(text, column.role.tone(row.tone)));
    }
    pieces
}

/// The heading line above a group.
fn heading_line(text: &str, today: bool) -> Line {
    Line::text(vec![Piece::new(
        text.to_owned(),
        if today { Tone::Today } else { Tone::Heading },
    )])
}

/// The column headings.
#[must_use]
pub fn header_line(columns: &[Column], widths: &[usize]) -> Line {
    let mut pieces = vec![Piece::new(GUTTER, Tone::Muted)];
    for (index, column) in columns.iter().enumerate() {
        if index > 0 {
            pieces.push(Piece::new(GAP, Tone::Muted));
        }
        let text = if index + 1 == columns.len() {
            column.header.to_owned()
        } else {
            format!("{:<width$}", column.header, width = widths[index])
        };
        pieces.push(Piece::new(text, Tone::Heading));
    }
    Line::text(pieces)
}

/// Every line of the body: a heading per group, then its rows, each followed by
/// its note.
///
/// `heading` renders a group's key into the words above it, and `today` says
/// which heading is the one somebody is looking for.
#[must_use]
pub fn body_lines(
    columns: &[Column],
    groups: &[Group<'_>],
    widths: &[usize],
    heading: &dyn Fn(&str) -> String,
    today: &dyn Fn(&str) -> bool,
) -> Vec<Line> {
    let mut lines = Vec::new();
    for (position, group) in groups.iter().enumerate() {
        if !group.key.is_empty() {
            if position > 0 {
                lines.push(Line::text(Vec::new()));
            }
            lines.push(heading_line(&heading(&group.key), today(&group.key)));
        }
        for &(place, row) in &group.rows {
            let mut line = Line {
                pieces: row_pieces(columns, row, widths),
                row: Some(place),
            };
            if !row.selectable {
                // A row nothing can be done with is drawn back rather than
                // left out: the stream is the stream.
                for piece in &mut line.pieces {
                    piece.tone = Tone::Muted;
                }
            }
            lines.push(line);
            if let Some(note) = &row.note {
                lines.push(Line::text(vec![Piece::new(
                    format!("{GUTTER}{GUTTER}{note}"),
                    Tone::Muted,
                )]));
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::listing::model::{Role, filter, grouped};

    const COLUMNS: [Column; 3] = [
        Column::new("when", Role::Key),
        Column::new("what", Role::Kind),
        Column::new("detail", Role::Value),
    ];

    fn rows() -> Vec<Row> {
        vec![
            Row::new("a", ["10:32 pm", "Diaper", "pee · big"].map(str::to_owned))
                .group("Sun 27 Sep"),
            Row::new("b", ["7:00 pm", "Bottle", "34 ml"].map(str::to_owned)).group("Sun 27 Sep"),
        ]
    }

    fn lines_of(rows: &[Row], width: Option<usize>) -> Vec<Line> {
        let filtered = filter(rows, "");
        let widths = widths(&COLUMNS, &filtered, width);
        body_lines(
            &COLUMNS,
            &grouped(&filtered),
            &widths,
            &str::to_owned,
            &|_| false,
        )
    }

    #[test]
    fn a_column_is_as_wide_as_its_widest_cell_or_its_heading() {
        let rows = rows();
        let filtered = filter(&rows, "");
        let widths = natural_widths(&COLUMNS, &filtered);
        assert_eq!(widths[0], "10:32 pm".len());
        assert_eq!(widths[1], "Diaper".len());
    }

    #[test]
    fn every_group_shares_one_set_of_widths_so_the_rows_line_up() {
        let mut rows = rows();
        rows.push(
            Row::new("c", ["9:00 am", "Sleep", "slept 2h"].map(str::to_owned)).group("Sat 26 Sep"),
        );
        let lines = lines_of(&rows, None);
        let starts: Vec<usize> = lines
            .iter()
            .filter(|line| line.row.is_some())
            .map(|line| line.plain().find("  ").unwrap_or(0))
            .collect();
        assert!(
            starts.windows(2).all(|pair| pair[0] == pair[1]),
            "the columns start in the same place in every group: {starts:?}"
        );
    }

    #[test]
    fn a_narrow_terminal_shares_the_width_out_rather_than_losing_a_column() {
        let rows = vec![Row::new(
            "a",
            [
                "10:32 pm",
                "Diaper",
                "a description far longer than any terminal would like to show",
            ]
            .map(str::to_owned),
        )];
        let filtered = filter(&rows, "");
        let widths = widths(&COLUMNS, &filtered, Some(40));
        assert!(widths.iter().all(|width| *width > 0), "{widths:?}");
        assert!(widths.iter().sum::<usize>() <= 40, "{widths:?}");
        let lines = lines_of(&rows, Some(40));
        let text = lines[0].plain();
        assert!(text.contains('…'), "the cut is marked: {text}");
        assert!(text.contains("Diaper"), "and the narrow column survives");
    }

    #[test]
    fn a_layout_that_fits_is_left_alone() {
        let rows = rows();
        let filtered = filter(&rows, "");
        assert_eq!(
            widths(&COLUMNS, &filtered, Some(200)),
            natural_widths(&COLUMNS, &filtered)
        );
    }

    #[test]
    fn a_note_is_its_own_line_under_the_row_it_belongs_to() {
        let rows = vec![
            Row::new("a", ["10:32 pm", "Diaper", "pee"].map(str::to_owned))
                .note(Some("a bit sore".to_owned())),
        ];
        let lines = lines_of(&rows, None);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].row, Some(0));
        assert_eq!(lines[1].row, None, "a note is not a row to land on");
        assert!(lines[1].plain().contains("a bit sore"));
    }

    #[test]
    fn a_row_that_cannot_be_chosen_is_drawn_back() {
        let rows = vec![
            Row::new("a", ["10:32 pm", "Pumping", "59 ml"].map(str::to_owned))
                .refused("this tool does not log a pumping session"),
        ];
        let lines = lines_of(&rows, None);
        assert!(
            lines[0]
                .pieces
                .iter()
                .all(|piece| piece.tone == Tone::Muted),
            "it is listed, and it is not what the eye goes to"
        );
    }

    #[test]
    fn the_last_column_is_never_padded() {
        let rows = rows();
        let lines = lines_of(&rows, None);
        for line in lines.iter().filter(|line| line.row.is_some()) {
            let text = line.plain();
            assert_eq!(text.trim_end(), text, "trailing space in `{text}`");
        }
    }
}
