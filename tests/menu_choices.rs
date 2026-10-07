use app::prompt::select::{Selection, SelectionAction};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

const fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn digits_highlight_the_corresponding_item_until_enter() {
    let mut state = Selection::new(11, 10);
    for (digit, cursor) in [
        ('1', 0),
        ('2', 1),
        ('3', 2),
        ('4', 3),
        ('5', 4),
        ('6', 5),
        ('7', 6),
        ('8', 7),
        ('9', 8),
    ] {
        assert_eq!(
            state.apply(key(KeyCode::Char(digit)), 11, 3),
            SelectionAction::Stay
        );
        assert_eq!(state.cursor, cursor);
        assert_eq!(
            state.apply(key(KeyCode::Enter), 11, 3),
            SelectionAction::Submit(cursor)
        );
    }
}

#[test]
fn digit_jumps_scroll_and_items_after_nine_remain_reachable() {
    let mut state = Selection::new(11, 0);
    state.apply(key(KeyCode::Char('9')), 11, 3);
    assert_eq!((state.cursor, state.top), (8, 6));
    state.apply(key(KeyCode::Down), 11, 3);
    assert_eq!(state.cursor, 9);
    state.apply(key(KeyCode::Down), 11, 3);
    assert_eq!(
        state.apply(key(KeyCode::Enter), 11, 3),
        SelectionAction::Submit(10)
    );
    state.apply(key(KeyCode::Char('1')), 11, 3);
    assert_eq!((state.cursor, state.top), (0, 0));
}

#[test]
fn zero_and_unavailable_digits_leave_the_highlight_unchanged() {
    for (count, default, digit) in [(3, 1, '4'), (3, 1, '9'), (11, 10, '0'), (0, 0, '1')] {
        let mut state = Selection::new(count, default);
        assert_eq!(
            state.apply(key(KeyCode::Char(digit)), count, 20),
            SelectionAction::Stay
        );
        assert_eq!(state.cursor, default);
    }
}

#[test]
fn digits_are_individual_shortcuts_and_never_form_ten() {
    let mut state = Selection::new(11, 8);
    for digit in ['1', '0'] {
        assert_eq!(
            state.apply(key(KeyCode::Char(digit)), 11, 20),
            SelectionAction::Stay
        );
        assert_eq!(state.cursor, 0);
    }
}

#[test]
fn digit_releases_repeats_and_control_chords_do_not_move() {
    let mut state = Selection::new(9, 0);
    for event in [
        KeyEvent::new_with_kind(
            KeyCode::Char('4'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        ),
        KeyEvent::new_with_kind(KeyCode::Char('4'), KeyModifiers::NONE, KeyEventKind::Repeat),
        KeyEvent::new(KeyCode::Char('4'), KeyModifiers::CONTROL),
    ] {
        assert_eq!(state.apply(event, 9, 3), SelectionAction::Stay);
        assert_eq!(state.cursor, 0);
    }
}

#[test]
fn menu_keys_select_only_on_enter() {
    for down in ['j', 'J', 's', 'S'] {
        for up in ['k', 'K', 'w', 'W'] {
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

/// Horizontal keys never move the highlight or submit a menu answer.
#[test]
fn horizontal_keys_never_move_or_submit_a_menu() {
    for letter in ['h', 'H', 'a', 'A', 'p', 'P', 'l', 'L', 'd', 'D'] {
        let mut state = Selection::new(3, 1);
        assert_ne!(
            state.apply(key(KeyCode::Char(letter)), 3, 2),
            SelectionAction::Submit(1)
        );
        assert_eq!(
            state.cursor, 1,
            "`{letter}` is not a movement key and must not move the cursor"
        );
    }
}

#[test]
fn h_a_and_the_left_arrow_back_out_of_a_menu_as_escape_does() {
    for code in ['h', 'H', 'a', 'A']
        .map(KeyCode::Char)
        .into_iter()
        .chain([KeyCode::Left])
    {
        let mut state = Selection::new(3, 1);
        assert_eq!(
            state.apply(key(code), 3, 2),
            SelectionAction::Cancel,
            "{code:?} means back, which in a question means not answering it"
        );
    }
}

/// Navigation has the same aliases in lists as in menus.
#[test]
fn listings_support_vim_and_wasd_movement_and_back() {
    use app::listing::state::{Flow, State, apply, key_for};

    for letter in ['h', 'H', 'a', 'A', 'p', 'P', 'l', 'L', 'd', 'D'] {
        let mut state = State::new(String::new());
        apply(&mut state, key_for(key(KeyCode::Char(letter))), 5, 2);
        assert_eq!(state.cursor, 0, "`{letter}` must not move the cursor");
    }
    for code in ['h', 'H', 'a', 'A']
        .map(KeyCode::Char)
        .into_iter()
        .chain([KeyCode::Left])
    {
        let mut state = State::new(String::new());
        assert_eq!(
            apply(&mut state, key_for(key(code)), 5, 2),
            Flow::Quit,
            "{code:?} leaves the list, as Escape does"
        );
    }
    for (letter, cursor) in [
        ('j', 2),
        ('J', 2),
        ('s', 2),
        ('S', 2),
        ('k', 0),
        ('K', 0),
        ('w', 0),
        ('W', 0),
    ] {
        let mut state = State::new(String::new());
        state.cursor = 1;
        assert_eq!(
            apply(&mut state, key_for(key(KeyCode::Char(letter))), 5, 2),
            Flow::Stay
        );
        assert_eq!(state.cursor, cursor, "{letter}");
    }
}

/// A list being searched is typing, so every letter is text there.
#[test]
fn navigation_letters_are_text_while_a_listing_is_being_searched() {
    use app::listing::state::{State, apply, key_for};

    let mut state = State::new(String::new());
    state.searching = true;
    for letter in "hush wasd WASD".chars() {
        apply(&mut state, key_for(key(KeyCode::Char(letter))), 5, 2);
    }
    assert_eq!(state.query, "hush wasd WASD");
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
fn lists_move_a_half_screen_and_jump_to_either_end() {
    use app::listing::state::{State, apply, key_for};
    let mut state = State::new(String::new());
    apply(&mut state, key_for(key(KeyCode::Char('G'))), 5, 2);
    assert_eq!(state.cursor, 4, "`G` is the last row");
    apply(&mut state, key_for(key(KeyCode::Char('u'))), 5, 2);
    assert_eq!(state.cursor, 2, "`u` is half a screen back");
    apply(&mut state, key_for(key(KeyCode::Char('g'))), 5, 2);
    assert_eq!(state.cursor, 0, "`g` is the first row");
}

/// The dashboard is the one screen with a left and a right, so `h` and `l`
/// mean them. `j` and `k` still scroll, on every tab including the Log.
#[test]
fn dashboard_h_and_l_move_between_tabs_on_every_tab() {
    use app::dashboard::state::{Action, Tab, action_for_tab};
    for tab in Tab::ALL {
        assert!(
            matches!(
                action_for_tab(key(KeyCode::Char('h')), tab),
                Action::Show(_)
            ),
            "`h` is left, not down, on the {} tab",
            tab.title()
        );
        assert!(matches!(
            action_for_tab(key(KeyCode::Left), tab),
            Action::Show(_)
        ));
        assert!(matches!(
            action_for_tab(key(KeyCode::Char('l')), tab),
            Action::Show(_)
        ));
        assert_eq!(
            action_for_tab(key(KeyCode::Char('j')), tab),
            Action::ScrollDown,
            "{}",
            tab.title()
        );
        assert_eq!(
            action_for_tab(key(KeyCode::Char('k')), tab),
            Action::ScrollUp,
            "{}",
            tab.title()
        );
    }
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

#[test]
fn long_selected_labels_wrap_inside_the_viewport() {
    use app::prompt::select::{MenuItem, render};
    let items = vec![MenuItem {
        label: "unique beginning and important ending".into(),
        detail: None,
    }];
    let lines = render(
        "Choose",
        &items,
        &Selection::new(1, 0),
        20,
        8,
        app::theme::Theme::dark(false),
    );
    assert!(lines.join(" ").contains("ending"));
    assert!(lines.len() <= 8);
    assert!(
        lines
            .iter()
            .all(|line| ratatui::text::Line::raw(line).width() < 20)
    );
}

#[test]
fn bold_menu_highlight_follows_arrow_navigation() {
    use app::prompt::select::{MenuItem, render};
    use app::theme::Theme;

    let items = ["First", "Second"].map(|label| MenuItem {
        label: label.into(),
        detail: None,
    });
    let mut state = Selection::new(items.len(), 0);
    for (key, cursor) in [(KeyCode::Down, 1), (KeyCode::Up, 0)] {
        state.apply(KeyEvent::new(key, KeyModifiers::NONE), items.len(), 10);
        let lines = render("Choose", &items, &state, 80, 10, Theme::dark(true));
        assert_eq!(
            lines[cursor + 1],
            format!("\x1b[1;97m> {}. {}\x1b[0m", cursor + 1, items[cursor].label)
        );
        assert!(!lines[2 - cursor].contains('\x1b'));
        let plain = render("Choose", &items, &state, 80, 10, Theme::dark(false));
        assert!(plain.iter().all(|line| !line.contains('\x1b')));
    }
}

#[test]
fn wrapped_highlighted_menu_labels_are_bold_on_every_line() {
    use app::prompt::select::{MenuItem, render};
    use app::theme::Theme;

    let items = [MenuItem {
        label: "A long menu label that wraps".into(),
        detail: None,
    }];
    let lines = render(
        "Choose",
        &items,
        &Selection::new(1, 0),
        15,
        20,
        Theme::dark(true),
    );
    assert!(lines.len() > 3);
    for line in &lines[1..lines.len() - 1] {
        assert!(line.starts_with("\x1b[1;97m"));
        assert!(line.ends_with("\x1b[0m"));
    }
}
