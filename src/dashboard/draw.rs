//! Drawing the dashboard.
//!
//! The colours come from [`crate::theme`] through [`tone`], so the dashboard
//! and the one-shot commands agree on what a heading looks like and there is
//! still exactly one file in this repository that decides what a role means.
//!
//! What each screen shows is chosen for a person holding a baby: the largest
//! thing on the Now tab is how long since the last feed, and the stripe chart
//! is a picture rather than a table because at 3am a picture is faster.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Sparkline, Table, Tabs};

use crate::cli::Units;
use crate::domain::time::{format_ago, format_duration};
use crate::domain::{log, now, stripes, summaries};
use crate::render::format;
use crate::render::stripes::{Cell, cells};
use crate::theme::Tone;

use super::state::{State, Tab};

/// The colour a semantic role paints as on the dashboard.
///
/// Indexed rather than RGB, so the dashboard inherits whatever palette the
/// person has chosen for their terminal instead of fighting it.
#[must_use]
pub const fn tone(role: Tone) -> Color {
    Color::Indexed(role.ansi_index())
}

/// Draws the whole frame.
pub fn draw(frame: &mut Frame, state: &State, units: Units, at: f64) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(6),
            Constraint::Length(1),
        ])
        .split(frame.area());

    frame.render_widget(tab_bar(state), areas[0]);
    match state.tab {
        Tab::Now => draw_now(frame, areas[1], state, units, at),
        Tab::Sleep => draw_sleep(frame, areas[1], state, at),
        Tab::Feeding => draw_feeding(frame, areas[1], state, units, at),
        Tab::Nappies => draw_nappies(frame, areas[1], state, at),
        Tab::Log => draw_log(frame, areas[1], state),
    }
    frame.render_widget(status_bar(state, at), areas[2]);
}

/// The tab bar, with the child's name on it.
fn tab_bar(state: &State) -> Tabs<'_> {
    let titles: Vec<Line<'_>> = Tab::ALL
        .into_iter()
        .enumerate()
        .map(|(index, tab)| {
            Line::from(vec![
                Span::styled(
                    format!("{} ", index + 1),
                    Style::default().fg(tone(Tone::Muted)),
                ),
                Span::raw(tab.title()),
            ])
        })
        .collect();
    Tabs::new(titles)
        .select(state.tab.index())
        .style(Style::default().fg(tone(Tone::Accent)))
        .highlight_style(
            Style::default()
                .fg(tone(Tone::Heading))
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::styled("│", Style::default().fg(tone(Tone::Muted))))
}

/// The line along the bottom: what is stale, what went wrong, and the keys.
fn status_bar(state: &State, at: f64) -> Paragraph<'_> {
    let mut spans = vec![Span::styled(
        format!(" {} · ", state.dataset.child.name),
        Style::default().fg(tone(Tone::Accent)),
    )];

    if state.refreshing {
        spans.push(Span::styled(
            "reading… ",
            Style::default().fg(tone(Tone::Info)),
        ));
    }
    spans.push(Span::styled(
        crate::render::now::as_of(&state.dataset, at),
        Style::default().fg(tone(Tone::Muted)),
    ));

    if let Some(trouble) = &state.trouble {
        spans.push(Span::styled(
            format!(" · {trouble}"),
            Style::default().fg(tone(Tone::Warning)),
        ));
    }
    spans.push(Span::styled(
        "   q quit · r refresh · tab/1-5 screens",
        Style::default().fg(tone(Tone::Muted)),
    ));
    Paragraph::new(Line::from(spans))
}

/// The Now tab: the four facts, large, and a week of sleep beneath them.
fn draw_now(frame: &mut Frame, area: Rect, state: &State, units: Units, at: f64) {
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
fn draw_sleep(frame: &mut Frame, area: Rect, state: &State, at: f64) {
    draw_stripe_chart(frame, area, state, at, state.days);
}

/// The stripe chart, as a block of rows.
fn draw_stripe_chart(frame: &mut Frame, area: Rect, state: &State, at: f64, days: usize) {
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
fn draw_feeding(frame: &mut Frame, area: Rect, state: &State, units: Units, at: f64) {
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
                Tone::Muted
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
fn draw_nappies(frame: &mut Frame, area: Rect, state: &State, at: f64) {
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
                Tone::Muted
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
fn draw_log(frame: &mut Frame, area: Rect, state: &State) {
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

/// A bordered block with a title, which every panel uses.
fn bordered(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(tone(Tone::Muted)))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(tone(Tone::Heading)),
        ))
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

#[cfg(test)]
mod frames {
    //! The dashboard drawn into a buffer, so what it says is asserted rather
    //! than looked at. This is the only way to test a full-screen program
    //! without a terminal, and it catches the two things that actually break:
    //! a panel that overflows its area, and a screen that renders empty.

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::dashboard::state::State;
    use crate::domain::Calendar;
    use crate::domain::fixtures::{AFTERNOON, bottle, dataset, nappy, sleep};
    use crate::domain::types::Dataset;

    fn populated() -> Dataset {
        let mut data = dataset();
        data.feeds = vec![
            bottle(AFTERNOON - 86_400.0, 75.0),
            bottle(AFTERNOON - 3_600.0, 90.0),
        ];
        data.sleep = vec![
            sleep(AFTERNOON - 90_000.0, 10_800.0),
            sleep(AFTERNOON - 7_200.0, 3_600.0),
        ];
        data.diapers = vec![nappy(AFTERNOON - 1_800.0, true, true)];
        data
    }

    fn screen(tab: Tab, data: Dataset) -> String {
        let mut state = State::new(
            data,
            Calendar::new("America/New_York").expect("a real timezone"),
            7,
        );
        state.tab = tab;
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("a test terminal");
        terminal
            .draw(|frame| draw(frame, &state, Units::Ml, AFTERNOON))
            .expect("a frame");
        let buffer = terminal.backend().buffer().clone();
        (0..buffer.area.height)
            .map(|row| {
                (0..buffer.area.width)
                    .map(|column| buffer[(column, row)].symbol().to_owned())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn every_tab_draws_without_panicking_and_names_itself() {
        for tab in Tab::ALL {
            let drawn = screen(tab, populated());
            assert!(
                drawn.contains(tab.title()),
                "the {} tab does not name itself",
                tab.title()
            );
        }
    }

    #[test]
    fn the_now_tab_leads_with_the_last_feed() {
        let drawn = screen(Tab::Now, populated());
        assert!(drawn.contains("Last fed"), "{drawn}");
        assert!(drawn.contains("1h 0m"), "{drawn}");
        assert!(drawn.contains("90 ml"), "{drawn}");
    }

    #[test]
    fn the_status_bar_always_says_how_stale_the_screen_is() {
        assert!(screen(Tab::Now, populated()).contains("as of"));
    }

    #[test]
    fn the_status_bar_names_the_child_and_the_keys() {
        let drawn = screen(Tab::Now, populated());
        assert!(drawn.contains("Bear"), "{drawn}");
        assert!(drawn.contains("q quit"), "{drawn}");
    }

    #[test]
    fn the_sleep_tab_draws_the_stripe_chart_with_its_legend() {
        let drawn = screen(Tab::Sleep, populated());
        assert!(drawn.contains("asleep"), "{drawn}");
        assert!(drawn.contains('█'), "{drawn}");
    }

    #[test]
    fn the_feeding_tab_shows_a_column_per_figure() {
        let drawn = screen(Tab::Feeding, populated());
        for column in ["day", "feeds", "milk ml", "nursed"] {
            assert!(
                drawn.contains(column),
                "`{column}` missing from the feeding tab"
            );
        }
    }

    #[test]
    fn the_nappies_tab_counts_wet_and_dirty_separately() {
        let drawn = screen(Tab::Nappies, populated());
        assert!(drawn.contains("wet"), "{drawn}");
        assert!(drawn.contains("dirty"), "{drawn}");
    }

    #[test]
    fn the_log_tab_says_how_far_through_it_is() {
        let drawn = screen(Tab::Log, populated());
        assert!(drawn.contains("Log ("), "{drawn}");
        assert!(drawn.contains("Bottle"), "{drawn}");
    }

    #[test]
    fn an_empty_dataset_draws_a_screen_rather_than_nothing() {
        for tab in Tab::ALL {
            let drawn = screen(tab, dataset());
            assert!(
                drawn.lines().filter(|line| !line.trim().is_empty()).count() > 3,
                "the {} tab is blank with no data",
                tab.title()
            );
        }
    }

    #[test]
    fn a_dropped_connection_is_reported_without_taking_the_numbers_away() {
        let mut state = State::new(
            populated(),
            Calendar::new("America/New_York").expect("a real timezone"),
            7,
        );
        state.record_trouble("could not reach Huckleberry");
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("a test terminal");
        terminal
            .draw(|frame| draw(frame, &state, Units::Ml, AFTERNOON))
            .expect("a frame");
        let buffer = terminal.backend().buffer().clone();
        let drawn: String = (0..buffer.area.height)
            .flat_map(|row| (0..buffer.area.width).map(move |column| (column, row)))
            .map(|(column, row)| buffer[(column, row)].symbol().to_owned())
            .collect();
        assert!(drawn.contains("could not reach"), "{drawn}");
        assert!(drawn.contains("90 ml"), "the data is still on screen");
    }
}
