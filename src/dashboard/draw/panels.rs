//! One function per tab.
//!
//! What each shows is chosen for a person holding a baby: the largest thing on
//! the Now tab is how long since the last feed, and the stripe chart is a
//! picture rather than a table because at 3am a picture is faster.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Sparkline, Table};

use crate::cli::Units;
use crate::dashboard::state::State;
use crate::domain::time::{format_ago, format_duration};
use crate::domain::{log, now, stripes, summaries};
use crate::render::format;
use crate::render::stripes::{Cell, cells};
use crate::theme::Tone;

use super::{bordered, tone};

/// The Now tab: the four facts, large, and a week of sleep beneath them.
pub(super) fn draw_now(frame: &mut Frame, area: Rect, state: &State, units: Units, at: f64) {
    let view = now::build(&state.dataset, &state.calendar, at);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(4)])
        .split(area);

    let mut lines = Vec::new();
    lines.push(big(
        "Last fed",
        &view.last_feed.as_ref().map_or_else(
            || "nothing logged".to_owned(),
            |last| format_ago(last.start, at),
        ),
        &view.last_feed.as_ref().map_or_else(String::new, |last| {
            last.feed.millilitres().map_or_else(
                || {
                    last.feed
                        .nursing_seconds()
                        .map_or_else(String::new, format_duration)
                },
                |millilitres| format::volume(Some(millilitres), units),
            )
        }),
    ));
    lines.push(big(
        "Nappy",
        &view.last_diaper.as_ref().map_or_else(
            || "nothing logged".to_owned(),
            |last| format_ago(last.start, at),
        ),
        view.last_diaper.as_ref().map_or("", |last| last.label()),
    ));
    lines.push(big(
        if view.sleep_state.asleep {
            "Asleep"
        } else {
            "Awake"
        },
        &view
            .sleep_state
            .asleep_seconds
            .or(view.sleep_state.awake_seconds)
            .map_or_else(|| "nothing logged".to_owned(), format_duration),
        if view.sleep_state.paused {
            "timer paused"
        } else {
            ""
        },
    ));
    let stretch_label = crate::render::now::stretch_label(&view);
    lines.push(big(
        &stretch_label,
        &view
            .longest_stretch
            .seconds
            .map_or_else(|| "no sleep yet".to_owned(), format_duration),
        "longest stretch",
    ));
    if let Some(nursing) = &view.nursing_now {
        lines.push(big(
            "Nursing now",
            &format_duration(nursing.elapsed_seconds),
            &format!("on the {}", nursing.side),
        ));
    }

    frame.render_widget(Paragraph::new(lines).block(bordered("Now")), rows[0]);
    draw_stripe_chart(frame, rows[1], state, at, 7);
}

/// One large fact: a label, a number, and a qualifier.
fn big<'a>(label: &'a str, value: &str, note: &str) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!(" {} ", format::pad(label, 14)),
            Style::default().fg(tone(Tone::Accent)),
        ),
        Span::styled(
            format::pad(value, 16),
            Style::default()
                .fg(tone(Tone::Value))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(note.to_owned(), Style::default().fg(tone(Tone::Muted))),
    ])
}

/// The Sleep tab: the stripe chart at full size.
pub(super) fn draw_sleep(frame: &mut Frame, area: Rect, state: &State, at: f64) {
    draw_stripe_chart(frame, area, state, at, state.days);
}

/// The stripe chart, as a block of rows.
pub(super) fn draw_stripe_chart(
    frame: &mut Frame,
    area: Rect,
    state: &State,
    at: f64,
    days: usize,
) {
    let rows = stripes::build(&state.dataset, &state.calendar, at, days);
    let mut lines = vec![Line::from(Span::styled(
        crate::render::stripes::ruler(),
        Style::default().fg(tone(Tone::Muted)),
    ))];

    for row in rows.iter().rev() {
        let mut spans = vec![Span::styled(
            format!("{} ", format::pad(&format::day_short(row.day), 11)),
            Style::default().fg(tone(Tone::Accent)),
        )];
        for cell in cells(row) {
            spans.push(Span::styled(
                cell.glyph().to_string(),
                Style::default().fg(cell_colour(cell)),
            ));
        }
        spans.push(Span::styled(
            format!(
                " {}h",
                format::hours(
                    row.sleep_blocks
                        .iter()
                        .map(|block| (block.to - block.from) * 86_400.0)
                        .sum()
                )
            ),
            Style::default().fg(tone(Tone::Value)),
        ));
        lines.push(Line::from(spans));
    }

    lines.push(Line::from(Span::styled(
        "            █ asleep   ▼ feed   ◦ nappy   · night",
        Style::default().fg(tone(Tone::Muted)),
    )));
    frame.render_widget(
        Paragraph::new(lines).block(bordered("Where sleep lands")),
        area,
    );
}

/// The colour a stripe cell paints as.
#[must_use]
pub const fn cell_colour(cell: Cell) -> Color {
    match cell {
        Cell::Asleep => tone(Tone::Info),
        Cell::Feed => tone(Tone::Success),
        Cell::Diaper => tone(Tone::Warning),
        Cell::NightAwake | Cell::DayAwake | Cell::Unlived => tone(Tone::Muted),
    }
}

/// The Feeding tab: a day table and a sparkline of daily milk.
pub(super) fn draw_feeding(frame: &mut Frame, area: Rect, state: &State, units: Units, at: f64) {
    let rows = summaries::build(&state.dataset, &state.calendar, at, state.days);
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(6), Constraint::Length(3)])
        .split(area);

    let unit = if units == Units::Oz { "oz" } else { "ml" };
    let header = Row::new(vec![
        "day".to_owned(),
        "feeds".to_owned(),
        format!("milk {unit}"),
        format!("formula {unit}"),
        format!("breast {unit}"),
        "nursed".to_owned(),
    ])
    .style(Style::default().fg(tone(Tone::Heading)));

    let table_rows: Vec<Row<'_>> = rows
        .iter()
        .map(|row| {
            Row::new(vec![
                format::day_short(row.day),
                format::count(row.feed_count),
                format::volume_bare((row.total_ml > 0.0).then_some(row.total_ml), units),
                format::volume_bare((row.formula_ml > 0.0).then_some(row.formula_ml), units),
                format::volume_bare(
                    (row.breast_milk_ml > 0.0).then_some(row.breast_milk_ml),
                    units,
                ),
                format::duration(row.nursing_seconds),
            ])
            .style(Style::default().fg(tone(if row.partial {
                Tone::Today
            } else {
                Tone::Value
            })))
        })
        .collect();

    frame.render_widget(
        Table::new(
            table_rows,
            [
                Constraint::Length(12),
                Constraint::Length(6),
                Constraint::Length(10),
                Constraint::Length(12),
                Constraint::Length(11),
                Constraint::Length(8),
            ],
        )
        .header(header)
        .block(bordered("Feeding")),
        areas[0],
    );

    let series: Vec<u64> = rows
        .iter()
        .rev()
        .map(|row| row.total_ml.max(0.0).round() as u64)
        .collect();
    frame.render_widget(
        Sparkline::default()
            .data(&series)
            .style(Style::default().fg(tone(Tone::Success)))
            .block(bordered(&format!("Milk per day ({unit} by bottle)"))),
        areas[1],
    );
}

/// The Nappies tab: counts per day, with a sparkline of wet nappies.
pub(super) fn draw_nappies(frame: &mut Frame, area: Rect, state: &State, at: f64) {
    let rows = summaries::build(&state.dataset, &state.calendar, at, state.days);
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(6), Constraint::Length(3)])
        .split(area);

    let header = Row::new(vec!["day", "wet", "dirty", "total", "rash"])
        .style(Style::default().fg(tone(Tone::Heading)));
    let table_rows: Vec<Row<'_>> = rows
        .iter()
        .map(|row| {
            Row::new(vec![
                format::day_short(row.day),
                format::count(row.wet_count),
                format::count(row.dirty_count),
                format::count(row.diaper_count),
                format::count(row.rash_count),
            ])
            .style(Style::default().fg(tone(if row.partial {
                Tone::Today
            } else {
                Tone::Value
            })))
        })
        .collect();

    frame.render_widget(
        Table::new(
            table_rows,
            [
                Constraint::Length(12),
                Constraint::Length(6),
                Constraint::Length(7),
                Constraint::Length(7),
                Constraint::Length(6),
            ],
        )
        .header(header)
        .block(bordered("Nappies")),
        areas[0],
    );

    let series: Vec<u64> = rows.iter().rev().map(|row| row.wet_count as u64).collect();
    frame.render_widget(
        Sparkline::default()
            .data(&series)
            .style(Style::default().fg(tone(Tone::Info)))
            .block(bordered("Wet nappies per day")),
        areas[1],
    );
}

/// The Log tab: everything, newest first, scrollable.
pub(super) fn draw_log(frame: &mut Frame, area: Rect, state: &State) {
    let entries = log::build(&state.dataset);
    if entries.is_empty() {
        frame.render_widget(
            Paragraph::new("nothing logged in this window")
                .style(Style::default().fg(tone(Tone::Muted)))
                .block(bordered("Log")),
            area,
        );
        return;
    }

    let visible = area.height.saturating_sub(2) as usize;
    let top = state.scroll.min(entries.len().saturating_sub(1));
    let lines: Vec<Line<'_>> = entries
        .iter()
        .skip(top)
        .take(visible)
        .map(|entry| {
            Line::from(vec![
                Span::styled(
                    format!(
                        " {} ",
                        format::pad_left(&format::clock(entry.start, &state.calendar), 8)
                    ),
                    Style::default().fg(tone(Tone::Muted)),
                ),
                Span::styled(
                    format::pad(&entry.title, 10),
                    Style::default().fg(tone(Tone::Accent)),
                ),
                Span::styled(
                    entry.description.clone(),
                    Style::default().fg(tone(Tone::Value)),
                ),
            ])
        })
        .collect();

    let title = format!(
        "Log ({} of {}, j/k to scroll)",
        top + lines.len(),
        entries.len()
    );
    frame.render_widget(Paragraph::new(lines).block(bordered(&title)), area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_role_paints_in_the_bright_half_of_the_palette() {
        for role in [
            Tone::Heading,
            Tone::Accent,
            Tone::Value,
            Tone::Muted,
            Tone::Success,
            Tone::Warning,
            Tone::Error,
            Tone::Info,
            Tone::Prompt,
        ] {
            let Color::Indexed(index) = tone(role) else {
                panic!("{role:?} should be an indexed colour");
            };
            assert!((8..=15).contains(&index), "{role:?} is {index}");
        }
    }

    #[test]
    fn sleep_and_a_feed_are_told_apart_by_colour_as_well_as_by_glyph() {
        assert_ne!(cell_colour(Cell::Asleep), cell_colour(Cell::Feed));
        assert_ne!(cell_colour(Cell::Feed), cell_colour(Cell::Diaper));
    }

    #[test]
    fn an_empty_cell_is_muted_whichever_kind_of_empty_it_is() {
        let muted = tone(Tone::Muted);
        assert_eq!(cell_colour(Cell::DayAwake), muted);
        assert_eq!(cell_colour(Cell::NightAwake), muted);
        assert_eq!(cell_colour(Cell::Unlived), muted);
    }
}
