//! Revisiting sleep times retains the end and revalidates it against a new start.
use app::{
    commands::sleep_form::{SleepForm, SleepInput, SleepOperation},
    domain::Calendar,
    prompt::{
        flow::{FormEvent, FormModel, FormState, FormValue, fields},
        host::Input,
    },
    theme::Theme,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use huckleberry_api::models::SleepTimer;
use serde_json::json;
fn model(timer: bool) -> SleepForm {
    let running: Option<SleepTimer> = timer.then(|| {
        serde_json::from_value(json!({"active":true,"paused":false,
        "uuid":"test","timerStartTime":900_000.0}))
        .unwrap()
    });
    let mut model = SleepForm::create_sleep(
        Calendar::new("UTC").unwrap(),
        SleepInput {
            operation: SleepOperation::Manual,
            start: None,
            end: None,
            given_end: None,
            overlap: None,
        },
        running,
        2000.0,
    )
    .unwrap();
    model
        .accept(
            &fields::create_id("start"),
            FormValue::Instant(1000.0),
            2000.0,
        )
        .unwrap();
    model
        .accept(
            &fields::create_id("end"),
            FormValue::Instant(1600.0),
            2000.0,
        )
        .unwrap();
    model
}
fn press(form: &mut FormState<SleepForm>, code: KeyCode, now: f64) -> FormEvent {
    form.apply(
        Input::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        now,
        (80, 24),
    )
}
#[test]
fn moving_start_revalidates_retained_end() {
    let mut form = FormState::create_form(model(false), 2000.0).unwrap();
    press(&mut form, KeyCode::Enter, 2000.0);
    press(&mut form, KeyCode::Left, 2000.0);
    press(&mut form, KeyCode::Left, 2000.0);
    press(&mut form, KeyCode::Char('n'), 1700.0);
    press(&mut form, KeyCode::Enter, 1700.0);
    assert!(form.complete().is_err());
    assert!(
        form.render(1700.0, (80, 24), Theme::dark(false))
            .join("\n")
            .contains("after it began")
    );
    assert_eq!(press(&mut form, KeyCode::Enter, 1700.0), FormEvent::Stay);
    press(&mut form, KeyCode::Char('n'), 1800.0);
    assert_eq!(press(&mut form, KeyCode::Enter, 1800.0), FormEvent::Ready);
    let answer = form.complete().unwrap();
    assert_eq!(answer.start, Some(1700.0));
    assert_eq!(answer.end, Some(1800.0));
}
#[test]
fn back_from_overlap_is_read_only() {
    let mut form = FormState::create_form(model(true), 2000.0).unwrap();
    press(&mut form, KeyCode::Enter, 2000.0);
    press(&mut form, KeyCode::Enter, 2000.0);
    assert!(
        form.render(2000.0, (80, 24), Theme::dark(false))
            .join("\n")
            .contains("Which one")
    );
    assert_eq!(press(&mut form, KeyCode::Left, 2000.0), FormEvent::Stay);
    assert!(
        form.render(2000.0, (80, 24), Theme::dark(false))
            .join("\n")
            .contains("When did it end?")
    );
    press(&mut form, KeyCode::Enter, 2000.0);
    assert_eq!(press(&mut form, KeyCode::Esc, 2000.0), FormEvent::Home);
}
#[test]
fn overlap_cancel_is_cancellation_not_a_validation_error() {
    let mut form = FormState::create_form(model(true), 2000.0).unwrap();
    press(&mut form, KeyCode::Enter, 2000.0);
    press(&mut form, KeyCode::Enter, 2000.0);
    assert_eq!(press(&mut form, KeyCode::Enter, 2000.0), FormEvent::Cancel);
}
#[test]
fn end_flag_resolves_once_after_the_first_start_answer() {
    let mut model = SleepForm::create_sleep(
        Calendar::new("UTC").unwrap(),
        SleepInput {
            operation: SleepOperation::Manual,
            start: None,
            end: None,
            given_end: Some("10 min ago".into()),
            overlap: None,
        },
        None,
        2000.0,
    )
    .unwrap();
    model
        .accept(
            &fields::create_id("start"),
            FormValue::Instant(1000.0),
            2000.0,
        )
        .unwrap();
    model
        .accept(
            &fields::create_id("start"),
            FormValue::Instant(1100.0),
            2300.0,
        )
        .unwrap();
    assert_eq!(model.complete().unwrap().end, Some(1400.0));
}

#[test]
fn overlap_explanation_is_part_of_the_retained_view() {
    let mut form = FormState::create_form(model(true), 2000.0).unwrap();
    press(&mut form, KeyCode::Enter, 2000.0);
    press(&mut form, KeyCode::Enter, 2000.0);
    assert!(
        form.render(2000.0, (80, 24), Theme::dark(false))
            .join("\n")
            .contains("overlaps the sleep in progress")
    );
}
