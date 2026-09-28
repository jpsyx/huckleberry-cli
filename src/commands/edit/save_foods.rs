//! Preserve original food identities, sources and per-food metadata during meal edits.
use crate::edit::SolidsDraft;
use anyhow::{Context as _, Result};
use huckleberry_api::models::solids::CustomFood;
use serde_json::{Map, Value, json};

pub(super) fn updated(
    meal: &SolidsDraft,
    changes: &[String],
    original: Option<&Value>,
    known: &[CustomFood],
) -> Result<Value> {
    let original = original
        .and_then(Value::as_object)
        .context("cannot edit foods: the stored food map is unavailable")?;
    let mut foods = if super::save::changed(changes, &["foods"]) {
        selected(meal, original, known)?
    } else {
        original.clone()
    };
    if super::save::changed(changes, &["amount"]) {
        for food in foods.values_mut() {
            food.as_object_mut()
                .context("cannot change amount on an unfamiliar food record")?
                .insert("amount".into(), json!(meal.amount));
        }
    }
    Ok(Value::Object(foods))
}

fn selected(
    meal: &SolidsDraft,
    original: &Map<String, Value>,
    known: &[CustomFood],
) -> Result<Map<String, Value>> {
    let mut foods = Map::new();
    for name in &meal.foods {
        let matching = original
            .iter()
            .filter(|(_, food)| {
                food.get("created_name")
                    .and_then(Value::as_str)
                    .is_some_and(|stored| stored.eq_ignore_ascii_case(name))
            })
            .collect::<Vec<_>>();
        if matching.is_empty() {
            let reference = crate::commands::feed::match_food(name, known, &meal.amount);
            let added = huckleberry_api::ops::solids::foods_map(&[reference])?;
            foods.extend(
                serde_json::to_value(added)?
                    .as_object()
                    .context("food map serialization")?
                    .clone(),
            );
        } else {
            foods.extend(
                matching
                    .into_iter()
                    .map(|(id, food)| (id.clone(), food.clone())),
            );
        }
    }
    Ok(foods)
}
