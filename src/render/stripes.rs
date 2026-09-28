//! The 24-hour stripe chart, in text.
//!
//! One row per day, each row a strip of cells across midnight-to-midnight.
//! Sleep fills a cell, a feed marks one, a diaper marks one, and the night
//! shows as a dimmer background. At a glance it answers the question a week of
//! numbers does not: *where* is the sleep actually landing.
//!
//! Positions come from `domain::stripes` as fractions of each day's own
//! length, so a 23-hour day still fills the strip exactly.

use crate::domain::stripes::StripeRow;
use crate::theme::Theme;

use super::format;

/// How many cells wide a day is drawn. Divides 24 evenly, so every cell is a
/// round number of minutes and the hour ruler lines up.
pub const CELLS: usize = 48;

/// What one cell of the strip is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    /// Nothing recorded, inside the night window.
    NightAwake,
    /// Nothing recorded, during the day.
    DayAwake,
    /// Asleep.
    Asleep,
    /// A feed happened in this cell.
    Feed,
    /// A diaper happened in this cell.
    Diaper,
    /// The part of today that has not happened yet.
    Unlived,
}

impl Cell {
    /// The character this cell draws as.
    #[must_use]
    pub const fn glyph(self) -> char {
        match self {
            Self::NightAwake => '·',
            Self::Asleep => '█',
            Self::Feed => '▼',
            Self::Diaper => '◦',
            // Nothing recorded during the day, and the part of today that has
            // not happened, both draw as blank. They are separate states
            // because they take different colours.
            Self::DayAwake | Self::Unlived => ' ',
        }
    }
}

/// One day's cells.
///
/// The precedence is deliberate: a feed drawn over sleep is the one that
/// matters, because a feed during a sleep block is the thing a parent is
/// looking for when they open this.
#[must_use]
pub fn cells(row: &StripeRow) -> Vec<Cell> {
    let mut strip = vec![Cell::DayAwake; CELLS];

    for (index, cell) in strip.iter_mut().enumerate() {
        let position = (index as f64 + 0.5) / CELLS as f64;
        if position < row.night_until || position >= row.night_from {
            *cell = Cell::NightAwake;
        }
    }

    for block in &row.sleep_blocks {
        let from = (block.from * CELLS as f64).floor() as usize;
        let to = (block.to * CELLS as f64).ceil() as usize;
        for cell in strip.iter_mut().take(to.min(CELLS)).skip(from.min(CELLS)) {
            *cell = Cell::Asleep;
        }
    }

    for tick in &row.diaper_ticks {
        if let Some(cell) = strip.get_mut(index_of(*tick)) {
            *cell = Cell::Diaper;
        }
    }

    for tick in &row.feed_ticks {
        if let Some(cell) = strip.get_mut(index_of(tick.at)) {
            *cell = Cell::Feed;
        }
    }

    if row.partial {
        let lived = (row.elapsed * CELLS as f64).ceil() as usize;
        for cell in strip.iter_mut().skip(lived.min(CELLS)) {
            *cell = Cell::Unlived;
        }
    }

    strip
}

fn index_of(position: f64) -> usize {
    ((position * CELLS as f64).floor() as usize).min(CELLS - 1)
}

/// The chart, oldest at the top.
#[must_use]
pub fn lines(rows: &[StripeRow], theme: Theme) -> Vec<String> {
    let mut lines = vec![
        theme.heading("Where sleep lands"),
        String::new(),
        theme.muted(&ruler()),
    ];
    for row in rows.iter().rev() {
        let strip: String = cells(row).into_iter().map(Cell::glyph).collect();
        let label = format::pad(&format::day_short(row.day), 11);
        let hours = format::pad_left(&format::hours(total_asleep(row)), 5);
        let line = format!("{label}│{strip}│{hours}h");
        lines.push(if row.partial {
            theme.today(&line)
        } else {
            theme.value(&line)
        });
    }
    lines.push(String::new());
    lines.push(theme.muted(&format!(
        "{}█ asleep   ▼ feed   ◦ diaper   · night",
        format::pad("", 12)
    )));
    lines
}

/// The hour ruler above the chart.
#[must_use]
pub fn ruler() -> String {
    let mut ruler = [' '; CELLS];
    for hour in (0..24).step_by(6) {
        let at = hour * CELLS / 24;
        for (offset, character) in format!("{hour:02}").chars().enumerate() {
            if let Some(cell) = ruler.get_mut(at + offset) {
                *cell = character;
            }
        }
    }
    format!(
        "{} {}",
        format::pad("", 11),
        ruler.iter().collect::<String>()
    )
}

/// How much of a row's strip is sleep, as seconds of that day.
fn total_asleep(row: &StripeRow) -> f64 {
    row.sleep_blocks
        .iter()
        .map(|block| (block.to - block.from) * 86_400.0)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::{AFTERNOON, bottle, dataset, diaper, sleep};
    use crate::domain::{Calendar, stripes};

    fn calendar() -> Calendar {
        Calendar::new("America/New_York").expect("a real timezone")
    }

    fn rows(data: &crate::domain::types::Dataset, days: usize) -> Vec<StripeRow> {
        stripes::build(data, &calendar(), AFTERNOON, days)
    }

    #[test]
    fn a_strip_is_always_the_full_width() {
        for row in rows(&dataset(), 3) {
            assert_eq!(cells(&row).len(), CELLS);
        }
    }

    #[test]
    fn the_night_shows_at_both_ends_of_a_day_and_not_in_the_middle() {
        let strip = cells(&rows(&dataset(), 2)[1]);
        assert_eq!(strip[0], Cell::NightAwake, "just after midnight is night");
        assert_eq!(strip[CELLS - 1], Cell::NightAwake, "just before is too");
        assert_eq!(strip[CELLS / 2], Cell::DayAwake, "noon is not");
    }

    #[test]
    fn a_sleep_fills_the_cells_it_covers() {
        let calendar = calendar();
        let mut data = dataset();
        let start = calendar.at("2025-09-21".parse().expect("a date"), 12, 0);
        data.sleep = vec![sleep(start, 3_600.0)];
        let strip = cells(&rows(&data, 2)[1]);
        assert_eq!(strip[CELLS / 2], Cell::Asleep);
    }

    #[test]
    fn a_feed_is_drawn_over_a_sleep_because_that_is_what_the_reader_is_looking_for() {
        let calendar = calendar();
        let mut data = dataset();
        let noon = calendar.at("2025-09-21".parse().expect("a date"), 12, 0);
        data.sleep = vec![sleep(noon, 3_600.0)];
        data.feeds = vec![bottle(noon + 60.0, 90.0)];
        assert_eq!(cells(&rows(&data, 2)[1])[CELLS / 2], Cell::Feed);
    }

    #[test]
    fn a_diaper_marks_its_cell() {
        let calendar = calendar();
        let mut data = dataset();
        let noon = calendar.at("2025-09-21".parse().expect("a date"), 12, 0);
        data.diapers = vec![diaper(noon, true, false)];
        assert_eq!(cells(&rows(&data, 2)[1])[CELLS / 2], Cell::Diaper);
    }

    #[test]
    fn the_rest_of_today_is_drawn_as_unlived_rather_than_as_awake() {
        // 2pm local, so the last third of today has not happened.
        let strip = cells(&rows(&dataset(), 1)[0]);
        assert_eq!(strip[CELLS - 1], Cell::Unlived);
        assert_ne!(strip[0], Cell::Unlived);
    }

    #[test]
    fn a_finished_day_has_no_unlived_cells_at_all() {
        let strip = cells(&rows(&dataset(), 2)[1]);
        assert!(!strip.contains(&Cell::Unlived));
    }

    #[test]
    fn the_ruler_marks_every_sixth_hour() {
        let ruler = ruler();
        for hour in ["00", "06", "12", "18"] {
            assert!(ruler.contains(hour), "`{hour}` missing from `{ruler}`");
        }
    }

    #[test]
    fn the_chart_runs_oldest_at_the_top_and_carries_a_legend() {
        let drawn = lines(&rows(&dataset(), 3), Theme::dark(false)).join("\n");
        let days: Vec<&str> = drawn.lines().filter(|line| line.contains('│')).collect();
        assert!(days.first().expect("a row").contains("20 Sep"), "{drawn}");
        assert!(drawn.contains("asleep"), "{drawn}");
    }
}
