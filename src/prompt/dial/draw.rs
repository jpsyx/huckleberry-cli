//! The dial as lines of text. No terminal, no clock.

use super::model::{Column, Dial};
use crate::theme::{Theme, Tone};

/// How many values show either side of the one chosen.
///
/// Two. One reads as a typo and three fills a panel; two is enough for the
/// column to read as a wheel that carries on past both ends.
const REACH: i16 = 2;

/// How wide every cell is, so the three columns line up whatever is in them.
const CELL: usize = 2;

/// What sits between the hour and the minute: the clock's own separator.
const GAP: &str = " : ";

/// What sits between the minute and the meridiem.
///
/// Plain space, not another colon. Three of the five rows have no meridiem to
/// show, and a colon with nothing after it reads as a value that failed to
/// load. Written as a constant of the same width so the columns still line up
/// and the rule underneath still lands where it should.
const WIDE_GAP: &str = "   ";

/// How far the dial sits from the edge.
const INDENT: usize = 2;

/// What the keys do, said once under the dial.
const HINTS: &str = "←/→ h/l dial · ↑/↓ j/k turn · shift ±1 min · Enter · Esc";

/// The dial, ready to draw.
///
/// The rows themselves are never clipped: they are sixteen columns wide at
/// most, and clipping painted text counts escape codes as though they were
/// letters. Only the label and the hint, which are prose, are cut to fit.
#[must_use]
pub fn render(label: &str, dial: &Dial, width: u16, theme: Theme) -> Vec<String> {
    let width = usize::from(width).saturating_sub(1).max(1);
    let fit = |text: &str| crate::prompt::select::clip(text, width);
    let mut lines = vec![theme.prompt(&fit(label)), String::new()];
    lines.extend((-REACH..=REACH).map(|offset| row(*dial, offset, theme)));
    lines.push(theme.paint(Tone::Selected, &marker(dial.column)));
    lines.push(String::new());
    lines.push(theme.muted(&fit(HINTS)));
    lines
}

/// One row of all three dials, `offset` places from the chosen values.
fn row(dial: Dial, offset: i16, theme: Theme) -> String {
    // Two steps of quiet, so the far values read as the wheel carrying on
    // rather than as more numbers to choose between.
    let tone = |lit: bool| match (offset, lit) {
        (0, true) => Tone::Selected,
        (0, false) => Tone::Value,
        (-1 | 1, _) => Tone::Muted,
        _ => Tone::Faint,
    };
    let between = if offset == 0 {
        Tone::Muted
    } else {
        Tone::Faint
    };
    let cell = |column: Column, text: &str| theme.paint(tone(dial.column == column), text);
    format!(
        "{}{}{}{}{}{}",
        " ".repeat(INDENT),
        cell(Column::Hour, &format!("{:>CELL$}", dial.hour_at(offset))),
        theme.paint(between, GAP),
        cell(
            Column::Minute,
            &format!("{:0CELL$}", dial.minute_at(offset))
        ),
        theme.paint(between, WIDE_GAP),
        cell(Column::Meridiem, &meridiem_at(dial, offset)),
    )
}

/// What the meridiem column shows `offset` places along.
///
/// Two values and no more, so the one not chosen sits directly above the one
/// that is, and the rest of the column is empty. A wheel of two would spin
/// past itself.
fn meridiem_at(dial: Dial, offset: i16) -> String {
    match offset {
        0 => dial.meridiem.label().to_owned(),
        -1 => dial.meridiem.flipped().label().to_owned(),
        _ => " ".repeat(CELL),
    }
}

/// The two gaps are the same width, which is what lets one stride place
/// every column and the rule beneath them.
const _: () = assert!(GAP.len() == WIDE_GAP.len());

/// The rule under whichever dial the keys are turning.
///
/// Colour says it too, but never alone: this survives a screenshot, a pipe
/// and colour blindness, which is the rule everywhere else in this tool.
fn marker(column: Column) -> String {
    let index = Column::ALL
        .iter()
        .position(|candidate| *candidate == column)
        .unwrap_or_default();
    format!(
        "{}{}",
        " ".repeat(INDENT + index * (CELL + GAP.len())),
        "\u{2500}".repeat(CELL)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::clock::TimeOfDay;
    use crate::prompt::dial::model::Meridiem;

    fn at(hour: i8, minute: i8) -> Dial {
        Dial::new(TimeOfDay::new(hour, minute).expect("a time"))
    }

    fn plain(dial: Dial) -> Vec<String> {
        render("When?", &dial, 60, Theme::dark(false))
    }

    /// The rows between the label and the hint, which is where the dial is.
    ///
    /// Found by the separator rather than by looking for digits: a painted
    /// line is full of digits, because escape codes are made of them.
    fn rows_of(lines: &[String]) -> Vec<String> {
        lines
            .iter()
            .filter(|line| line.contains(GAP))
            .cloned()
            .collect()
    }

    fn wheel(dial: Dial) -> Vec<String> {
        rows_of(&plain(dial))
    }

    #[test]
    fn two_values_show_either_side_of_the_one_chosen() {
        let rows = wheel(at(13, 44));
        assert_eq!(rows.len(), 5, "{rows:?}");
        assert!(
            rows[2].contains(" 1 "),
            "the chosen hour is the middle one: {rows:?}"
        );
        assert!(rows[2].contains("44"), "{rows:?}");
    }

    #[test]
    fn the_hours_cycle_so_the_wheel_never_runs_out() {
        let rows = wheel(at(12, 30));
        assert!(rows[3].contains(" 1 "), "after twelve comes one: {rows:?}");
        assert!(rows[4].contains(" 2 "), "{rows:?}");
        assert!(rows[0].contains("10"), "{rows:?}");
        assert!(rows[1].contains("11"), "{rows:?}");
    }

    #[test]
    fn the_minutes_cycle_too() {
        let rows = wheel(at(1, 59));
        assert!(rows[2].contains("59"), "{rows:?}");
        assert!(rows[3].contains("00"), "and then round again: {rows:?}");
        assert!(rows[4].contains("01"), "{rows:?}");
        assert!(rows[0].contains("57"), "{rows:?}");
    }

    /// Two values, so there is one to show and it goes next to the other.
    #[test]
    fn the_meridiem_shows_only_the_other_one_and_only_beside_it() {
        let rows = wheel(at(1, 0));
        assert!(
            rows[2].contains("am"),
            "the chosen one is the middle: {rows:?}"
        );
        assert!(
            rows[1].contains("pm"),
            "and the other sits above it: {rows:?}"
        );
        for row in [&rows[0], &rows[3], &rows[4]] {
            assert!(!row.contains("am") && !row.contains("pm"), "{row}");
        }
    }

    #[test]
    fn the_dial_under_the_cursor_is_marked_in_words_as_well_as_colour() {
        let mut dial = at(1, 44);
        let under = |dial: Dial| {
            plain(dial)
                .into_iter()
                .find(|line| line.contains('\u{2500}'))
                .unwrap_or_else(|| panic!("no marker row"))
        };
        let hour = under(dial);
        dial.column = Column::Minute;
        let minute = under(dial);
        dial.column = Column::Meridiem;
        let meridiem = under(dial);
        let start = |line: &str| line.find('\u{2500}').expect("a marker");
        assert!(start(&hour) < start(&minute), "{hour}|{minute}");
        assert!(start(&minute) < start(&meridiem), "{minute}|{meridiem}");
    }

    /// The far neighbours recede twice over, so the column reads as a wheel
    /// turning away rather than as a list of five numbers.
    #[test]
    fn the_far_neighbours_are_fainter_than_the_near_ones() {
        let rows = rows_of(&render("When?", &at(1, 44), 60, Theme::dark(true)));
        assert!(rows[0].contains(Tone::Faint.sgr()), "{:?}", rows[0]);
        assert!(rows[1].contains(Tone::Muted.sgr()), "{:?}", rows[1]);
        assert!(rows[2].contains(Tone::Selected.sgr()), "{:?}", rows[2]);
        assert!(rows[3].contains(Tone::Muted.sgr()), "{:?}", rows[3]);
        assert!(rows[4].contains(Tone::Faint.sgr()), "{:?}", rows[4]);
    }

    /// Only the dial being turned is lit: the other two are still values.
    #[test]
    fn the_columns_not_under_the_cursor_are_not_highlighted() {
        let mut dial = at(1, 44);
        dial.column = Column::Minute;
        let lines = render("When?", &dial, 60, Theme::dark(true));
        let centre = lines
            .iter()
            .find(|line| line.contains("44"))
            .expect("a centre row");
        let lit = centre.match_indices(Tone::Selected.sgr()).count();
        assert_eq!(lit, 1, "exactly one cell is lit: {centre:?}");
    }

    #[test]
    fn the_keys_are_said_under_the_dial() {
        let lines = plain(at(1, 0));
        let hint = lines.last().expect("a hint line");
        for key in ["h/l", "j/k", "Enter", "Esc"] {
            assert!(hint.contains(key), "{hint}");
        }
    }

    #[test]
    fn the_meridiem_reads_as_the_dial_says_it() {
        assert_eq!(Meridiem::Am.label(), "am");
        assert_eq!(Meridiem::Pm.label(), "pm");
    }
}
