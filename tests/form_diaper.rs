//! A diaper correction retains every applicable answer and its unfinished note.
use app::{
    commands::diaper_form::{DiaperForm, DiaperInput},
    domain::Calendar,
    prompt::{
        flow::{FormEvent, FormState},
        host::Input,
    },
    theme::Theme,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
type Form = FormState<DiaperForm>;
fn form(potty: bool) -> Form {
    Form::create_form(
        DiaperForm::create_diaper(
            Calendar::new("UTC").unwrap(),
            DiaperInput {
                potty,
                ..DiaperInput::default()
            },
        ),
        10000.0,
    )
    .unwrap()
}
fn press(form: &mut Form, code: KeyCode) -> FormEvent {
    form.apply(
        Input::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        10000.0,
        (80, 24),
    )
}
fn choose(form: &mut Form, digit: char) {
    press(form, KeyCode::Char(digit));
    press(form, KeyCode::Enter);
}
fn back(form: &mut Form, count: usize) {
    for _ in 0..count {
        assert_eq!(press(form, KeyCode::Left), FormEvent::Stay);
    }
}
fn shown(form: &Form) -> String {
    form.render(10000.0, (80, 24), Theme::dark(false))
        .join("\n")
}
fn dirty_note() -> Form {
    let mut form = form(false);
    press(&mut form, KeyCode::Enter); // now
    choose(&mut form, '2'); // dirty
    choose(&mut form, '3'); // medium
    choose(&mut form, '3'); // brown
    press(&mut form, KeyCode::Enter); // skip consistency
    press(&mut form, KeyCode::Enter); // no rash
    form.apply(Input::Pasted("remember this".into()), 10000.0, (80, 24));
    form
}
#[test]
fn diaper_back_edits_time_and_prefills_later_views() {
    let mut form = dirty_note();
    back(&mut form, 6);
    for _ in 0..20 {
        press(&mut form, KeyCode::Up);
    }
    press(&mut form, KeyCode::Enter);
    for _ in 0..5 {
        press(&mut form, KeyCode::Enter);
    }
    assert!(shown(&form).contains("remember this"));
    assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
    let submission = form.complete().unwrap();
    assert!((submission.at - 8800.0).abs() < f64::EPSILON);
    assert_eq!(submission.details.color.unwrap().as_str(), "brown");
    assert_eq!(submission.details.notes.as_deref(), Some("remember this"));
}
#[test]
fn wet_submission_omits_remembered_dirty_details() {
    let mut form = dirty_note();
    back(&mut form, 5);
    choose(&mut form, '1');
    for _ in 0..2 {
        press(&mut form, KeyCode::Enter);
    }
    assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
    let details = form.complete().unwrap().details;
    assert!(details.color.is_none());
    assert!(details.poo_amount.is_none());
    assert_eq!(details.notes.as_deref(), Some("remember this"));
}
#[test]
fn dirty_restores_hidden_answers() {
    let mut form = dirty_note();
    back(&mut form, 5);
    choose(&mut form, '1');
    back(&mut form, 1);
    choose(&mut form, '2');
    for _ in 0..4 {
        press(&mut form, KeyCode::Enter);
    }
    assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
    let details = form.complete().unwrap().details;
    assert_eq!(details.color.unwrap().as_str(), "brown");
    assert_eq!(details.poo_amount.unwrap().to_string(), "medium");
}
#[test]
fn potty_preserves_outcome_without_rash() {
    let mut form = form(true);
    press(&mut form, KeyCode::Enter);
    choose(&mut form, '2');
    choose(&mut form, '3');
    back(&mut form, 1);
    press(&mut form, KeyCode::Enter);
    for _ in 0..3 {
        press(&mut form, KeyCode::Enter);
    }
    assert!(shown(&form).contains("Anything to note?"));
    assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
    let submission = form.complete().unwrap();
    assert_eq!(submission.potty.unwrap().to_api().as_str(), "accident");
    assert!(!submission.details.rash);
}
