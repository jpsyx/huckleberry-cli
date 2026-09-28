//! Solid food: the curated database, a family's own foods, and what was eaten.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::common::{Number, TextOrNumber};

string_enum! {
    /// Where a food came from.
    SolidsFoodSource {
        /// One of the family's own foods, from `types/{cid}/custom`.
        Custom => "custom",
        /// One from the curated database in Firebase Storage.
        Curated => "curated",
    }
}

string_enum! {
    /// How the baby took it.
    SolidsReaction {
        Loved => "LOVED",
        Meh => "MEH",
        Hated => "HATED",
        Allergic => "ALLERGIC",
    }
}

/// One food inside a solids entry, as the row stores it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolidsFoodEntry {
    /// The food's identifier, which is also its key in the enclosing map.
    pub id: String,
    /// The food's name as it was when the entry was written. Kept on the row
    /// so renaming a custom food later does not rewrite history.
    pub created_name: String,
    /// Which database it came from.
    pub source: SolidsFoodSource,
    /// How much, in whatever the app let the parent say.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub amount: Option<TextOrNumber>,
}

/// A food to log, as a caller names one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FoodReference {
    /// The food's identifier.
    pub id: String,
    /// Which database it came from.
    pub source: SolidsFoodSource,
    /// The food's name.
    pub name: String,
    /// How much.
    pub amount: TextOrNumber,
}

impl FoodReference {
    /// A whole serving of a curated food.
    #[must_use]
    pub fn curated(id: &str, name: &str, amount: &str) -> Self {
        Self {
            id: id.to_owned(),
            source: SolidsFoodSource::Curated,
            name: name.to_owned(),
            amount: TextOrNumber::Text(amount.to_owned()),
        }
    }

    /// A serving of one of the family's own foods.
    #[must_use]
    pub fn custom(id: &str, name: &str, amount: &str) -> Self {
        Self {
            id: id.to_owned(),
            source: SolidsFoodSource::Custom,
            name: name.to_owned(),
            amount: TextOrNumber::Text(amount.to_owned()),
        }
    }
}

/// `types/{cid}/custom/{food_id}`: a food the family added.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomFood {
    /// When it was added, as an ISO 8601 instant.
    pub created_at: String,
    /// When it last changed.
    pub updated_at: String,
    /// What it is called.
    pub name: String,
    /// Whether it has been hidden. Archived foods are still referenced by
    /// entries that used them, which is why they are hidden and not deleted.
    pub archived: bool,
    /// The identifier, which matches the document id.
    pub id: String,
    /// Always `solids`.
    #[serde(rename = "type")]
    pub kind: String,
    /// A Firebase Storage filename, or an empty string.
    #[serde(default)]
    pub image: String,
    /// Always `custom`.
    pub source: SolidsFoodSource,
}

/// One entry in the curated food database.
///
/// This does not come from Firestore: it is a single JSON object in Firebase
/// Storage, keyed by food id, which the app downloads whole.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CuratedFood {
    /// The identifier.
    pub id: String,
    /// What it is called.
    pub name: String,
    /// Always `curated`.
    pub source: SolidsFoodSource,
    /// Other names it goes by.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub aka: Option<Vec<String>>,
    /// Whether it is one of the common allergens.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub is_common_allergen: Option<bool>,
    /// Whether it is a choking risk.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub is_high_choking_hazard: Option<bool>,
    /// The age in months the app suggests starting at.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub recommended_age_to_start: Option<Number>,
    /// Which food groups it belongs to.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub category: Option<BTreeMap<String, bool>>,
    /// The key the app uses to link to more about it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub link_key: Option<String>,
    /// Where it sorts in the app's own list. Lower is earlier.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub rank: Option<Number>,
    /// A Firebase Storage filename.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub image: Option<String>,
}

/// Sorts curated foods the way the app lists them: by rank, then by name.
///
/// A food with no rank sorts after every ranked one rather than before, which
/// is what "unranked" has to mean in a list the app presents as a shortlist.
pub fn sort_curated(foods: &mut [CuratedFood]) {
    foods.sort_by(|left, right| {
        let rank = |food: &CuratedFood| food.rank.map_or(f64::INFINITY, Number::as_f64);
        rank(left)
            .partial_cmp(&rank(right))
            .unwrap_or(core::cmp::Ordering::Equal)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
}

/// Which trackers `types/{cid}` says are available.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AvailableTypes {
    /// Whether solids is switched on.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub solids: Option<bool>,
}

/// `types/{cid}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TypesDocument {
    /// The trackers this child has.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub available_types: Option<AvailableTypes>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn food(name: &str, rank: Option<f64>) -> CuratedFood {
        CuratedFood {
            id: name.to_lowercase(),
            name: name.to_owned(),
            source: SolidsFoodSource::Curated,
            aka: None,
            is_common_allergen: None,
            is_high_choking_hazard: None,
            recommended_age_to_start: None,
            category: None,
            link_key: None,
            rank: rank.map(Number::Float),
            image: None,
        }
    }

    #[test]
    fn curated_foods_sort_by_rank_then_by_name() {
        let mut foods = vec![food("Banana", Some(2.0)), food("Avocado", Some(1.0))];
        sort_curated(&mut foods);
        assert_eq!(foods[0].name, "Avocado");
    }

    #[test]
    fn an_unranked_food_sorts_after_every_ranked_one() {
        let mut foods = vec![food("Aubergine", None), food("Banana", Some(9.0))];
        sort_curated(&mut foods);
        assert_eq!(foods[0].name, "Banana");
    }

    #[test]
    fn foods_of_equal_rank_sort_by_name_ignoring_case() {
        let mut foods = vec![food("banana", Some(1.0)), food("Apple", Some(1.0))];
        sort_curated(&mut foods);
        assert_eq!(foods[0].name, "Apple");
    }

    #[test]
    fn an_amount_may_be_a_word_or_a_number() {
        let entry: SolidsFoodEntry = serde_json::from_value(json!({
            "id": "avocado",
            "created_name": "Avocado",
            "source": "curated",
            "amount": "a few bites",
        }))
        .expect("an entry");
        assert_eq!(
            entry.amount.as_ref().map(TextOrNumber::to_text),
            Some("a few bites".to_owned())
        );
    }
}
