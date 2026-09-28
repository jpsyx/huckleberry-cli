use crossterm::event::{KeyCode, KeyModifiers};

use super::*;

const SETTINGS: [Choice<'static>; 2] = [
    Choice {
        value: "greeting",
        hint: "the opening word",
    },
    Choice {
        value: "verbose",
        hint: "more detail",
    },
];

#[test]
fn free_text_is_taken_as_typed_once_trimmed() {
    assert_eq!(
        interpret("  Ada \n", &[], None),
        Reply::Accepted("Ada".to_owned())
    );
}

#[test]
fn an_empty_answer_takes_the_default() {
    assert_eq!(
        interpret("\n", &[], Some("Hello")),
        Reply::Accepted("Hello".to_owned())
    );
}

#[test]
fn an_empty_answer_without_a_default_leaves_the_question_standing() {
    assert_eq!(interpret("   \n", &[], None), Reply::Blank);
}

#[test]
fn a_choice_can_be_picked_by_number() {
    assert_eq!(
        interpret("2", &SETTINGS, None),
        Reply::Accepted("verbose".to_owned())
    );
}

#[test]
fn a_choice_can_be_picked_by_name() {
    assert_eq!(
        interpret("greeting", &SETTINGS, None),
        Reply::Accepted("greeting".to_owned())
    );
}

#[test]
fn a_number_outside_the_list_is_not_a_choice() {
    for answer in ["0", "3", "-1"] {
        assert_eq!(
            interpret(answer, &SETTINGS, None),
            Reply::Unknown(answer.to_owned()),
            "{answer} should not pick anything"
        );
    }
}

#[test]
fn something_else_entirely_is_reported_back() {
    assert_eq!(
        interpret("verbos", &SETTINGS, None),
        Reply::Unknown("verbos".to_owned())
    );
}

#[test]
fn the_rendered_question_numbers_every_choice_and_shows_the_default() {
    let question = Question::new("setting to change", "Which setting?", "<KEY>")
        .with_choices(&SETTINGS)
        .with_default("greeting");
    let text = render(&question, Theme::dark(false));
    assert!(text.starts_with("Which setting?\n"), "{text}");
    assert!(text.contains("1) greeting  the opening word"), "{text}");
    assert!(text.contains("2) verbose   more detail"), "{text}");
    assert!(text.ends_with("[greeting] > "), "{text}");
}

#[test]
fn helper_text_is_muted_and_keeps_the_default_on_the_answer_line() {
    let help = "E.g. '1:23 pm' or '123pm' or '32 min ago' are all valid";
    let question = Question::new("time", "When?", "--at <TIME>")
        .with_help(help)
        .with_default("now");
    for color in [false, true] {
        let theme = Theme::dark(color);
        assert_eq!(
            render(&question, theme),
            format!(
                "{}\n{}\n[{}] > ",
                theme.prompt("When?"),
                theme.muted(help),
                theme.value("now")
            )
        );
    }
}

#[test]
fn an_optional_question_says_that_enter_skips_it() {
    let question = Question::new("colour", "What colour?", "--color <COLOUR>").optional();
    let text = render(&question, Theme::dark(false));
    assert!(text.contains("(enter to skip)"), "{text}");
    assert!(text.ends_with("> "), "{text}");
}

#[test]
fn a_question_with_a_default_shows_the_default_rather_than_the_skip_hint() {
    let question = Question::new("colour", "What colour?", "--color <COLOUR>")
        .optional()
        .with_default("yellow");
    let text = render(&question, Theme::dark(false));
    assert!(!text.contains("(enter to skip)"), "{text}");
    assert!(text.ends_with("[yellow] > "), "{text}");
}

#[test]
fn a_free_text_question_renders_a_bare_prompt() {
    let question = Question::new("name to greet", "Who should I greet?", "--name <NAME>");
    let text = render(&question, Theme::dark(false));
    assert_eq!(text, "Who should I greet?\n> ");
}

#[test]
fn a_typed_character_becomes_one_asterisk_of_the_mask() {
    assert_eq!(
        interpret_keystroke(KeyCode::Char('h'), KeyModifiers::NONE),
        Keystroke::Add('h')
    );
    assert_eq!(
        interpret_keystroke(KeyCode::Char('H'), KeyModifiers::SHIFT),
        Keystroke::Add('H'),
        "shift is how a capital arrives, not a control key"
    );
}

#[test]
fn control_c_abandons_rather_than_submitting_what_was_typed_so_far() {
    assert_eq!(
        interpret_keystroke(KeyCode::Char('c'), KeyModifiers::CONTROL),
        Keystroke::Cancel
    );
    assert_eq!(
        interpret_keystroke(KeyCode::Char('d'), KeyModifiers::CONTROL),
        Keystroke::Cancel
    );
    assert_eq!(
        interpret_keystroke(KeyCode::Esc, KeyModifiers::NONE),
        Keystroke::Cancel
    );
}

#[test]
fn a_control_character_carrying_a_letter_is_not_that_letter() {
    // The trap this guards: `Ctrl-a` must not silently become an `a` in
    // somebody's password.
    assert_eq!(
        interpret_keystroke(KeyCode::Char('a'), KeyModifiers::CONTROL),
        Keystroke::Ignore
    );
}

#[test]
fn backspace_takes_a_character_back() {
    assert_eq!(
        interpret_keystroke(KeyCode::Backspace, KeyModifiers::NONE),
        Keystroke::Remove
    );
    assert_eq!(
        interpret_keystroke(KeyCode::Char('u'), KeyModifiers::CONTROL),
        Keystroke::Remove
    );
}

#[test]
fn enter_finishes_the_answer() {
    assert_eq!(
        interpret_keystroke(KeyCode::Enter, KeyModifiers::NONE),
        Keystroke::Submit
    );
}

#[test]
fn a_key_with_no_meaning_here_is_ignored() {
    for code in [KeyCode::Up, KeyCode::F(1), KeyCode::Tab] {
        assert_eq!(
            interpret_keystroke(code, KeyModifiers::NONE),
            Keystroke::Ignore,
            "{code:?}"
        );
    }
}

#[test]
fn a_confirmation_takes_a_yes_or_a_no_and_defaults_otherwise() {
    assert!(interpret_confirmation("y\n", false));
    assert!(interpret_confirmation("YES", false));
    assert!(!interpret_confirmation("n", true));
    assert!(!interpret_confirmation("No\n", true));
    assert!(
        interpret_confirmation("\n", true),
        "an empty answer takes the default"
    );
    assert!(
        !interpret_confirmation("maybe", false),
        "so does anything else"
    );
}

#[test]
fn the_unanswerable_failure_names_the_flag() {
    let question = Question::new("name to greet", "Who should I greet?", "--name <NAME>");
    assert_eq!(
        unanswerable_message(&question),
        "no name to greet: pass --name <NAME> (stdin is not a terminal, so I cannot ask)"
    );
}
