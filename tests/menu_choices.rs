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

#[test]
fn defaults_skip_and_text_are_distinct_answers() {
    use app::prompt::select::{Answer, question_menu};
    use app::prompt::{Choice, Question};
    let choices = [
        Choice {
            value: "yes",
            hint: "Yes",
        },
        Choice {
            value: "no",
            hint: "No",
        },
    ];
    let required = Question::new("answer", "Choose", "--answer").with_choices(&choices);
    let menu = question_menu(&required.with_default("no"));
    assert_eq!(menu.answers[menu.default], Answer::Value("no".into()));
    assert!(!menu.answers.contains(&Answer::Skip));
    let optional = question_menu(&required.optional());
    assert_eq!(optional.answers[optional.default], Answer::Skip);
    let unknown = question_menu(&required.with_default("legacy"));
    assert_eq!(
        unknown.answers[unknown.default],
        Answer::Value("legacy".into())
    );
    let text = question_menu(&Question::new("note", "Notes?", "--notes").optional());
    assert_eq!(text.answers, vec![Answer::Skip, Answer::Text]);
}

#[test]
fn typed_cancel_survives_error_context() {
    let error = anyhow::Error::new(app::prompt::Cancelled).context("answering a question");
    assert!(app::prompt::is_cancelled(&error));
    assert!(!app::prompt::is_cancelled(&anyhow::anyhow!("cancelled")));
}

#[test]
fn lists_use_extra_keys_but_search_keeps_letters() {
    use app::listing::state::{Flow, State, apply, key_for};
    let mut state = State::new(String::new());
    assert_eq!(
        apply(&mut state, key_for(key(KeyCode::Char('H'))), 5, 2),
        Flow::Stay
    );
    assert_eq!(state.cursor, 1);
    apply(&mut state, key_for(key(KeyCode::Char('P'))), 5, 2);
    assert_eq!(state.cursor, 0);
    state.searching = true;
    for letter in "happy".chars() {
        apply(&mut state, key_for(key(KeyCode::Char(letter))), 5, 2);
    }
    assert_eq!(state.query, "happy");
}

#[test]
fn dashboard_h_scrolls_log_instead_of_changing_tab() {
    use app::dashboard::state::{Action, Tab, action_for_tab};
    assert_eq!(
        action_for_tab(key(KeyCode::Char('H')), Tab::Log),
        Action::ScrollDown
    );
    assert_eq!(
        action_for_tab(key(KeyCode::Char('p')), Tab::Log),
        Action::ScrollUp
    );
    assert!(matches!(
        action_for_tab(key(KeyCode::Left), Tab::Log),
        Action::Show(_)
    ));
}

#[test]
fn list_numbering_preserves_row_keys_and_leaves_headings_unnumbered() {
    use app::listing::{Line, Piece};
    let mut lines = vec![
        Line::text(vec![Piece::new("Today", app::theme::Tone::Heading)]),
        Line {
            pieces: vec![Piece::new("meal", app::theme::Tone::Value)],
            row: Some(0),
        },
    ];
    app::listing::layout::number_rows(&mut lines);
    assert_eq!(lines[0].pieces[0].text, "Today");
    assert!(
        lines[1]
            .pieces
            .iter()
            .any(|piece| piece.text.contains("1."))
    );
    assert_eq!(lines[1].row, Some(0));
}
