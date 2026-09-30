//! Drawing the shell.
//!
//! One frame, always the same three parts: who and where at the top, the
//! widgets in the middle, the keys along the bottom. The parts never move,
//! because a screen somebody glances at with a baby on one arm is read by
//! position long before it is read by word.
//!
//! The middle holds the [`menu`] view above the [`now`] drawer. The Menu view
//! is the main panel and takes everything left over; the Now drawer is a
//! full-width strip along the bottom, close to the keys and to the hand, where
//! a glance goes without leaving the row being navigated.
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

/// The style a semantic role draws in.
///
/// Bold and faint come from the palette in [`crate::theme`] rather than from a
/// second opinion here, so a role that is bold on stdout is bold on the screen
/// and there is still exactly one file that decides what a role looks like.
#[must_use]
pub fn style(role: Tone) -> Style {
    let style = Style::default().fg(tone(role));
    if role.sgr().starts_with("1;") {
        return style.add_modifier(Modifier::BOLD);
    }
    if role.sgr().starts_with("2;") {
        return style.add_modifier(Modifier::DIM);
    }
    style
}

/// The keys, spelled out on every screen rather than hidden behind `?`.
pub const HINTS: &str =
    "↑/↓ j/k move · ← h back · → l open · Enter select · r refresh · ctrl-q quit";

/// The fewest rows the menu is ever squeezed to.
///
/// The drawer gives way rather than the menu: the menu is the part being
/// operated, and a drawer scrolled off its last line is still readable where a
/// menu with no rows is not.
const MENU_FLOOR: u16 = 5;

/// How many rows fit in the menu list, given the whole terminal.
///
/// The loop passes the same number to [`App::apply`](super::state::App::apply),
/// so scrolling and drawing agree about what is on screen.
#[must_use]
pub fn viewport(area: Rect, app: &App) -> usize {
    let menu = split(area, app).0;
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
    let (rows, facts) = split(areas[1], app);
    menu::draw(frame, rows, app);
    frame.render_widget(now::widget(&app.facts, at).block(block("Now")), facts);
    frame.render_widget(footer(app), areas[2]);
}

/// What the menu gets, and where the drawer sits under it.
///
/// The drawer is only as tall as it has something to say, and gives way to the
/// menu on a short terminal rather than the other way round.
fn split(body: Rect, app: &App) -> (Rect, Rect) {
    let room = body.height.saturating_sub(MENU_FLOOR);
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(now::height(&app.facts).min(room)),
        ])
        .split(body);
    (areas[0], areas[1])
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
        crate::domain::today::DayRule::discrete(6.0, 19.5)
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
        let drawn = screen(&loaded(), 100, 30);
        assert!(drawn.contains("Last fed"), "{drawn}");
        assert!(drawn.contains("1h 0m ago"), "{drawn}");
        assert!(drawn.contains("90 ml of Formula"), "{drawn}");
    }

    /// The rule the drawer exists to keep: it is `h now`, and nothing else.
    #[test]
    fn the_drawer_draws_exactly_what_the_now_command_prints() {
        let app = loaded();
        let reading = app.facts.reading().expect("a reading");
        let view =
            crate::domain::now::build(&reading.dataset, &reading.calendar, reading.rule, AFTERNOON);
        let printed = crate::render::now::lines(
            &view,
            &reading.dataset,
            &reading.calendar,
            crate::theme::Theme::dark(false),
            reading.units,
            AFTERNOON,
        );
        let drawn = screen(&app, 100, 30);
        for line in printed.iter().filter(|line| !line.trim().is_empty()) {
            assert!(
                drawn.contains(line.trim_end()),
                "`h now` prints `{line}` and the drawer does not: {drawn}"
            );
        }
    }

    #[test]
    fn the_drawer_shows_every_fact_the_now_command_does() {
        let drawn = screen(&loaded(), 100, 30);
        // The wording is `h now`'s, because the drawer has none of its own.
        for fact in ["Last fed", "Diaper", "Sleep", "Last 3h", "Fed", "Slept"] {
            assert!(drawn.contains(fact), "`{fact}` missing: {drawn}");
        }
        assert!(drawn.contains("wet"), "{drawn}");
    }

    #[test]
    fn a_volume_follows_the_unit_the_reader_chose() {
        let mut app = App::new();
        app.facts
            .replace(populated(), calendar(), Units::Oz, rule());
        assert!(screen(&app, 100, 30).contains("3.0 oz"));
    }

    #[test]
    fn the_widget_carries_the_running_totals_the_now_command_shows() {
        let drawn = screen(&loaded(), 100, 30);
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
            crate::domain::today::DayRule::continuous(6.0, 19.5),
        );
        let drawn = screen(&app, 100, 30);
        assert!(drawn.contains("Fed in 24h"), "{drawn}");
        assert!(!drawn.contains("Fed today"), "{drawn}");
    }

    #[test]
    fn the_widget_always_says_how_stale_it_is() {
        let mut data = populated();
        data.fetched_at = AFTERNOON - 600.0;
        let mut app = App::new();
        app.facts.replace(data, calendar(), Units::Ml, rule());
        assert!(screen(&app, 100, 30).contains("as of 10m ago"));
    }

    #[test]
    fn the_night_a_stretch_belongs_to_is_named_rather_than_left_ambiguous() {
        let drawn = screen(&loaded(), 100, 30);
        assert!(drawn.contains("Night of"), "{drawn}");
        assert!(
            !drawn.contains("Tonight"),
            "an ambiguous label is the bug: {drawn}"
        );
    }

    #[test]
    fn before_the_first_read_the_widget_says_it_is_reading() {
        let mut app = App::new();
        app.facts.start_reading();
        let drawn = screen(&app, 100, 30);
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
        let drawn = screen(&app, 100, 30);
        assert!(drawn.contains("could not reach"), "{drawn}");
        assert!(
            drawn.contains("90 ml"),
            "the data is still on screen: {drawn}"
        );
    }

    #[test]
    fn with_nothing_read_at_all_the_widget_says_which_key_reads() {
        let drawn = screen(&App::new(), 100, 30);
        assert!(drawn.contains("press r"), "{drawn}");
    }

    /// The layout rule: the menu on top, the drawer along the bottom.
    #[test]
    fn the_drawer_is_a_full_width_strip_under_the_menu() {
        let drawn = screen(&loaded(), 100, 30);
        let menu_row = drawn
            .lines()
            .position(|line| line.contains("What would you like to do?"))
            .expect("a menu");
        let drawer_row = drawn
            .lines()
            .position(|line| line.contains(" Now "))
            .expect("a drawer");
        assert!(
            menu_row < drawer_row,
            "the drawer sits under the menu: {drawn}"
        );
        let width = drawn
            .lines()
            .nth(drawer_row)
            .map(|line| line.trim_end().chars().count())
            .unwrap_or_default();
        assert_eq!(width, 100, "and spans the whole width: {drawn}");
    }

    #[test]
    fn the_menu_is_the_main_panel_and_gets_the_room() {
        let body = Rect::new(0, 1, 100, 28);
        let (rows, facts) = split(body, &loaded());
        assert_eq!(rows.width, facts.width, "both span the width");
        assert!(
            rows.height > facts.height,
            "the menu view is the main panel: {} vs {}",
            rows.height,
            facts.height
        );
        assert_eq!(
            rows.y + rows.height,
            facts.y,
            "and the drawer is beneath it"
        );
    }

    #[test]
    fn a_narrow_terminal_keeps_the_same_shape() {
        let drawn = screen(&loaded(), 60, 30);
        assert!(drawn.contains("Last fed"), "{drawn}");
        assert!(drawn.contains("Log a diaper"), "{drawn}");
    }

    #[test]
    fn the_menu_keeps_rows_even_when_the_drawer_would_fill_the_screen() {
        for height in [12, 16, 24] {
            let drawn = screen(&loaded(), 80, height);
            assert!(
                drawn.contains("Log a diaper"),
                "the menu is never squeezed out at {height} rows: {drawn}"
            );
        }
    }

    #[test]
    fn the_header_names_the_child_once_a_read_has_said_who_it_is() {
        assert!(screen(&loaded(), 100, 30).contains("Huckleberry · Bear"));
        assert!(screen(&App::new(), 100, 30).contains("Huckleberry"));
    }

    #[test]
    fn nothing_about_the_baby_is_ever_painted_red() {
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).expect("a test terminal");
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
