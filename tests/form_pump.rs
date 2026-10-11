//! Pump modes and custom duration are independent retained branches.
use app::{
    cli::{PumpValues, Units},
    commands::pump::form::{PumpDefaults, PumpForm, PumpInput, PumpOperation},
    domain::Calendar,
    prompt::{
        flow::{FormEvent, FormState},
        host::Input,
    },
    theme::Theme,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use huckleberry_api::models::pump::PumpAmounts;
type Form = FormState<PumpForm>;
fn form() -> Form {
    Form::create_form(
        PumpForm::create_pump(
            Calendar::new("UTC").unwrap(),
            PumpInput {
                operation: PumpOperation::Log,
                at: None,
                duration: None,
                values: PumpValues::default(),
            },
            PumpDefaults {
                units: Units::Ml,
                total: Some(90.0),
                left: Some(0.0),
                right: Some(60.0),
            },
        )
        .unwrap(),
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
        press(form, KeyCode::Left);
    }
}
fn at_duration() -> Form {
    let mut form = form();
    for _ in 0..3 {
        press(&mut form, KeyCode::Enter);
    }
    form
}
#[test]
fn pump_modes_keep_separate_answers() {
    let mut form = at_duration();
    back(&mut form, 2);
    choose(&mut form, '2');
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Enter);
    back(&mut form, 3);
    choose(&mut form, '1');
    for _ in 0..2 {
        press(&mut form, KeyCode::Enter);
    }
    assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
    assert_eq!(
        form.complete().unwrap().amounts,
        Some(PumpAmounts::Total(90.0))
    );
}
#[test]
fn pump_zero_side_survives_back() {
    let mut form = at_duration();
    back(&mut form, 2);
    choose(&mut form, '2');
    press(&mut form, KeyCode::Enter);
    back(&mut form, 1);
    for _ in 0..3 {
        press(&mut form, KeyCode::Enter);
    }
    assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
    assert_eq!(
        form.complete().unwrap().amounts,
        Some(PumpAmounts::LeftRight {
            left: 0.0,
            right: 60.0
        })
    );
}
#[test]
fn custom_duration_returns_to_presets() {
    let mut form = at_duration();
    choose(&mut form, '8');
    form.apply(Input::Pasted("1.25".into()), 10000.0, (80, 24));
    back(&mut form, 1);
    assert!(
        form.render(10000.0, (80, 24), Theme::dark(false))
            .join("\n")
            .contains("How long?")
    );
    press(&mut form, KeyCode::Enter);
    assert!(
        form.render(10000.0, (80, 24), Theme::dark(false))
            .join("\n")
            .contains("1.25")
    );
    press(&mut form, KeyCode::Enter);
    assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
    assert_eq!(form.complete().unwrap().duration, Some(75.0));
}
#[test]
fn skipped_duration_omits_a_remembered_custom_answer() {
    let mut form = at_duration();
    choose(&mut form, '8');
    form.apply(Input::Pasted("1.25".into()), 10000.0, (80, 24));
    press(&mut form, KeyCode::Enter);
    back(&mut form, 2);
    choose(&mut form, '1');
    assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
    assert_eq!(form.complete().unwrap().duration, None);
}

#[test]
fn one_flagged_zero_side_keeps_its_quantity_when_the_other_changes_units() {
    let model = PumpForm::create_pump(
        Calendar::new("UTC").unwrap(),
        PumpInput {
            operation: PumpOperation::Log,
            at: Some(8800.0),
            duration: Some(75.0),
            values: PumpValues {
                left: Some(0.0),
                units: Some(Units::Ml),
                notes: Some(String::new()),
                ..PumpValues::default()
            },
        },
        PumpDefaults {
            units: Units::Ml,
            total: None,
            left: None,
            right: Some(60.0),
        },
    )
    .unwrap();
    let mut form = FormState::create_form(model, 10000.0).unwrap();
    press(&mut form, KeyCode::Right);
    press(&mut form, KeyCode::Up);
    assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
    let result = form.complete().unwrap();
    assert_eq!(result.units, Units::Ml);
    let Some(PumpAmounts::LeftRight { left, right }) = result.amounts else {
        panic!("separate amounts")
    };
    assert!(left.abs() < f64::EPSILON);
    assert!((right - 60.0).abs() < 1e-9);
}
