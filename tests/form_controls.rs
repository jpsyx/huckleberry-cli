//! Navigation belongs to the form while text and dial editing keep their keys.
use app::{
    prompt::{
        flow::{ControlEvent, FormValue, View},
        host::Input,
    },
    theme::Theme,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

const fn key(code: KeyCode) -> Input {
    Input::Key(KeyEvent::new(code, KeyModifiers::NONE))
}
fn choice() -> View {
    View::create_choices(
        "Pick",
        vec![("Yes".into(), FormValue::Choice("yes".into()))],
        0,
    )
}

#[test]
fn choice_back_and_home_are_distinct() {
    for code in [
        KeyCode::Left,
        KeyCode::Char('h'),
        KeyCode::Char('a'),
        KeyCode::Char('H'),
        KeyCode::Char('A'),
    ] {
        assert_eq!(
            choice().apply(key(code), 1000.0, (120, 24)),
            ControlEvent::Back
        );
    }
    assert_eq!(
        choice().apply(key(KeyCode::Esc), 1000.0, (120, 24)),
        ControlEvent::Home
    );
    for code in [
        KeyCode::Enter,
        KeyCode::Right,
        KeyCode::Char('l'),
        KeyCode::Char('d'),
        KeyCode::Char(' '),
    ] {
        assert_eq!(
            choice().apply(key(code), 1000.0, (120, 24)),
            ControlEvent::Accept
        );
    }
    let hints = choice()
        .render(1000.0, (120, 24), Theme::dark(false))
        .join("\n");
    assert!(hints.contains("Esc home"), "{hints}");
}

#[test]
fn text_back_keeps_letters_and_paste() {
    let mut view = View::create_text("Notes", "", false);
    for character in "haha quiet".chars() {
        view.apply(key(KeyCode::Char(character)), 1000.0, (120, 24));
    }
    view.apply(Input::Pasted("first\nsecond".into()), 1000.0, (120, 24));
    assert_eq!(
        view.apply(key(KeyCode::Left), 1000.0, (120, 24)),
        ControlEvent::Back
    );
    assert!(
        matches!(view.value(1000.0).unwrap(), FormValue::Text(value) if value == "haha quietfirstsecond")
    );
    let shown = view
        .render(1000.0, (120, 24), Theme::dark(false))
        .join("\n");
    assert!(shown.contains("← back"), "{shown}");
    assert!(shown.contains("Esc home"), "{shown}");
}

#[test]
fn reopened_secret_is_masked() {
    let mut view = View::create_text("Password", "secret-value", true);
    view.apply(key(KeyCode::Left), 1000.0, (120, 24));
    let shown = view
        .render(1000.0, (120, 24), Theme::dark(false))
        .join("\n");
    assert!(shown.contains("************"));
    assert!(!shown.contains("secret-value"));
    assert!(
        matches!(view.value(1000.0).unwrap(), FormValue::Text(value) if value == "secret-value")
    );
}

#[test]
fn control_chords_do_not_become_navigation_letters() {
    let mut view = choice();
    let control = |character| {
        Input::Key(KeyEvent::new(
            KeyCode::Char(character),
            KeyModifiers::CONTROL,
        ))
    };
    assert_eq!(
        view.apply(control('h'), 1000.0, (120, 24)),
        ControlEvent::Stay
    );
    assert_eq!(
        view.apply(control('c'), 1000.0, (120, 24)),
        ControlEvent::Cancel
    );
}

#[test]
fn retained_choice_keeps_its_question_inside_the_panel() {
    use app::prompt::flow::{FormValue, View};
    let view = View::create_choices(
        "Which setting?",
        (0..10)
            .map(|index| {
                (
                    format!("Setting {index}"),
                    FormValue::Choice(index.to_string()),
                )
            })
            .collect(),
        0,
    );
    let lines = view.render(10000.0, (80, 10), app::theme::Theme::dark(false));
    assert!(
        lines.len() <= 10,
        "a second footer must not push the question out of the panel: {lines:?}"
    );
    assert!(lines[0].contains("Which setting?"));
}
