//! Pump amounts move together because both sides share one unit field.
use anyhow::{Result, bail};
use huckleberry_api::firestore::FieldUpdate;
use serde_json::json;

use crate::edit::PumpDraft;

pub(super) fn updates(pump: &PumpDraft, key: &str) -> Result<Vec<FieldUpdate>> {
    match key {
        "amount" | "left" | "right" | "mode" | "units" => {
            let mut updates = Vec::new();
            if key != "units" {
                updates.push(FieldUpdate::set("entryMode", json!(pump.mode.as_str())));
            }
            updates.extend([
                FieldUpdate::set_or_clear("leftAmount", pump.left_amount.map(|value| json!(value))),
                FieldUpdate::set_or_clear(
                    "rightAmount",
                    pump.right_amount.map(|value| json!(value)),
                ),
                FieldUpdate::set("units", json!(pump.units.as_str())),
            ]);
            Ok(updates)
        }
        "duration" => Ok(vec![FieldUpdate::set_or_clear(
            "duration",
            pump.duration_minutes.map(|minutes| json!(minutes * 60.0)),
        )]),
        "notes" => Ok(vec![FieldUpdate::set_or_clear(
            "notes",
            pump.notes.as_ref().map(|value| json!(value)),
        )]),
        _ => bail!("unknown pump field {key}"),
    }
}
