//! Drawing the dashboard.
//!
//! This file holds the frame: the tab bar, the status line, and the routing to
//! one of the panels in [`panels`].
//!
//! The colours come from [`crate::theme`] through [`tone`], so the dashboard
//! and the one-shot commands agree on what a heading looks like and there is
//! still exactly one file in this repository that decides what a role means.
//!
//! What each screen shows is chosen for a person holding a baby: the largest
//! thing on the Now tab is how long since the last feed, and the stripe chart
//! is a picture rather than a table because at 3am a picture is faster.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Tabs};

use crate::cli::Units;
use crate::theme::Tone;

use self::panels::{draw_diapers, draw_feeding, draw_log, draw_now, draw_sleep};
use super::state::{State, Tab};

pub mod panels;

pub use panels::cell_colour;

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
        Tab::Diapers => draw_diapers(frame, areas[1], state, at),
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

/// A bordered block with a title, which every panel uses.
pub(super) fn bordered(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(tone(Tone::Muted)))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(tone(Tone::Heading)),
        ))
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
    use crate::domain::fixtures::{AFTERNOON, bottle, dataset, diaper, sleep};
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
        data.diapers = vec![diaper(AFTERNOON - 1_800.0, true, true)];
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
    fn the_diapers_tab_counts_wet_and_dirty_separately() {
        let drawn = screen(Tab::Diapers, populated());
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
