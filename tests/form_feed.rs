//! Feeding drafts retain accepted quantities and sides through time corrections.
use app::{
    cli::{BottleKind, Side, Units},
    commands::feed::{
        bottle_form::{BottleDefaults, BottleForm, BottleInput},
        nursing_form::{NursingForm, NursingInput, NursingOperation},
    },
    domain::Calendar,
    prompt::{
        flow::{FormEvent, FormModel, FormState},
        host::Input,
    },
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
fn press<Form: FormModel>(form: &mut FormState<Form>, code: KeyCode) -> FormEvent {
    form.apply(
        Input::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        10000.0,
        (80, 24),
    )
}
#[test]
fn bottle_back_retains_volume_kind_and_notes() {
    let model = BottleForm::create_bottle(
        Calendar::new("UTC").unwrap(),
        BottleInput::default(),
        BottleDefaults {
            units: Units::Ml,
            amount: Some(90.125),
            kind: BottleKind::Formula,
        },
    )
    .unwrap();
    let mut form = FormState::create_form(model, 10000.0).unwrap();
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Char('2'));
    press(&mut form, KeyCode::Enter);
    form.apply(Input::Pasted("quiet feed".into()), 10000.0, (80, 24));
    for _ in 0..3 {
        press(&mut form, KeyCode::Left);
    }
    for _ in 0..20 {
        press(&mut form, KeyCode::Up);
    }
    for _ in 0..3 {
        press(&mut form, KeyCode::Enter);
    }
    assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
    let result = form.complete().unwrap();
    assert!((result.at - 8800.0).abs() < f64::EPSILON);
    assert!((result.amount - 90.125).abs() < f64::EPSILON);
    assert_eq!(result.kind, BottleKind::BreastMilk);
    assert_eq!(result.notes.as_deref(), Some("quiet feed"));
}
#[test]
fn nursing_back_retains_time_and_side() {
    let model = NursingForm::create_nursing(
        Calendar::new("UTC").unwrap(),
        NursingInput {
            operation: NursingOperation::Start,
            at: None,
            side: None,
        },
        Side::Right,
    );
    let mut form = FormState::create_form(model, 10000.0).unwrap();
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Char('1')); // unfinished side selection
    press(&mut form, KeyCode::Left);
    for _ in 0..20 {
        press(&mut form, KeyCode::Up);
    }
    press(&mut form, KeyCode::Enter);
    assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
    let result = form.complete().unwrap();
    assert_eq!(result.at, Some(8800.0));
    assert_eq!(result.side, Some(Side::Left));
}
#[test]
fn bottle_flagged_units_are_not_asked_again() {
    let model = BottleForm::create_bottle(
        Calendar::new("UTC").unwrap(),
        BottleInput {
            at: Some(8800.0),
            units: Some(Units::Oz),
            kind: Some(BottleKind::Formula),
            notes: Some(String::new()),
            ..BottleInput::default()
        },
        BottleDefaults {
            units: Units::Ml,
            amount: Some(2.5),
            kind: BottleKind::Formula,
        },
    )
    .unwrap();
    let mut form = FormState::create_form(model, 10000.0).unwrap();
    assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
    let result = form.complete().unwrap();
    assert_eq!(result.units, Units::Oz);
    assert!((result.amount - 2.5).abs() < f64::EPSILON);
}
