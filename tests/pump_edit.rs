//! Pump details use the same sparse edits and located history rows as other entries.
use app::{
    cli::Units,
    commands::edit::save,
    domain::{Calendar, log, normalize, types::Dataset},
    edit::{self, Draft},
    interactive::options::fields,
    render,
};
use huckleberry_api::{Located, RowRef};
use serde_json::{Value, json};

fn dataset(mode: &str, left: Option<f64>, right: Option<f64>) -> Dataset {
    let mut dataset: Dataset = serde_json::from_value(json!({
        "fetched_at": 2000.0, "timezone": "UTC", "days": 1,
        "child": {"cid": "test", "name": "Test", "birthdate": null,
                  "night_start_hour": 20.0, "morning_cutoff_hour": 7.0},
        "growth": null, "sleep": [], "feeds": [], "diapers": [],
        "pumps": [], "milestones": [], "live": {}, "notes": []
    }))
    .unwrap();
    dataset.pumps = normalize::pumps(&[Located::new(
        RowRef::batched("pump", "batch", "session"),
        serde_json::from_value(json!({
            "start": 1000.0, "entryMode": mode, "units": "ml", "offset": 0,
            "leftAmount": left, "rightAmount": right, "duration": 900,
            "notes": "original", "extra": {"keep": true}
        }))
        .unwrap(),
    )]);
    dataset
}

fn draft(mode: &str, left: Option<f64>, right: Option<f64>, units: Units) -> Draft {
    edit::draft_for(
        &dataset(mode, left, right),
        &RowRef::batched("pump", "batch", "session"),
        units,
    )
    .expect("a pump has a detail editor")
}

fn updates(draft: &mut Draft, pairs: &[&str]) -> Value {
    let changes: Vec<String> = pairs.iter().map(|pair| (*pair).into()).collect();
    draft.apply(&changes).unwrap();
    let mut result = json!({});
    for update in save::build_updates(draft, &changes, None, &[], 2000.0).unwrap() {
        result[update.path.join(".")] = update.value.unwrap_or(Value::Null);
    }
    result
}

#[test]
fn pump_edit_keeps_combined_and_separate_amounts_distinct() {
    let total = draft("total", Some(50.0), Some(50.0), Units::Ml);
    let separate = draft("leftright", Some(30.0), Some(70.0), Units::Ml);
    assert_eq!(fields::current(&total, "mode"), "Total");
    assert_eq!(fields::current(&total, "amount"), "100 ml");
    assert_eq!(fields::current(&separate, "left"), "30 ml");
    assert_eq!(fields::current(&separate, "right"), "70 ml");
    assert_eq!(fields::current(&total, "duration"), "15m");
}

#[test]
fn pump_total_edit_splits_evenly_and_switches_mode() {
    let result = updates(
        &mut draft("leftright", Some(30.0), Some(70.0), Units::Ml),
        &["amount=80"],
    );
    assert_eq!(
        result,
        json!({"entryMode":"total", "leftAmount":40.0,
        "rightAmount":40.0, "units":"ml", "lastUpdated":2000.0})
    );
}

#[test]
fn pump_side_edit_preserves_the_other_quantity_in_the_display_units() {
    let result = updates(
        &mut draft(
            "total",
            Some(29.573_529_562_5),
            Some(29.573_529_562_5),
            Units::Oz,
        ),
        &["left=2"],
    );
    assert_eq!(
        result,
        json!({"entryMode":"leftright", "leftAmount":2.0,
        "rightAmount":1.0, "units":"oz", "lastUpdated":2000.0})
    );
}

#[test]
fn pump_unit_change_converts_both_sides_instead_of_relabelling_them() {
    let result = updates(
        &mut draft(
            "leftright",
            Some(29.573_529_562_5),
            Some(59.147_059_125),
            Units::Ml,
        ),
        &["units=oz"],
    );
    assert_eq!(result["leftAmount"], 1.0);
    assert_eq!(result["rightAmount"], 2.0);
    assert_eq!(result["units"], "oz");
}

#[test]
fn pump_notes_edit_writes_only_the_note_and_sync_time() {
    let result = updates(
        &mut draft("future-mode", Some(30.12345), None, Units::Oz),
        &["notes=new note"],
    );
    assert_eq!(result, json!({"notes":"new note", "lastUpdated":2000.0}));
}

#[test]
fn pump_optional_values_clear_and_duration_is_stored_in_seconds() {
    let mut pump = draft("leftright", Some(30.0), Some(70.0), Units::Ml);
    assert_eq!(
        updates(&mut pump, &["duration=12.5"]),
        json!({"duration":750.0,"lastUpdated":2000.0})
    );
    assert_eq!(
        updates(&mut pump, &["duration=", "notes="]),
        json!({"duration":null,"notes":null,"lastUpdated":2000.0})
    );
    let result = updates(&mut pump, &["amount="]);
    assert!(result["leftAmount"].is_null());
    assert!(result["rightAmount"].is_null());
}

#[test]
fn pump_rejects_invalid_amounts_durations_modes_and_units_before_save() {
    for (field, value) in [
        ("left", "-1"),
        ("right", "NaN"),
        ("amount", "inf"),
        ("duration", "-1"),
        ("duration", "NaN"),
        ("mode", "separate-ish"),
        ("units", "litres"),
    ] {
        assert!(
            draft("total", Some(50.0), Some(50.0), Units::Ml)
                .set(field, value)
                .is_err(),
            "{field}={value}"
        );
    }
}

#[test]
fn pump_without_recorded_amounts_stays_distinct_from_zero() {
    let empty = draft("total", None, None, Units::Ml);
    let zero = draft("total", Some(0.0), Some(0.0), Units::Ml);
    assert_eq!(fields::current(&empty, "amount"), "not set");
    assert_eq!(fields::current(&zero, "amount"), "0 ml");
}

#[test]
fn pump_log_and_delete_rows_share_the_canonical_location() {
    let data = dataset("total", Some(50.0), Some(50.0));
    let entries = log::build(&data, |amount| render::format::volume(amount, Units::Ml));
    let entry = &entries[0];
    assert_eq!(entry.kind, log::Kind::Pump);
    assert_eq!(entry.description, "100 ml over 15m");
    let rows = render::log::rows(&entries, &Calendar::new("UTC").unwrap(), 2000.0, &|_| None);
    assert_eq!(rows[0].key, "pump/batch#session");
    assert!(rows[0].selectable);
    assert_eq!(
        edit::parse_token(&rows[0].key).unwrap(),
        *entry.at.as_ref().unwrap()
    );
}

#[test]
fn pump_old_snapshots_default_to_separate_sides_without_losing_measurements() {
    let row: app::domain::types::PumpEvent = serde_json::from_value(json!({
        "id":"old", "start":1000.0, "left_ml":30.12345, "right_ml":null,
        "total_ml":30.12345, "duration_seconds":null, "notes":null
    }))
    .unwrap();
    assert_eq!(
        row.entry_mode,
        huckleberry_api::models::pump::PumpEntryMode::LeftRight
    );
    assert_eq!(row.left_ml, Some(30.12345));
    assert_eq!(row.right_ml, None);
    assert_eq!(row.total_ml, Some(30.12345));
}

#[test]
fn pump_explicit_amounts_use_requested_units_regardless_of_argument_order() {
    for pairs in [["amount=120", "units=ml"], ["units=ml", "amount=120"]] {
        let result = updates(
            &mut draft("total", Some(50.0), Some(50.0), Units::Oz),
            &pairs,
        );
        assert_eq!(result["leftAmount"], 60.0);
        assert_eq!(result["rightAmount"], 60.0);
        assert_eq!(result["units"], "ml");
    }
}

#[test]
fn pump_changing_to_total_retains_the_combined_quantity() {
    let result = updates(
        &mut draft("leftright", Some(20.0), Some(70.0), Units::Ml),
        &["mode=total"],
    );
    assert_eq!(result["entryMode"], "total");
    assert_eq!(result["leftAmount"], 45.0);
    assert_eq!(result["rightAmount"], 45.0);
}
