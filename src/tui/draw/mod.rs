//! Drawing the shell.
//!
//! One frame, always the same three parts: who and where at the top, the
//! widgets in the middle, the keys along the bottom. The parts never move,
//! because a screen somebody glances at with a baby on one arm is read by
//! position long before it is read by word.
//!
//! The middle holds the [`now`] widget beside the [`menu`] view. The Now
//! widget is on the left because the eye lands top-left first and the facts
//! are what somebody opened the terminal to read; the Menu view is the main
//! panel and takes everything left over, because it is the part that is
//! navigated and its rows need the room. On a terminal too narrow to hold
//! both, they stack with the facts on top, in the same reading order.
//!
//! Colours come from [`crate::theme`] through [`crate::dashboard::draw::tone`],
//! so this screen and every other one in the tool agree on what a heading
//! looks like.

pub mod menu;
pub mod now;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use super::state::App;
use crate::theme::Tone;

pub use crate::dashboard::draw::tone;

/// The keys, spelled out on every screen rather than hidden behind `?`.
pub const HINTS: &str = "↑/↓ j/k move · ← h back · → l open · Enter select · r refresh · q quit";

/// How wide the Now widget's column is when there is room for one beside the
/// menu. Enough for a twelve-column label and a value like
/// `night of Mon 28 Sep` without it touching the border.
const SIDEBAR: u16 = 36;

/// Below this, the sidebar would leave the menu too narrow to read, so the two
/// stack instead.
const SIDE_BY_SIDE_FROM: u16 = 72;

/// The fewest rows the menu is ever squeezed to when stacked.
const MENU_FLOOR: u16 = 5;

/// How many rows fit in the menu list, given the whole terminal.
///
/// The loop passes the same number to [`App::apply`](super::state::App::apply),
/// so scrolling and drawing agree about what is on screen.
#[must_use]
pub fn viewport(area: Rect, app: &App) -> usize {
    let menu = split(area, app).1;
    usize::from(menu.height).saturating_sub(2).max(1)
}

/// Draws the whole frame, as of `at`.
pub fn draw(frame: &mut Frame, app: &App, at: f64) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    frame.render_widget(header(app, areas[0].width), areas[0]);
    let (facts, rows) = split(areas[1], app);
    let stacked = is_stacked(areas[1]);
    frame.render_widget(
        now::widget(&app.facts, at, stacked).block(block("Now")),
        facts,
    );
    menu::draw(frame, rows, app);
    frame.render_widget(footer(app), areas[2]);
}

/// Where the Now widget goes, and what the menu gets.
///
/// Either way the widget is only as tall as it has something to say. Beside
/// the menu that leaves the foot of the column empty, which is where the next
/// widget goes; a box two thirds full of nothing reads as broken rather than
/// as finished.
fn split(body: Rect, app: &App) -> (Rect, Rect) {
    if is_stacked(body) {
        let room = body.height.saturating_sub(MENU_FLOOR);
        let areas = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(now::height(&app.facts, true).min(room)),
                Constraint::Min(3),
            ])
            .split(body);
        return (areas[0], areas[1]);
    }
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(SIDEBAR), Constraint::Min(20)])
        .split(body);
    let facts = Rect {
        height: now::height(&app.facts, false).min(columns[0].height),
        ..columns[0]
    };
    (facts, columns[1])
}

/// Side by side when there is room, stacked when there is not.
const fn is_stacked(body: Rect) -> bool {
    body.width < SIDE_BY_SIDE_FROM
}

/// The name of the tool and the child on the left, and where you are on the
/// right.
fn header(app: &App, width: u16) -> Paragraph<'static> {
    let name = app.facts.child_name().map_or_else(
        || crate::APP_TITLE.to_owned(),
        |child| format!("{} · {child}", crate::APP_TITLE),
    );
    let breadcrumb = app.breadcrumb();
    let used = name.chars().count() + breadcrumb.chars().count() + 2;
    let gap = usize::from(width).saturating_sub(used).max(1);
    Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {name}"),
            Style::default()
                .fg(tone(Tone::Heading))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(gap)),
        Span::styled(breadcrumb, Style::default().fg(tone(Tone::Accent))),
    ]))
}

/// A bordered block with a title, which every widget uses.
fn block(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(tone(Tone::Muted)))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(tone(Tone::Heading)),
        ))
}

/// The keys, and whatever the shell has to say for itself.
///
/// A read in flight wins over a failure and a failure wins over a note,
/// because they are in the order somebody needs them: what is happening now,
/// what is wrong, and what just happened.
fn footer(app: &App) -> Paragraph<'static> {
    let note = if app.facts.refreshing {
        Some(("reading…".to_owned(), Tone::Info))
    } else if let Some(trouble) = &app.facts.trouble {
        Some((trouble.clone(), Tone::Warning))
    } else {
        app.status().map(|status| (status.to_owned(), Tone::Info))
    };
    let mut spans = Vec::new();
    match note {
        Some((text, role)) => spans.push(Span::styled(
            format!(" {text} · "),
            Style::default().fg(tone(role)),
        )),
        None => spans.push(Span::raw(" ")),
    }
    spans.push(Span::styled(HINTS, Style::default().fg(tone(Tone::Muted))));
    Paragraph::new(Line::from(spans))
}

#[cfg(test)]
mod frames {
    //! The shell drawn into a buffer, so what it says is asserted rather than
    //! looked at. These live here rather than in `tests/` because the dataset
    //! fixtures are internal, and because it is the same place the dashboard
    //! keeps its own frame tests.

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::cli::Units;
    use crate::domain::Calendar;
    use crate::domain::fixtures::{AFTERNOON, bottle, dataset, diaper, sleep};
    use crate::domain::types::Dataset;

    fn calendar() -> Calendar {
        Calendar::new("America/New_York").expect("a real timezone")
    }

    /// Days from 6am, which is what setup asks a family with an older baby.
    fn rule() -> crate::domain::today::DayRule {
        crate::domain::today::DayRule::discrete(6.0, None)
    }

    fn populated() -> Dataset {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 3_600.0, 90.0)];
        data.sleep = vec![sleep(AFTERNOON - 50_400.0, 10_800.0)];
        data.diapers = vec![diaper(AFTERNOON - 5_400.0, true, false)];
        data
    }

    fn loaded() -> App {
        let mut app = App::new();
        app.facts
            .replace(populated(), calendar(), Units::Ml, rule());
        app
    }

    fn screen(app: &App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("a test terminal");
        terminal
            .draw(|frame| draw(frame, app, AFTERNOON))
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
    fn the_now_widget_leads_with_the_last_feed() {
        let drawn = screen(&loaded(), 100, 24);
        assert!(drawn.contains("Last fed"), "{drawn}");
        assert!(drawn.contains("1h 0m ago"), "{drawn}");
        assert!(drawn.contains("90 ml of Formula"), "{drawn}");
    }

    #[test]
    fn the_widget_shows_every_fact_the_now_command_does() {
        let drawn = screen(&loaded(), 100, 24);
        for fact in ["Last fed", "Diaper", "Awake", "Longest"] {
            assert!(drawn.contains(fact), "`{fact}` missing: {drawn}");
        }
        assert!(drawn.contains("wet"), "{drawn}");
    }

    #[test]
    fn a_volume_follows_the_unit_the_reader_chose() {
        let mut app = App::new();
        app.facts
            .replace(populated(), calendar(), Units::Oz, rule());
        assert!(screen(&app, 100, 24).contains("3.0 oz"));
    }

    #[test]
    fn the_widget_carries_the_running_totals_the_now_command_shows() {
        let drawn = screen(&loaded(), 100, 24);
        for fact in ["Last 3h", "Fed today", "Slept today"] {
            assert!(drawn.contains(fact), "`{fact}` missing: {drawn}");
        }
        assert!(drawn.contains("90 ml · 1 feed"), "{drawn}");
    }

    #[test]
    fn a_rolling_day_is_labelled_as_one_rather_than_called_today() {
        let mut app = App::new();
        app.facts.replace(
            populated(),
            calendar(),
            Units::Ml,
            crate::domain::today::DayRule::continuous(),
        );
        let drawn = screen(&app, 100, 24);
        assert!(drawn.contains("Fed in 24h"), "{drawn}");
        assert!(!drawn.contains("Fed today"), "{drawn}");
    }

    #[test]
    fn the_widget_always_says_how_stale_it_is() {
        let mut data = populated();
        data.fetched_at = AFTERNOON - 600.0;
        let mut app = App::new();
        app.facts.replace(data, calendar(), Units::Ml, rule());
        assert!(screen(&app, 100, 24).contains("as of 10m ago"));
    }

    #[test]
    fn the_night_a_stretch_belongs_to_is_named_rather_than_left_ambiguous() {
        let drawn = screen(&loaded(), 100, 24);
        assert!(drawn.contains("night of Sun 21 Sep"), "{drawn}");
        assert!(!drawn.contains("tonight"), "{drawn}");
    }

    #[test]
    fn before_the_first_read_the_widget_says_it_is_reading() {
        let mut app = App::new();
        app.facts.start_reading();
        let drawn = screen(&app, 100, 24);
        assert!(drawn.contains("reading…"), "{drawn}");
        assert!(
            drawn.contains("Now"),
            "the widget is there before its data is"
        );
        assert!(
            drawn.contains("Log a diaper"),
            "the menu is usable before the network answers: {drawn}"
        );
    }

    #[test]
    fn a_failed_read_keeps_the_numbers_and_says_so() {
        let mut app = loaded();
        app.facts.record_trouble("could not reach Huckleberry");
        let drawn = screen(&app, 100, 24);
        assert!(drawn.contains("could not reach"), "{drawn}");
        assert!(
            drawn.contains("90 ml"),
            "the data is still on screen: {drawn}"
        );
    }

    #[test]
    fn with_nothing_read_at_all_the_widget_says_which_key_reads() {
        let drawn = screen(&App::new(), 100, 24);
        assert!(drawn.contains("press r"), "{drawn}");
    }

    /// The layout rule: facts on the left, menu on the right, one row apiece.
    #[test]
    fn a_wide_terminal_puts_the_widget_beside_the_menu() {
        let drawn = screen(&loaded(), 100, 24);
        let titles = drawn
            .lines()
            .find(|line| line.contains("Now"))
            .unwrap_or_default();
        assert!(
            titles.contains("What would you like to do?"),
            "the two titles share a row when they are side by side: {titles}"
        );
        assert!(
            titles.find("Now") < titles.find("What would"),
            "the facts are on the left, where the eye lands first: {titles}"
        );
    }

    #[test]
    fn the_menu_is_the_main_panel_and_gets_the_room() {
        let body = Rect::new(0, 1, 100, 22);
        let (facts, rows) = split(body, &loaded());
        assert!(
            rows.width > facts.width,
            "the menu view is the main panel: {} vs {}",
            rows.width,
            facts.width
        );
    }

    #[test]
    fn a_narrow_terminal_stacks_them_with_the_facts_on_top() {
        let drawn = screen(&loaded(), 60, 24);
        let now_row = drawn.lines().position(|line| line.contains("Now"));
        let menu_row = drawn
            .lines()
            .position(|line| line.contains("What would you like to do?"));
        assert!(now_row.is_some() && menu_row.is_some(), "{drawn}");
        assert!(now_row < menu_row, "the facts stay above the menu: {drawn}");
        assert!(drawn.contains("Last fed"), "{drawn}");
        assert!(drawn.contains("Log a diaper"), "{drawn}");
    }

    #[test]
    fn the_menu_keeps_rows_even_when_the_facts_would_fill_the_screen() {
        for height in [12, 16, 24] {
            let drawn = screen(&loaded(), 60, height);
            assert!(
                drawn.contains("Log a diaper"),
                "the menu is never squeezed out at {height} rows: {drawn}"
            );
        }
    }

    #[test]
    fn the_header_names_the_child_once_a_read_has_said_who_it_is() {
        assert!(screen(&loaded(), 100, 24).contains("Huckleberry · Bear"));
        assert!(screen(&App::new(), 100, 24).contains("Huckleberry"));
    }

    #[test]
    fn nothing_about_the_baby_is_ever_painted_red() {
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).expect("a test terminal");
        terminal
            .draw(|frame| draw(frame, &loaded(), AFTERNOON))
            .expect("a frame");
        let buffer = terminal.backend().buffer().clone();
        let red = tone(Tone::Error);
        assert!(
            (0..buffer.area.height)
                .flat_map(|row| (0..buffer.area.width).map(move |column| (column, row)))
                .all(|cell| buffer[cell].fg != red),
            "a screen about a baby never delivers a verdict"
        );
    }

    #[test]
    fn the_viewport_leaves_room_for_the_rows_it_reports() {
        for (width, height) in [(100_u16, 24_u16), (60, 24), (40, 14), (100, 8)] {
            let body = Rect::new(0, 1, width, height.saturating_sub(2));
            let rows = viewport(body, &loaded());
            assert!(rows >= 1, "{width}x{height} reported {rows} rows");
        }
    }
}
