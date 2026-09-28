//! Solid food: the two food databases, and logging a meal.
//!
//! The curated database is the odd one out in this whole crate: it is not
//! Firestore at all but a single JSON object in Firebase Storage, keyed by
//! food id, which the app downloads whole and caches.

use std::collections::BTreeMap;

use serde_json::{Value as Json, json};

use crate::client::{Huckleberry, now_seconds};
use crate::constants::{CURATED_FOODS_BUCKET, CURATED_FOODS_OBJECT};
use crate::error::{Error, Result, describe_failure};
use crate::firestore::client::encode_segment;
use crate::firestore::{FieldUpdate, Op, Query};
use crate::ids;
use crate::models::common::Number;
use crate::models::feed::{FeedInterval, LastSolid, SolidsFeedInterval};
use crate::models::solids::{
    CuratedFood, CustomFood, FoodReference, SolidsFoodEntry, SolidsFoodSource, SolidsReaction,
    sort_curated,
};
use crate::models::{to_fields, to_json};
use crate::ops::feed::bottle::note;
use crate::paths;
use crate::rows::RowRef;

/// Turns the foods a caller named into the map a solids row stores.
///
/// # Errors
///
/// [`Error::Invalid`] when the list is empty or a food has a blank name:
/// a meal of nothing, and a food with no name, are both rows that would be
/// unreadable in the app afterwards.
pub fn foods_map(foods: &[FoodReference]) -> Result<BTreeMap<String, SolidsFoodEntry>> {
    if foods.is_empty() {
        return Err(Error::Invalid(
            "a solids entry needs at least one food".to_owned(),
        ));
    }
    let mut entries = BTreeMap::new();
    for food in foods {
        let name = food.name.trim();
        if name.is_empty() {
            return Err(Error::Invalid("a food needs a name".to_owned()));
        }
        entries.insert(
            food.id.clone(),
            SolidsFoodEntry {
                id: food.id.clone(),
                created_name: name.to_owned(),
                source: food.source.clone(),
                amount: Some(food.amount.clone()),
            },
        );
    }
    Ok(entries)
}

/// The URL the curated food database is served from.
#[must_use]
pub fn curated_foods_url() -> String {
    format!(
        "https://firebasestorage.googleapis.com/v0/b/{CURATED_FOODS_BUCKET}/o/{}?alt=media",
        encode_segment(CURATED_FOODS_OBJECT)
    )
}

/// Reads the curated database's payload: one object keyed by food id.
///
/// Entries that are not objects, or that this crate cannot read, are skipped
/// rather than failing the download: the database is Huckleberry's and grows
/// without asking.
#[must_use]
pub fn parse_curated_foods(payload: &Json) -> Vec<CuratedFood> {
    let mut foods: Vec<CuratedFood> = payload
        .as_object()
        .map(|entries| {
            entries
                .values()
                .filter(|food| food.is_object())
                .filter_map(|food| serde_json::from_value(food.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    sort_curated(&mut foods);
    foods
}

impl Huckleberry {
    /// The curated food database, in the order the app lists it.
    ///
    /// # Errors
    ///
    /// [`Error::Api`] when Storage refuses the download, [`Error::Network`]
    /// when it cannot be reached.
    pub async fn curated_foods(&self) -> Result<Vec<CuratedFood>> {
        let operation = "downloading the curated foods";
        let token = self.token().await?;
        let response = self
            .http()
            .get(curated_foods_url())
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|source| Error::network(operation, source))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|source| Error::network(operation, source))?;
        if !status.is_success() {
            return Err(Error::api(operation, status.as_u16(), &body));
        }
        let payload: Json = serde_json::from_str(&body).map_err(|error| {
            Error::decode(operation, format!("{error}: {}", describe_failure(&body)))
        })?;
        Ok(parse_curated_foods(&payload))
    }

    /// The family's own foods, newest first.
    ///
    /// Archived foods are left out unless asked for: they still exist because
    /// past entries reference them, not because anybody wants to log them.
    ///
    /// # Errors
    ///
    /// As every read: a refused or unreachable Firestore.
    pub async fn custom_foods(&self, cid: &str, include_archived: bool) -> Result<Vec<CustomFood>> {
        let operation = "reading the family's foods";
        let token = self.token().await?;
        let query = Query::on("custom").filter("type", Op::Equal, json!("solids"));
        let mut foods: Vec<CustomFood> = self
            .firestore()
            .query(&token, &paths::types(cid), &query, operation)
            .await?
            .into_iter()
            .filter_map(|document| serde_json::from_value(document.into_json()).ok())
            .filter(|food: &CustomFood| include_archived || !food.archived)
            .collect();
        foods.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(foods)
    }

    /// Adds a food to the family's own list.
    ///
    /// # Errors
    ///
    /// [`Error::Invalid`] when the name is blank, and otherwise as every
    /// write.
    pub async fn create_custom_food(
        &self,
        cid: &str,
        name: &str,
        image: &str,
    ) -> Result<CustomFood> {
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::Invalid("a food needs a name".to_owned()));
        }
        let stamp = iso_8601(now_seconds());
        let food = CustomFood {
            created_at: stamp.clone(),
            updated_at: stamp,
            name: name.to_owned(),
            archived: false,
            id: ids::custom_food_id(),
            kind: "solids".to_owned(),
            image: image.to_owned(),
            source: SolidsFoodSource::Custom,
        };

        let token = self.token().await?;
        let operation = "adding a food";
        // Switching solids on for the child, which the app does when the first
        // custom food is added.
        let mut available = serde_json::Map::new();
        available.insert("available_types".to_owned(), json!({ "solids": true }));
        self.firestore()
            .merge(
                &token,
                &paths::types(cid),
                &crate::firestore::value::fields_from_json(&available),
                operation,
            )
            .await?;
        self.firestore()
            .set(
                &token,
                &paths::custom_food(cid, &food.id),
                &to_fields(&food)?,
                operation,
            )
            .await?;
        Ok(food)
    }

    /// Records a meal.
    ///
    /// # Errors
    ///
    /// [`Error::Invalid`] when no food was named or a food has a blank name,
    /// and otherwise as every write.
    pub async fn log_solids(
        &self,
        cid: &str,
        foods: &[FoodReference],
        notes: Option<&str>,
        reaction: Option<SolidsReaction>,
        photo: Option<&str>,
    ) -> Result<()> {
        let eaten = foods_map(foods)?;
        let now = now_seconds();
        let offset = self.zone().offset_minutes(now);
        let reactions = reaction
            .as_ref()
            .map(|taken| BTreeMap::from([(taken.as_str().to_owned(), true)]));
        let notes = notes.map(str::trim).filter(|text| !text.is_empty());

        let entry = FeedInterval::Solids(SolidsFeedInterval {
            start: Number::Float(now),
            last_updated: Some(Number::Float(now)),
            offset: Number::Float(offset),
            foods: Some(eaten.clone()),
            reactions: reactions.clone(),
            notes: notes.map(ToOwned::to_owned),
            food_note_image: photo.map(ToOwned::to_owned),
            multientry_key: None,
            end_offset: None,
        });

        let token = self.token().await?;
        let operation = "recording the meal";
        self.firestore()
            .set(
                &token,
                &paths::history_row(paths::FEED, cid, &ids::interval_id(now)),
                &to_fields(&entry)?,
                operation,
            )
            .await?;

        let last_solid = LastSolid {
            mode: Some("solids".to_owned()),
            start: Some(Number::Float(now)),
            foods: Some(eaten),
            reactions,
            notes: notes.map(ToOwned::to_owned),
            offset: Some(Number::Float(offset)),
        };
        self.firestore()
            .update(
                &token,
                &paths::tracker(paths::FEED, cid),
                &[
                    FieldUpdate::set("prefs.lastSolid", to_json(&last_solid)?),
                    FieldUpdate::set("prefs.timestamp", json!({ "seconds": now })),
                    FieldUpdate::set("prefs.local_timestamp", json!(now)),
                ],
                operation,
            )
            .await
    }

    /// Changes a meal that is already on the record.
    ///
    /// What was eaten, how it went, and what was noted. Not when: the row
    /// keeps the moment it happened.
    ///
    /// # Errors
    ///
    /// [`Error::Invalid`] when the meal is left with no foods in it, and
    /// otherwise as [`Huckleberry::update_history_row`].
    pub async fn update_solids_entry(
        &self,
        cid: &str,
        at: &RowRef,
        foods: &[FoodReference],
        reaction: Option<SolidsReaction>,
        notes: Option<&str>,
    ) -> Result<()> {
        let eaten = foods_map(foods)?;
        let reactions = reaction
            .as_ref()
            .map(|taken| json!({ taken.as_str(): true }));
        let updates = [
            FieldUpdate::set("foods", to_json(&eaten)?),
            FieldUpdate::set_or_clear("reactions", reactions),
            FieldUpdate::set_or_clear("notes", note(notes)),
            FieldUpdate::set("lastUpdated", json!(now_seconds())),
        ];
        self.update_history_row(cid, at, &updates, "changing the meal")
            .await
    }
}

/// An instant as the app writes it into a custom food: UTC, milliseconds, `Z`.
#[must_use]
pub fn iso_8601(at: f64) -> String {
    jiff::Timestamp::from_second(at as i64)
        .unwrap_or(jiff::Timestamp::UNIX_EPOCH)
        .strftime("%Y-%m-%dT%H:%M:%S.000Z")
        .to_string()
}

#[cfg(test)]
mod food_lists {
    use super::*;
    use crate::models::common::TextOrNumber;

    fn avocado() -> FoodReference {
        FoodReference::curated("avocado", "Avocado", "a few bites")
    }

    #[test]
    fn a_food_becomes_an_entry_keyed_by_its_id() {
        let map = foods_map(&[avocado()]).expect("a food map");
        let entry = map.get("avocado").expect("the avocado");
        assert_eq!(entry.created_name, "Avocado");
        assert_eq!(entry.source, SolidsFoodSource::Curated);
    }

    #[test]
    fn a_meal_of_nothing_is_refused() {
        assert!(matches!(foods_map(&[]), Err(Error::Invalid(_))));
    }

    #[test]
    fn a_food_with_a_blank_name_is_refused() {
        let nameless = FoodReference {
            id: "x".to_owned(),
            source: SolidsFoodSource::Custom,
            name: "   ".to_owned(),
            amount: TextOrNumber::Text("some".to_owned()),
        };
        assert!(matches!(foods_map(&[nameless]), Err(Error::Invalid(_))));
    }

    #[test]
    fn a_name_is_trimmed_before_it_is_stored() {
        let padded = FoodReference::curated("pear", "  Pear  ", "half");
        let map = foods_map(&[padded]).expect("a food map");
        assert_eq!(map["pear"].created_name, "Pear");
    }
}

#[cfg(test)]
mod curated {
    use super::*;

    #[test]
    fn the_payload_is_an_object_keyed_by_food_id() {
        let payload = json!({
            "banana": { "id": "banana", "name": "Banana", "source": "curated", "rank": 2 },
            "avocado": { "id": "avocado", "name": "Avocado", "source": "curated", "rank": 1 },
        });
        let foods = parse_curated_foods(&payload);
        assert_eq!(foods.len(), 2);
        assert_eq!(foods[0].name, "Avocado", "sorted by rank");
    }

    #[test]
    fn an_entry_this_crate_cannot_read_is_skipped_and_the_rest_survive() {
        let payload = json!({
            "banana": { "id": "banana", "name": "Banana", "source": "curated" },
            "broken": { "name": "No id here" },
            "also_broken": 7,
        });
        assert_eq!(parse_curated_foods(&payload).len(), 1);
    }

    #[test]
    fn a_payload_that_is_not_an_object_yields_nothing_rather_than_failing() {
        assert!(parse_curated_foods(&json!([1, 2, 3])).is_empty());
    }

    #[test]
    fn the_object_path_is_encoded_into_the_url() {
        assert!(curated_foods_url().contains("foods%2Ffooddb.json"));
    }
}

#[cfg(test)]
mod timestamps {
    use super::*;

    #[test]
    fn an_instant_is_written_the_way_the_app_writes_it() {
        assert_eq!(iso_8601(1_758_572_400.0), "2025-09-22T20:20:00.000Z");
    }

    #[test]
    fn the_instant_is_utc_whatever_the_machine_is_set_to() {
        assert!(iso_8601(0.0).starts_with("1970-01-01T00:00:00"));
    }
}
