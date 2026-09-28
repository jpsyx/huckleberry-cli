//! Sparse history edits: Keep never serializes a projected default over stored data.
use crate::edit::Draft;
use anyhow::{Context as _, Result};
use huckleberry_api::{
    Huckleberry, RowRef, client::now_seconds, firestore::FieldUpdate, models::solids::CustomFood,
};
use serde_json::{Value, json};

/// Writes only the fields deliberately selected by the parent or script.
pub(super) async fn run(
    client: &Huckleberry,
    cid: &str,
    at: &RowRef,
    draft: &Draft,
    changes: &[String],
) -> Result<()> {
    let changes_foods = matches!(draft, Draft::Solids(_)) && changed(changes, &["foods", "amount"]);
    let (foods, known) = if changes_foods {
        let row = client
            .located_rows(&at.tracker, cid, "preserving the meal's foods")
            .await?
            .into_iter()
            .find(|(found, _)| found == at)
            .context("the meal no longer exists")?
            .1;
        let known = if changed(changes, &["foods"]) {
            client.custom_foods(cid, false).await?
        } else {
            vec![]
        };
        (row.get("foods").cloned(), known)
    } else {
        (None, vec![])
    };
    let updates = build_updates(draft, changes, foods.as_ref(), &known, now_seconds())?;
    if !updates.is_empty() {
        client
            .update_history_row(cid, at, &updates, "saving changed entry fields")
            .await?;
    }
    Ok(())
}

/// Converts validated explicit changes into the existing API's field updates.
/// Omitted fields, including unknown enum values and unmodeled metadata, are untouched.
pub fn build_updates(
    draft: &Draft,
    changes: &[String],
    foods: Option<&Value>,
    known: &[CustomFood],
    now: f64,
) -> Result<Vec<FieldUpdate>> {
    let mut updates = Vec::new();
    for change in changes {
        let (key, _) = change
            .split_once('=')
            .context("an edit must be key=value")?;
        let key = key.trim().to_lowercase();
        if matches!(key.as_str(), "at" | "start") {
            continue;
        }
        let added = if let Draft::Solids(meal) = draft
            && matches!(key.as_str(), "foods" | "amount")
        {
            vec![FieldUpdate::set(
                "foods",
                super::save_foods::updated(meal, changes, foods, known)?,
            )]
        } else {
            super::save_values::updates(draft, &key)?
        };
        for update in added {
            updates.retain(|existing: &FieldUpdate| existing.path != update.path);
            updates.push(update);
        }
    }
    if !updates.is_empty() {
        updates.push(FieldUpdate::set("lastUpdated", json!(now)));
    }
    Ok(updates)
}

pub(super) fn changed(changes: &[String], fields: &[&str]) -> bool {
    changes
        .iter()
        .filter_map(|change| change.split_once('='))
        .any(|(key, _)| fields.contains(&key.trim().to_lowercase().as_str()))
}
