//! The always-on TUI shell: its one-handed key map, its navigation, and what
//! it draws. Everything asserted here is pure, so the shell that owns the
//! alternate screen holds no decision worth testing.
use app::interactive::catalog::{self, MenuId};
use app::tui::{App, Intent, Motion, draw, motion_for};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

const HEIGHT: usize = 20;

const fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

const fn shifted(letter: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(letter), KeyModifiers::SHIFT)
}

/// Moves the cursor onto the row with this label, the way a thumb would.
fn highlight(app: &mut App, label: &str) {
    let index = app
        .rows()
        .iter()
        .position(|row| row.label == label)
        .unwrap_or_else(|| panic!("no row labelled {label}"));
    while app.cursor() < index {
        app.apply(Motion::Next, HEIGHT);
    }
    while app.cursor() > index {
        app.apply(Motion::Previous, HEIGHT);
    }
}

#[test]
fn every_direction_has_an_arrow_and_a_letter_that_agree() {
    for (arrow, letters, motion) in [
        (KeyCode::Down, ['j', 'J'], Motion::Next),
        (KeyCode::Up, ['k', 'K'], Motion::Previous),
        (KeyCode::Right, ['l', 'L'], Motion::Open),
        (KeyCode::Left, ['h', 'H'], Motion::Back),
    ] {
        assert_eq!(motion_for(key(arrow)), motion, "{arrow:?}");
        for letter in letters {
            assert_eq!(motion_for(key(KeyCode::Char(letter))), motion, "{letter}");
            assert_eq!(motion_for(shifted(letter)), motion, "shift-{letter}");
        }
    }
}

#[test]
fn enter_opens_and_escape_goes_back() {
    assert_eq!(motion_for(key(KeyCode::Enter)), Motion::Open);
    assert_eq!(motion_for(key(KeyCode::Char(' '))), Motion::Open);
    assert_eq!(motion_for(key(KeyCode::Esc)), Motion::Back);
    assert_eq!(motion_for(key(KeyCode::Backspace)), Motion::Back);
}

#[test]
fn q_leaves_and_so_does_the_chord() {
    for quit in ['q', 'Q'] {
        assert_eq!(motion_for(key(KeyCode::Char(quit))), Motion::Quit);
        assert_eq!(
            motion_for(KeyEvent::new(KeyCode::Char(quit), KeyModifiers::CONTROL)),
            Motion::Quit
        );
    }
    assert_eq!(
        motion_for(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        Motion::Cancel,
        "ctrl-c cancels what you are in, which is not always the session"
    );
}

/// The difference between the two is how far each one reaches.
#[test]
fn only_the_chord_reaches_past_a_question_somebody_is_typing_into() {
    use app::tui::keys::forces_quit;

    for quit in ['q', 'Q'] {
        assert!(
            forces_quit(KeyEvent::new(KeyCode::Char(quit), KeyModifiers::CONTROL)),
            "ctrl-{quit} ends the session from inside anything"
        );
        assert!(
            !forces_quit(key(KeyCode::Char(quit))),
            "a bare `{quit}` is a letter when something is asking for letters"
        );
    }
    for other in [
        key(KeyCode::Char('a')),
        key(KeyCode::Enter),
        key(KeyCode::Esc),
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
    ] {
        assert!(!forces_quit(other), "{other:?}");
    }
    assert!(
        !forces_quit(KeyEvent::new_with_kind(
            KeyCode::Char('q'),
            KeyModifiers::CONTROL,
            KeyEventKind::Release,
        )),
        "a release is not a press"
    );
}

#[test]
fn ctrl_c_walks_out_of_a_submenu_rather_than_ending_the_session() {
    let mut app = App::new();
    highlight(&mut app, "Log a feed");
    app.apply(Motion::Open, HEIGHT);
    highlight(&mut app, "Nursing");
    app.apply(Motion::Open, HEIGHT);
    assert_eq!(app.breadcrumb(), "Home › Log a feed › Nursing");

    assert_eq!(app.apply(Motion::Cancel, HEIGHT), Intent::Stay);
    assert_eq!(
        app.breadcrumb(),
        "Home › Log a feed",
        "one level, not all of them"
    );
    assert_eq!(app.apply(Motion::Cancel, HEIGHT), Intent::Stay);
    assert_eq!(app.breadcrumb(), "Home");
}

#[test]
fn ctrl_c_ends_the_session_once_there_is_nothing_left_to_cancel() {
    let mut app = App::new();
    assert_eq!(app.breadcrumb(), "Home");
    assert_eq!(
        app.apply(Motion::Cancel, HEIGHT),
        Intent::Quit,
        "at the top level the thing being cancelled is the session"
    );
}

#[test]
fn ctrl_q_ends_the_session_however_deep_the_menu_is() {
    let mut app = App::new();
    assert_eq!(app.apply(Motion::Quit, HEIGHT), Intent::Quit);
    highlight(&mut app, "Log a feed");
    app.apply(Motion::Open, HEIGHT);
    highlight(&mut app, "Nursing");
    app.apply(Motion::Open, HEIGHT);
    assert_eq!(
        app.apply(Motion::Quit, HEIGHT),
        Intent::Quit,
        "it is the one key that does not care where you are"
    );
}

#[test]
fn the_reflex_navigation_keys_still_never_end_the_session() {
    let mut app = App::new();
    for motion in [
        Motion::Back,
        Motion::Next,
        Motion::Previous,
        Motion::Refresh,
    ] {
        assert_ne!(app.apply(motion, HEIGHT), Intent::Quit, "{motion:?}");
    }
    for code in [
        KeyCode::Esc,
        KeyCode::Backspace,
        KeyCode::Left,
        KeyCode::Char('h'),
    ] {
        assert_eq!(motion_for(key(code)), Motion::Back, "{code:?}");
    }
}

#[test]
fn digits_highlight_a_row_and_only_enter_opens_it() {
    let mut app = App::new();
    assert_eq!(motion_for(key(KeyCode::Char('3'))), Motion::Highlight(2));
    assert_eq!(motion_for(key(KeyCode::Char('0'))), Motion::Ignore);
    assert_eq!(app.apply(Motion::Highlight(1), HEIGHT), Intent::Stay);
    assert_eq!(app.cursor(), 1);
    assert_eq!(app.rows()[1].label, "Log a feed");
    assert!(matches!(app.apply(Motion::Open, HEIGHT), Intent::Stay));
    assert_eq!(app.title(), "Log a feed");
}

#[test]
fn a_release_moves_nothing() {
    let released = KeyEvent::new_with_kind(
        KeyCode::Char('j'),
        KeyModifiers::NONE,
        KeyEventKind::Release,
    );
    assert_eq!(motion_for(released), Motion::Ignore);
}

#[test]
fn opening_a_submenu_pushes_a_level_and_back_pops_it() {
    let mut app = App::new();
    assert_eq!(app.breadcrumb(), "Home");
    highlight(&mut app, "Log a feed");
    app.apply(Motion::Open, HEIGHT);
    assert_eq!(app.breadcrumb(), "Home › Log a feed");
    assert_eq!(
        app.rows().last().map(|row| row.label.clone()),
        Some("Back".into())
    );
    app.apply(Motion::Back, HEIGHT);
    assert_eq!(app.breadcrumb(), "Home");
    // The row it was opened from is still under the cursor on the way back.
    assert_eq!(app.rows()[app.cursor()].label, "Log a feed");
}

#[test]
fn the_back_row_leaves_a_submenu_the_same_way_the_left_arrow_does() {
    let mut app = App::new();
    highlight(&mut app, "Log sleep");
    app.apply(Motion::Open, HEIGHT);
    highlight(&mut app, "Back");
    assert_eq!(app.apply(Motion::Open, HEIGHT), Intent::Stay);
    assert_eq!(app.breadcrumb(), "Home");
}

#[test]
fn nothing_but_quit_ever_ends_the_session() {
    let mut app = App::new();
    assert_eq!(app.apply(Motion::Back, HEIGHT), Intent::Stay);
    assert_eq!(app.breadcrumb(), "Home");
    assert_eq!(app.apply(Motion::Quit, HEIGHT), Intent::Quit);
}

#[test]
fn no_row_anywhere_in_the_menu_tree_can_end_the_session() {
    fn walk(app: &mut App) {
        for index in 0..app.rows().len() {
            if app.rows()[index].label == "Back" {
                continue;
            }
            app.apply(Motion::Highlight(index), HEIGHT);
            let depth = app.breadcrumb().matches('›').count();
            let intent = app.apply(Motion::Open, HEIGHT);
            assert_ne!(
                intent,
                Intent::Quit,
                "`{}` ends the session, and no row may: {}",
                app.rows()[index].label,
                app.breadcrumb()
            );
            if intent == Intent::Stay && app.breadcrumb().matches('›').count() > depth {
                walk(app);
                app.apply(Motion::Back, HEIGHT);
            }
        }
    }
    walk(&mut App::new());
}

#[test]
fn a_submenu_row_says_it_opens_a_menu_and_a_command_row_does_not() {
    let app = App::new();
    let rows = app.rows();
    let feed = rows.iter().find(|row| row.label == "Log a feed").unwrap();
    let diaper = rows.iter().find(|row| row.label == "Log a diaper").unwrap();
    assert!(feed.opens_menu);
    assert!(!diaper.opens_menu);
}

#[test]
fn selecting_a_command_asks_the_shell_to_run_it() {
    let mut app = App::new();
    highlight(&mut app, "Log a diaper");
    let Intent::Run(path) = app.apply(Motion::Open, HEIGHT) else {
        panic!("a command row runs a command");
    };
    assert_eq!(path.0, ["diaper"]);
}

#[test]
fn movement_stops_at_both_ends_and_g_reaches_them_in_one_key() {
    let mut app = App::new();
    let last = app.rows().len() - 1;
    for _ in 0..last + 5 {
        app.apply(Motion::Next, HEIGHT);
    }
    assert_eq!(app.cursor(), last);
    for _ in 0..last + 5 {
        app.apply(Motion::Previous, HEIGHT);
    }
    assert_eq!(app.cursor(), 0);
    assert_eq!(motion_for(key(KeyCode::Char('G'))), Motion::Last);
    assert_eq!(motion_for(key(KeyCode::Char('g'))), Motion::First);
    app.apply(Motion::Last, HEIGHT);
    assert_eq!(app.cursor(), last);
    app.apply(Motion::First, HEIGHT);
    assert_eq!(app.cursor(), 0);
}

#[test]
fn a_long_menu_scrolls_so_the_cursor_stays_on_screen() {
    let mut app = App::new();
    app.apply(Motion::Last, 4);
    assert!(app.top() > 0, "a menu taller than its viewport scrolls");
    assert!(app.top() <= app.cursor());
    app.apply(Motion::First, 4);
    assert_eq!(app.top(), 0);
}

#[test]
fn every_command_the_catalog_knows_is_reachable_by_moving_and_opening() {
    fn walk(app: &mut App, found: &mut Vec<Vec<String>>) {
        for index in 0..app.rows().len() {
            if app.rows()[index].label == "Back" {
                continue;
            }
            app.apply(Motion::Highlight(index), HEIGHT);
            let depth = app.breadcrumb().matches('›').count();
            match app.apply(Motion::Open, HEIGHT) {
                Intent::Run(path) => found.push(path.0),
                // The dashboard is reached the same way and drawn by the shell
                // rather than run as a command; it is still reached.
                Intent::Dashboard => found.push(vec!["dash".to_owned()]),
                Intent::Stay if app.breadcrumb().matches('›').count() > depth => {
                    walk(app, found);
                    app.apply(Motion::Back, HEIGHT);
                }
                _ => {}
            }
        }
    }
    let mut found = Vec::new();
    walk(&mut App::new(), &mut found);
    found.sort();
    found.dedup();
    let mut expected = catalog::command_paths()
        .into_iter()
        .map(|path| path.0)
        .collect::<Vec<_>>();
    expected.sort();
    expected.dedup();
    assert_eq!(found, expected);
}

#[test]
fn every_menu_has_a_title_and_every_submenu_offers_back() {
    for menu in MenuId::ALL {
        assert!(!catalog::title(menu).is_empty(), "{menu:?}");
        let has_back = catalog::entries(menu)
            .iter()
            .any(|entry| entry.target == catalog::MenuTarget::Back);
        assert_eq!(has_back, menu != MenuId::Home, "{menu:?}");
    }
}

/// 2025-09-22T18:00:00Z, which is 2pm in New York.
const AFTERNOON: f64 = 1_758_564_000.0;

fn screen(app: &App, width: u16, height: u16) -> String {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height))
        .expect("a test terminal");
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
fn the_shell_draws_the_breadcrumb_the_rows_and_the_keys_it_answers_to() {
    let drawn = screen(&App::new(), 80, 24);
    assert!(drawn.contains("Huckleberry"), "{drawn}");
    assert!(drawn.contains("Home"), "{drawn}");
    assert!(drawn.contains("Log a diaper"), "{drawn}");
    assert!(
        !drawn.contains("Exit"),
        "no row leaves the shell, so none offers to: {drawn}"
    );
    for hint in ["j/k", "h", "l", "Enter", "r refresh", "q quit"] {
        assert!(
            drawn.contains(hint),
            "`{hint}` missing from the key hints: {drawn}"
        );
    }
}

#[test]
fn the_cursor_is_visible_and_moves_with_the_keys() {
    let mut app = App::new();
    let first = screen(&app, 80, 24);
    app.apply(Motion::Next, 20);
    let second = screen(&app, 80, 24);
    assert_ne!(first, second, "the cursor has to be visible on the screen");
    let marked = second
        .lines()
        .find(|line| line.contains("Log a feed"))
        .unwrap_or_default()
        .to_owned();
    assert!(
        marked.contains('▌'),
        "the cursor row carries a marker: {marked}"
    );
}

#[test]
fn a_status_message_reaches_the_foot_of_the_screen() {
    let mut app = App::new();
    app.set_status("huckleberry-cli 9.9.9");
    assert!(screen(&app, 80, 24).contains("huckleberry-cli 9.9.9"));
}

#[test]
fn a_narrow_terminal_still_draws_every_part_of_the_shell() {
    let drawn = screen(&App::new(), 28, 16);
    assert!(drawn.contains("Log a diaper"), "{drawn}");
    assert_eq!(drawn.lines().count(), 16);
}

#[test]
fn the_menu_view_no_longer_offers_view_latest() {
    let app = App::new();
    let labels = app
        .rows()
        .into_iter()
        .map(|row| row.label)
        .collect::<Vec<_>>();
    assert!(
        !labels.contains(&"View latest".to_owned()),
        "the Now widget shows it permanently, so the row is redundant: {labels:?}"
    );
    assert_eq!(labels.first().map(String::as_str), Some("Log a diaper"));
    assert_eq!(labels.last().map(String::as_str), Some("More"));
    // It is still a command, and still reachable, just not from Home.
    assert!(
        catalog::command_paths()
            .iter()
            .any(|path| path.0 == ["now"]),
        "`now` has to stay reachable from somewhere"
    );
}

#[test]
fn r_on_its_own_refreshes_every_widget() {
    assert_eq!(motion_for(key(KeyCode::Char('r'))), Motion::Refresh);
    assert_eq!(motion_for(shifted('R')), Motion::Refresh);
    assert_eq!(
        App::new().apply(Motion::Refresh, HEIGHT),
        Intent::Refresh,
        "the shell does the reading; the state only asks for it"
    );
}

#[test]
fn a_refresh_does_not_move_the_cursor_or_the_menu() {
    let mut app = App::new();
    highlight(&mut app, "Edit");
    let before = app.cursor();
    app.apply(Motion::Refresh, HEIGHT);
    assert_eq!(app.cursor(), before);
    assert_eq!(app.breadcrumb(), "Home");
}

#[test]
fn the_dashboard_opens_in_the_panel_rather_than_taking_the_screen() {
    let mut app = App::new();
    highlight(&mut app, "Visualizations");
    app.apply(Motion::Open, HEIGHT);
    highlight(&mut app, "Dashboard");
    assert_eq!(
        app.apply(Motion::Open, HEIGHT),
        Intent::Dashboard,
        "it is a screen the shell draws, not a command it steps aside for"
    );
}

#[test]
fn the_dashboard_needs_something_read_before_it_can_open() {
    let mut app = App::new();
    app.open_dashboard(7);
    assert!(app.dashboard.is_none(), "there is nothing to draw yet");
    assert_eq!(app.status(), Some("nothing read yet · press r"));
}

#[test]
fn closing_the_dashboard_puts_the_menu_back() {
    let mut app = App::new();
    assert!(!app.close_dashboard(), "there was none to close");
    assert!(
        !app.rows().is_empty(),
        "and the menu is what the panel shows"
    );
}
