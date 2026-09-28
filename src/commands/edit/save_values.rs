//! Field paths mirror the existing typed API edits, restricted to explicit changes.
use crate::edit::{DiaperDraft, Draft};
use anyhow::{Result, bail};
use huckleberry_api::firestore::FieldUpdate;
use serde_json::{Value, json};

pub(super) fn updates(draft: &Draft, key: &str) -> Result<Vec<FieldUpdate>> {
    let update = match draft {
        Draft::Diaper(diaper) => diaper_value(diaper, key)?,
        Draft::Bottle(bottle) => match key {
            "amount" | "units" => {
                return Ok(vec![
                    FieldUpdate::set("amount", json!(bottle.amount)),
                    FieldUpdate::set("units", json!(bottle.units.to_api().as_str())),
                ]);
            }
            "type" => FieldUpdate::set("bottleType", json!(bottle.kind.to_api().as_str())),
            "notes" => note("notes", bottle.notes.as_deref()),
            _ => bail!("unknown bottle field {key}"),
        },
        Draft::Nursing(nursing) => match key {
            "left" => FieldUpdate::set("leftDuration", json!(nursing.left_minutes * 60.0)),
            "right" => FieldUpdate::set("rightDuration", json!(nursing.right_minutes * 60.0)),
            "notes" => note("notes", nursing.notes.as_deref()),
            _ => bail!("unknown nursing field {key}"),
        },
        Draft::Solids(meal) => match key {
            "reaction" => FieldUpdate::set_or_clear(
                "reactions",
                meal.reaction
                    .map(|reaction| json!({reaction.to_api().as_str(): true})),
            ),
            "notes" => note("notes", meal.notes.as_deref()),
            _ => bail!("unknown meal field {key}"),
        },
        Draft::Sleep(sleep) => match key {
            "duration" => FieldUpdate::set("duration", json!(sleep.minutes * 60.0)),
            "notes" => note("details.notes", sleep.notes.as_deref()),
            _ => bail!("unknown sleep field {key}"),
        },
    };
    Ok(vec![update])
}

fn diaper_value(diaper: &DiaperDraft, key: &str) -> Result<FieldUpdate> {
    Ok(match key {
        "mode" => FieldUpdate::set("mode", json!(diaper.mode.to_api().as_str())),
        "pee" => FieldUpdate::set_or_clear(
            "quantity.pee",
            diaper
                .pee
                .and_then(|amount| amount.to_api().to_stored())
                .map(|amount| json!(amount)),
        ),
        "poo" => FieldUpdate::set_or_clear(
            "quantity.poo",
            diaper
                .poo
                .and_then(|amount| amount.to_api().to_stored())
                .map(|amount| json!(amount)),
        ),
        "color" => FieldUpdate::set_or_clear(
            "color",
            diaper.color.map(|value| json!(value.to_api().as_str())),
        ),
        "consistency" => FieldUpdate::set_or_clear(
            "consistency",
            diaper
                .consistency
                .map(|value| json!(value.to_api().as_str())),
        ),
        "rash" => FieldUpdate::set_or_clear("diaperRash", diaper.rash.then_some(Value::Bool(true))),
        "how" => FieldUpdate::set_or_clear(
            "howItHappened",
            diaper.how.map(|value| json!(value.to_api().as_str())),
        ),
        "notes" => note("notes", diaper.notes.as_deref()),
        _ => bail!("unknown diaper field {key}"),
    })
}

fn note(path: &str, value: Option<&str>) -> FieldUpdate {
    FieldUpdate::set_or_clear(
        path,
        value
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(|text| json!(text)),
    )
}
