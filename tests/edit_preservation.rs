use app::{
    cli::DiaperKind,
    commands::edit::save,
    edit::{DiaperDraft, Draft, SolidsDraft},
};
use serde_json::json;

#[test]
fn notes_only_meal_edit_does_not_write_foods_amounts_or_reactions() {
    let meal = Draft::Solids(SolidsDraft {
        foods: vec!["Avocado".into()],
        amount: "some".into(),
        reaction: None,
        notes: Some("new note".into()),
    });
    let updates = save::build_updates(&meal, &["notes=new note".into()], None, &[], 123.0).unwrap();
    assert_eq!(
        updates
            .iter()
            .map(|update| update.path.join("."))
            .collect::<Vec<_>>(),
        ["notes", "lastUpdated"]
    );
}

fn potty() -> Draft {
    Draft::Diaper(DiaperDraft {
        potty: true,
        mode: DiaperKind::Pee,
        pee: None,
        poo: None,
        color: None,
        consistency: None,
        rash: false,
        how: None,
        notes: Some("note".into()),
    })
}

#[test]
fn keeping_known_and_unknown_potty_outcomes_never_writes_them() {
    for stored in ["accident", "satButDry", "futureOutcome"] {
        let mut row = json!({"howItHappened": stored, "notes": "old"});
        for update in
            save::build_updates(&potty(), &["notes=note".into()], None, &[], 123.0).unwrap()
        {
            assert_eq!(update.path.len(), 1);
            row[&update.path[0]] = update.value.unwrap();
        }
        assert_eq!(row["howItHappened"], stored);
    }
}

#[test]
fn clearing_potty_outcome_deletes_the_field() {
    let updates = save::build_updates(&potty(), &["how=".into()], None, &[], 123.0).unwrap();
    assert_eq!(updates[0].path, ["howItHappened"]);
    assert_eq!(updates[0].value, None);
}

#[test]
fn editing_meal_amount_preserves_curated_and_unknown_food_metadata() {
    let foods = json!({"curated-id": {"id":"curated-id", "created_name":"Avocado", "source":"curated", "amount": 0.25, "extra":"retain"}});
    let meal = Draft::Solids(SolidsDraft {
        foods: vec!["Avocado".into()],
        amount: "half".into(),
        reaction: None,
        notes: None,
    });
    let updates =
        save::build_updates(&meal, &["amount=half".into()], Some(&foods), &[], 123.0).unwrap();
    let mut expected = foods;
    expected["curated-id"]["amount"] = json!("half");
    assert_eq!(updates[0].path, ["foods"]);
    assert_eq!(updates[0].value, Some(expected));
}

#[test]
fn removing_one_food_preserves_the_remaining_foods_exactly() {
    let foods = json!({"a": {"id":"a", "created_name":"Avocado", "source":"curated", "amount":0.25}, "b": {"id":"b", "created_name":"Banana", "source":"custom"}});
    let meal = Draft::Solids(SolidsDraft {
        foods: vec!["Banana".into()],
        amount: "some".into(),
        reaction: None,
        notes: None,
    });
    let updates =
        save::build_updates(&meal, &["foods=Banana".into()], Some(&foods), &[], 123.0).unwrap();
    assert_eq!(updates[0].value, Some(json!({"b": foods["b"]})));
}

#[test]
fn all_scalar_edits_use_the_existing_api_field_paths() {
    use app::{
        cli::{BottleKind, Units},
        edit::{BottleDraft, NursingDraft, SleepDraft},
    };
    let examples = [
        (
            potty(),
            vec!["mode=pee", "color=", "consistency=", "how=", "notes=note"],
            vec![
                "mode",
                "color",
                "consistency",
                "howItHappened",
                "notes",
                "lastUpdated",
            ],
        ),
        (
            Draft::Bottle(BottleDraft {
                amount: 92.125,
                kind: BottleKind::Formula,
                units: Units::Ml,
                notes: None,
            }),
            vec!["amount=92.125", "type=Formula", "notes="],
            vec!["amount", "units", "bottleType", "notes", "lastUpdated"],
        ),
        (
            Draft::Nursing(NursingDraft {
                left_minutes: 1.125,
                right_minutes: 2.0,
                notes: None,
            }),
            vec!["left=1.125", "right=2", "notes="],
            vec!["leftDuration", "rightDuration", "notes", "lastUpdated"],
        ),
        (
            Draft::Sleep(SleepDraft {
                minutes: 10.25,
                notes: None,
            }),
            vec!["duration=10.25", "notes="],
            vec!["duration", "details.notes", "lastUpdated"],
        ),
    ];
    for (draft, changes, expected) in examples {
        let changes = changes.into_iter().map(str::to_owned).collect::<Vec<_>>();
        let updates = save::build_updates(&draft, &changes, None, &[], 123.0).unwrap();
        assert_eq!(
            updates
                .iter()
                .map(|update| update.path.join("."))
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn keeping_a_bottle_amount_never_writes_a_rounded_projection() {
    use app::{
        cli::{BottleKind, Units},
        edit::BottleDraft,
    };
    let draft = Draft::Bottle(BottleDraft {
        amount: 3.381,
        kind: BottleKind::Formula,
        units: Units::Oz,
        notes: Some("new".into()),
    });
    let updates = save::build_updates(&draft, &["notes=new".into()], None, &[], 123.0).unwrap();
    assert!(
        !updates
            .iter()
            .any(|update| matches!(update.path[0].as_str(), "amount" | "units" | "bottleType"))
    );
}
