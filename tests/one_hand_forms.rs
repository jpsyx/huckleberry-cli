use app::commands::feed::solids::{FoodAction, FoodSelection};

#[test]
fn meal_can_add_remove_and_finish_multiple_foods() {
    let mut foods = FoodSelection { selected: vec![] };
    assert!(!foods.apply(FoodAction::Done));
    foods.apply(FoodAction::Add("Avocado".into()));
    foods.apply(FoodAction::Add("Banana".into()));
    foods.apply(FoodAction::Remove(0));
    assert_eq!(foods.selected, ["Banana"]);
    assert!(foods.apply(FoodAction::Done));
}

#[test]
fn settings_units_measurements_and_verbose_are_choices() {
    for (name, expected) in [
        ("units", vec!["ml", "oz"]),
        ("measurements", vec!["metric", "imperial"]),
        ("verbose", vec!["true", "false"]),
    ] {
        let choices = app::commands::settings::value_choices(name);
        assert_eq!(
            choices
                .iter()
                .map(|choice| choice.value)
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn sleep_overlap_starts_on_cancel() {
    assert_eq!(app::commands::sleep::overlap_choices()[0].value, "cancel");
}

#[test]
fn keeping_a_field_preserves_unknown_wire_values() {
    use app::commands::edit::preserve::retained;
    assert_eq!(
        retained(Some("Future Milk"), "Formula", "Formula", false),
        Some("Future Milk".into())
    );
    assert_eq!(
        retained(Some("Future Milk"), "Formula", "Breast Milk", false),
        Some("Breast Milk".into())
    );
    assert_eq!(retained(Some("Future Color"), "", "", true), None);
}
