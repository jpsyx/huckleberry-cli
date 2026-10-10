//! Modal navigation preserves the menu underneath and follows the shared key map.
use app::tui::version::{Action, CheckState, State};
use app::tui::{App, Intent, Motion, motion_for};
use app::version::{VersionReport, compare_versions};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn open_version(app: &mut App, offline: bool) {
    for label in ["More", "Version"] {
        let row = app
            .rows()
            .iter()
            .position(|row| row.label == label)
            .unwrap();
        for _ in 0..row {
            app.apply(Motion::Next, 20);
        }
        let intent = app.apply(Motion::Open, 20);
        if label == "Version" {
            assert_eq!(intent, Intent::Version);
            app.open_version(offline);
        }
    }
}

#[test]
fn modal_motions_cannot_change_the_menu_behind_it() {
    let mut app = App::new();
    open_version(&mut app, false);
    let position = (app.menu(), app.cursor());
    for motion in [
        Motion::Next,
        Motion::Previous,
        Motion::First,
        Motion::Last,
        Motion::Highlight(0),
    ] {
        assert_eq!(app.apply(motion, 20), Intent::Stay);
        assert_eq!((app.menu(), app.cursor()), position);
    }
    assert!(app.version.is_some());
    assert_eq!(app.apply(Motion::Back, 20), Intent::CloseVersion);
    assert!(app.version.is_none());
    assert_eq!((app.menu(), app.cursor()), position);
}

#[test]
fn existing_open_back_and_cancel_keys_close_the_modal() {
    for code in [
        KeyCode::Enter,
        KeyCode::Char(' '),
        KeyCode::Right,
        KeyCode::Char('l'),
        KeyCode::Char('d'),
        KeyCode::Esc,
        KeyCode::Left,
        KeyCode::Char('h'),
        KeyCode::Char('a'),
        KeyCode::Backspace,
    ] {
        let mut app = App::new();
        open_version(&mut app, false);
        assert_eq!(
            app.apply(motion_for(KeyEvent::new(code, KeyModifiers::NONE)), 20),
            Intent::CloseVersion
        );
    }
    let mut app = App::new();
    open_version(&mut app, false);
    assert_eq!(app.apply(Motion::Quit, 20), Intent::Quit);
    assert_eq!(app.apply(Motion::Cancel, 20), Intent::CloseVersion);
}

#[test]
fn retry_is_only_available_for_a_finished_online_check() {
    let mut state = State::new(false);
    assert_eq!(state.apply(Motion::Refresh), Action::Stay);
    state.check = CheckState::Complete(compare_versions("0.47.2", "v0.48.0").unwrap());
    assert_eq!(state.apply(Motion::Refresh), Action::Check);
    assert_eq!(state.check, CheckState::Checking);
    assert_eq!(state.apply(Motion::Refresh), Action::Stay);
    let mut offline = State::new(true);
    assert_eq!(offline.apply(Motion::Refresh), Action::Stay);
}

#[test]
fn closed_results_are_ignored_and_reopening_checks_again() {
    let mut app = App::new();
    app.open_version(false);
    app.apply(Motion::Back, 20);
    app.complete_version(VersionReport::offline("stale"));
    assert!(app.version.is_none());
    app.open_version(false);
    assert_eq!(app.version.unwrap().check, CheckState::Checking);
}

#[test]
fn modal_is_centered_clamped_and_recentered_after_resize() {
    use app::tui::draw::version::area;
    use ratatui::layout::Rect;
    assert_eq!(area(Rect::new(0, 0, 80, 24)), Rect::new(8, 6, 64, 12));
    assert_eq!(area(Rect::new(0, 0, 120, 40)), Rect::new(28, 14, 64, 12));
    assert_eq!(area(Rect::new(5, 3, 30, 8)), Rect::new(5, 3, 30, 8));
}

fn screen(app: &App, width: u16, height: u16) -> String {
    use ratatui::{Terminal, backend::TestBackend};
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| app::tui::draw(frame, app, 0.0))
        .unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|row| {
            (0..width)
                .map(|column| buffer[(column, row)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn checking_modal_shows_installed_version_close_and_its_own_keys() {
    let mut app = App::new();
    app.open_version(false);
    let drawn = screen(&app, 80, 24);
    for text in [
        env!("CARGO_PKG_VERSION"),
        "Checking",
        "Close",
        "r retry",
        "q quit",
    ] {
        assert!(drawn.contains(text), "missing {text}: {drawn}");
    }
    assert!(drawn.lines().nth(6).unwrap().contains("Version"));
    assert!(!drawn.lines().last().unwrap().contains("r refresh"));
}

#[test]
fn every_completed_status_displays_its_versions_and_message() {
    use app::version::UpdateStatus;
    let mut reports = vec![
        compare_versions("0.47.2", "v0.47.2").unwrap(),
        compare_versions("0.47.2", "v0.48.0").unwrap(),
        compare_versions("0.49.0", "v0.48.0").unwrap(),
        VersionReport::offline("0.47.2"),
        VersionReport::unavailable("0.47.2", "Request timed out"),
    ];
    reports.push(VersionReport {
        installed: "0.47.2".into(),
        latest: None,
        status: UpdateStatus::NoRelease,
        reason: None,
    });
    for report in reports {
        let mut app = App::new();
        app.open_version(false);
        app.complete_version(report.clone());
        let drawn = screen(&app, 80, 24);
        for text in [
            Some(report.installed.as_str()),
            report.latest.as_deref(),
            Some(report.status.message()),
            report.reason.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            assert!(drawn.contains(text), "missing {text}: {drawn}");
        }
    }
}

#[test]
fn tiny_modal_draws_without_panicking() {
    let mut app = App::new();
    app.open_version(false);
    for size in [(1, 1), (10, 3), (30, 8)] {
        screen(&app, size.0, size.1);
    }
}
