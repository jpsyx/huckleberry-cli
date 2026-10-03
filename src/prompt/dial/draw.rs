//! A dial as lines of text. No terminal, no clock, no idea what it counts.

use super::model::Dial;
use super::wheel::Wheel;
use crate::theme::{Theme, Tone};

/// How many values show either side of the one chosen.
///
/// Two. One reads as a typo and three fills a panel; two is enough for the
/// column to read as a wheel that carries on past both ends.
const REACH: isize = 2;

/// How far the dial sits from the edge.
const INDENT: usize = 2;

/// What the keys do, said once under the dial.
const HINTS: &str = "←/→ h/l column · ↑/↓ j/k turn · shift leaps · Enter · Esc";

/// The dial, ready to draw.
///
/// The rows themselves are never clipped: they are a few columns wide, and
/// clipping painted text counts escape codes as though they were letters.
/// Only the label and the hint, which are prose, are cut to fit.
#[must_use]
pub fn render(label: &str, dial: &Dial, width: u16, theme: Theme) -> Vec<String> {
    let width = usize::from(width).saturating_sub(1).max(1);
    let fit = |text: &str| crate::prompt::select::clip(text, width);
    let mut lines = vec![theme.prompt(&fit(label)), String::new()];
    lines.extend((-REACH..=REACH).map(|offset| row(dial, offset, theme)));
    lines.push(theme.paint(Tone::Selected, &marker(dial)));
    lines.push(String::new());
    lines.push(theme.muted(&fit(HINTS)));
    lines
}

/// One row across every column, `offset` places from the chosen values.
fn row(dial: &Dial, offset: isize, theme: Theme) -> String {
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
    let mut line = " ".repeat(INDENT);
    for (index, wheel) in dial.wheels.iter().enumerate() {
        if index > 0 {
            line.push_str(&theme.paint(between, wheel.gap()));
        }
        let cell = format!("{:>1$}", shown(wheel, offset), wheel.width());
        line.push_str(&theme.paint(tone(dial.column == index), &cell));
    }
    line
}

/// What a column shows `offset` places along.
///
/// A wheel of two has nothing to spin: the one not chosen sits directly above
/// the one that is, and the rest of the column stays empty. Drawn as a wheel
/// it would repeat itself every other row, which reads as a blur rather than
/// as a choice between two things.
fn shown(wheel: &Wheel, offset: isize) -> &str {
    if wheel.count() <= 2 && offset != 0 && offset != -1 {
        return "";
    }
    wheel.at(offset)
}

/// The rule under whichever column the keys are turning.
///
/// Colour says it too, but never alone: this survives a screenshot, a pipe
/// and colour blindness, which is the rule everywhere else in this tool.
fn marker(dial: &Dial) -> String {
    let before: usize = dial
        .wheels
        .iter()
        .take(dial.column)
        .enumerate()
        .map(|(index, wheel)| wheel.width() + if index > 0 { wheel.gap().len() } else { 0 })
        .sum();
    let lead = dial
        .wheels
        .get(dial.column)
        .filter(|_| dial.column > 0)
        .map_or(0, |wheel| wheel.gap().len());
    let under = dial.wheels.get(dial.column).map_or(1, Wheel::width);
    format!(
        "{}{}",
        " ".repeat(INDENT + before + lead),
        "\u{2500}".repeat(under)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock() -> Dial {
        Dial::new(vec![
            Wheel::new((1..=12).map(|hour| hour.to_string()).collect(), 0, 1, ""),
            Wheel::new(
                (0..60).map(|minute| format!("{minute:02}")).collect(),
                44,
                5,
                " : ",
            ),
            Wheel::new(vec!["am".into(), "pm".into()], 1, 1, "   "),
        ])
    }

    fn plain(dial: &Dial) -> Vec<String> {
        render("When?", dial, 60, Theme::dark(false))
    }

    fn rows_of(lines: &[String]) -> Vec<String> {
        lines
            .iter()
            .filter(|line| line.contains(" : "))
            .cloned()
            .collect()
    }

    #[test]
    fn two_values_show_either_side_of_the_one_chosen() {
        let rows = rows_of(&plain(&clock()));
        assert_eq!(rows.len(), 5, "{rows:?}");
        assert!(rows[2].contains(" 1 "), "{rows:?}");
        assert!(rows[2].contains("44"), "{rows:?}");
    }

    #[test]
    fn every_column_cycles_so_the_wheel_never_runs_out() {
        let rows = rows_of(&plain(&clock()));
        assert!(rows[0].contains("11") && rows[0].contains("42"), "{rows:?}");
        assert!(rows[1].contains("12") && rows[1].contains("43"), "{rows:?}");
        assert!(
            rows[3].contains(" 2 ") && rows[3].contains("45"),
            "{rows:?}"
        );
        assert!(
            rows[4].contains(" 3 ") && rows[4].contains("46"),
            "{rows:?}"
        );
    }

    /// Two values, so there is one to show and it goes next to the other.
    #[test]
    fn a_column_of_two_shows_only_the_other_one_and_only_beside_it() {
        let rows = rows_of(&plain(&clock()));
        assert!(rows[2].contains("pm"), "{rows:?}");
        assert!(rows[1].contains("am"), "{rows:?}");
        for row in [&rows[0], &rows[3], &rows[4]] {
            assert!(!row.contains("am") && !row.contains("pm"), "{row}");
        }
    }

    #[test]
    fn the_column_under_the_cursor_is_marked_in_words_as_well_as_colour() {
        let mut dial = clock();
        let under = |dial: &Dial| {
            plain(dial)
                .into_iter()
                .find(|line| line.contains('\u{2500}'))
                .unwrap_or_else(|| panic!("no marker row"))
        };
        let first = under(&dial);
        dial.column = 1;
        let second = under(&dial);
        dial.column = 2;
        let third = under(&dial);
        let start = |line: &str| line.find('\u{2500}').expect("a marker");
        assert!(start(&first) < start(&second), "{first}|{second}");
        assert!(start(&second) < start(&third), "{second}|{third}");
    }

    /// The rule is as wide as the column it sits under, so a two-character
    /// wheel is not marked by a one-character line.
    #[test]
    fn the_marker_is_as_wide_as_the_column_it_marks() {
        let mut dial = clock();
        dial.column = 1;
        let marker = plain(&dial)
            .into_iter()
            .find(|line| line.contains('\u{2500}'))
            .expect("a marker row");
        assert_eq!(marker.matches('\u{2500}').count(), 2);
    }

    #[test]
    fn the_far_neighbours_are_fainter_than_the_near_ones() {
        let rows = rows_of(&render("When?", &clock(), 60, Theme::dark(true)));
        assert!(rows[0].contains(Tone::Faint.sgr()), "{:?}", rows[0]);
        assert!(rows[1].contains(Tone::Muted.sgr()), "{:?}", rows[1]);
        assert!(rows[2].contains(Tone::Selected.sgr()), "{:?}", rows[2]);
        assert!(rows[4].contains(Tone::Faint.sgr()), "{:?}", rows[4]);
    }

    #[test]
    fn only_the_column_under_the_cursor_is_lit() {
        let mut dial = clock();
        dial.column = 1;
        let centre = render("When?", &dial, 60, Theme::dark(true))
            .into_iter()
            .find(|line| line.contains("44"))
            .expect("a centre row");
        assert_eq!(
            centre.matches(Tone::Selected.sgr()).count(),
            1,
            "{centre:?}"
        );
    }

    #[test]
    fn the_keys_are_said_under_the_dial() {
        let lines = plain(&clock());
        let hint = lines.last().expect("a hint line");
        for key in ["h/l", "j/k", "Enter", "Esc"] {
            assert!(hint.contains(key), "{hint}");
        }
    }

    /// Two columns, different widths: the general case the clock does not
    /// exercise.
    #[test]
    fn columns_of_different_widths_still_line_up() {
        let dial = Dial::new(vec![
            Wheel::new(vec!["1.00".into(), "1.25".into()], 0, 1, ""),
            Wheel::new(vec!["ml".into(), "oz".into()], 1, 1, "   "),
        ]);
        let lines = plain(&dial);
        let centre = lines
            .iter()
            .find(|line| line.contains("1.00"))
            .expect("a centre row");
        assert!(centre.contains("1.00   oz"), "{centre:?}");
    }
}
