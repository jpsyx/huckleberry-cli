use app::prompt::select::{Selection, SelectionAction};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

const fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn menu_keys_select_only_on_enter() {
    for down in ['j', 'J', 'h', 'H'] {
        for up in ['k', 'K', 'p', 'P'] {
            let mut state = Selection::new(3, 0);
            assert_eq!(
                state.apply(key(KeyCode::Char(down)), 3, 2),
                SelectionAction::Stay
            );
            assert_eq!(state.cursor, 1);
            assert_eq!(
                state.apply(key(KeyCode::Enter), 3, 2),
                SelectionAction::Submit(1)
            );
            state.apply(key(KeyCode::Char(up)), 3, 2);
            assert_eq!(state.cursor, 0);
        }
    }
}

#[test]
fn empty_menu_cannot_submit_and_movement_stops_at_boundaries() {
    let mut empty = Selection::new(0, 8);
    assert_eq!(
        empty.apply(key(KeyCode::Enter), 0, 0),
        SelectionAction::Stay
    );
    let mut state = Selection::new(100, 99);
    state.apply(key(KeyCode::Down), 100, 2);
    assert_eq!(state.cursor, 99);
    assert_eq!(state.top, 98);
    let mut first = Selection::new(3, 0);
    first.apply(key(KeyCode::Up), 3, 2);
    assert_eq!(first.cursor, 0);
}

#[test]
fn release_events_do_not_select_and_cancel_does_not_submit() {
    let mut state = Selection::new(2, 1);
    let release =
        KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Release);
    assert_eq!(state.apply(release, 2, 2), SelectionAction::Stay);
    assert_eq!(
        state.apply(key(KeyCode::Esc), 2, 2),
        SelectionAction::Cancel
    );
    assert_eq!(
        state.apply(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            2,
            2
        ),
        SelectionAction::Cancel
    );
}

#[test]
fn narrow_menu_keeps_number_and_cursor() {
    use app::prompt::select::{MenuItem, render};
    let items = vec![MenuItem {
        label: "A very long 🥑 food name".into(),
        detail: None,
    }];
    let lines = render(
        "Food?",
        &items,
        &Selection::new(1, 0),
        16,
        4,
        app::theme::Theme::dark(false),
    );
    assert!(lines.iter().any(|line| line.starts_with("> 1. ")));
    assert!(
        lines
            .iter()
            .all(|line| ratatui::text::Line::raw(line).width() <= 16)
    );
}
