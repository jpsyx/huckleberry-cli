//! Nested food views and measurement units preserve a draft's meaning.
use app::{
    cli::System,
    commands::{
        feed::solids_form::{SolidsForm, SolidsInput},
        growth_form::{GrowthForm, GrowthInput},
    },
    domain::Calendar,
    prompt::{
        flow::{FormEvent, FormModel, FormState, FormValue, fields},
        host::Input,
    },
    theme::Theme,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use huckleberry_api::GrowthMeasurements;
fn press<Form: FormModel>(form: &mut FormState<Form>, code: KeyCode) -> FormEvent {
    form.apply(
        Input::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        10000.0,
        (80, 24),
    )
}
fn paste<Form: FormModel>(form: &mut FormState<Form>, text: &str) {
    form.apply(Input::Pasted(text.into()), 10000.0, (80, 24));
}
fn solids() -> FormState<SolidsForm> {
    FormState::create_form(
        SolidsForm::create_solids(
            Calendar::new("UTC").unwrap(),
            SolidsInput {
                at: Some(9000.0),
                ..Default::default()
            },
            vec![],
        ),
        10000.0,
    )
    .unwrap()
}
#[test]
fn food_custom_entry_returns_to_selection() {
    let mut form = solids();
    press(&mut form, KeyCode::Enter);
    paste(&mut form, "Avocado");
    press(&mut form, KeyCode::Left);
    press(&mut form, KeyCode::Enter);
    assert!(
        form.render(10000.0, (80, 24), Theme::dark(false))
            .join("\n")
            .contains("Avocado")
    );
    press(&mut form, KeyCode::Enter);
    assert!(
        form.render(10000.0, (80, 24), Theme::dark(false))
            .join("\n")
            .contains("Foods:")
    );
}
#[test]
fn food_selection_retains_items() {
    let mut form = solids();
    press(&mut form, KeyCode::Enter);
    paste(&mut form, "Avocado");
    press(&mut form, KeyCode::Enter);
    // The cursor follows the semantic "new" row as Done appears above it.
    press(&mut form, KeyCode::Enter);
    paste(&mut form, "Pear");
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Char('1'));
    press(&mut form, KeyCode::Enter); // Done
    press(&mut form, KeyCode::Enter); // Some
    press(&mut form, KeyCode::Enter); // Skip reaction
    paste(&mut form, "lunch");
    press(&mut form, KeyCode::Left);
    press(&mut form, KeyCode::Left);
    press(&mut form, KeyCode::Left);
    for _ in 0..3 {
        press(&mut form, KeyCode::Enter);
    }
    assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
    assert_eq!(
        form.complete()
            .unwrap()
            .foods
            .iter()
            .map(|food| food.name.as_str())
            .collect::<Vec<_>>(),
        ["Avocado", "Pear"]
    );
}
fn growth() -> GrowthForm {
    GrowthForm::create_growth(
        Calendar::new("UTC").unwrap(),
        GrowthInput {
            at: Some(9000.0),
            measurements: GrowthMeasurements::default(),
            system: None,
        },
        System::Metric,
    )
    .unwrap()
}
#[test]
fn growth_units_preserve_measurements() {
    let mut model = growth();
    model
        .accept(
            &fields::create_id("system"),
            FormValue::Choice("metric".into()),
            10000.0,
        )
        .unwrap();
    model
        .accept(
            &fields::create_id("weight"),
            FormValue::Number(0.453_592_37),
            10000.0,
        )
        .unwrap();
    model
        .accept(
            &fields::create_id("height"),
            FormValue::Number(2.54),
            10000.0,
        )
        .unwrap();
    model
        .accept(&fields::create_id("head"), FormValue::Skipped, 10000.0)
        .unwrap();
    for system in ["imperial", "metric", "imperial"] {
        model
            .accept(
                &fields::create_id("system"),
                FormValue::Choice(system.into()),
                10000.0,
            )
            .unwrap();
        let answer = model.complete().unwrap();
        let expected = if system == "imperial" {
            (1.0, 1.0)
        } else {
            (0.453_592_37, 2.54)
        };
        assert!((answer.measurements.weight.unwrap() - expected.0).abs() < 1e-9);
        assert!((answer.measurements.height.unwrap() - expected.1).abs() < 1e-9);
    }
}
#[test]
fn optional_measurements_stay_cleared() {
    let mut form = FormState::create_form(growth(), 10000.0).unwrap();
    press(&mut form, KeyCode::Enter);
    paste(&mut form, "0.45359237");
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Enter); // height Skip
    press(&mut form, KeyCode::Left);
    press(&mut form, KeyCode::Left);
    press(&mut form, KeyCode::Left);
    press(&mut form, KeyCode::Down);
    press(&mut form, KeyCode::Enter);
    let shown = form
        .render(10000.0, (80, 24), Theme::dark(false))
        .join("\n");
    assert!(shown.contains("pounds"), "{shown}");
    assert!(shown.contains("> 1"), "{shown}");
    for _ in 0..3 {
        press(&mut form, KeyCode::Enter);
    }
    let answer = form.complete().unwrap();
    assert_eq!(answer.measurements.height, None);
    assert_eq!(answer.measurements.head, None);
    assert!((answer.measurements.weight.unwrap() - 1.0).abs() < 1e-9);
}

#[test]
fn unfinished_numeric_text_keeps_its_original_units_until_corrected() {
    let mut form = FormState::create_form(growth(), 10000.0).unwrap();
    press(&mut form, KeyCode::Enter);
    paste(&mut form, "oops");
    press(&mut form, KeyCode::Left);
    press(&mut form, KeyCode::Down);
    press(&mut form, KeyCode::Enter);
    let shown = form
        .render(10000.0, (80, 24), Theme::dark(false))
        .join("\n");
    assert!(shown.contains("kilograms"), "{shown}");
    assert!(shown.contains("oops"), "{shown}");
    assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Stay);
    for _ in 0..4 {
        press(&mut form, KeyCode::Backspace);
    }
    paste(&mut form, "0.45359237");
    for _ in 0..3 {
        press(&mut form, KeyCode::Enter);
    }
    assert!((form.complete().unwrap().measurements.weight.unwrap() - 1.0).abs() < 1e-9);
}
