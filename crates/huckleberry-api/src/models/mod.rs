//! The Huckleberry documents, as Rust types.
//!
//! One module per Firestore collection, named for the collection. Every type
//! is a plain serde struct: an absent field is `None`, a field this crate does
//! not model is ignored rather than fatal, and nothing serializes a `None`, so
//! a write carries exactly the keys it means to set. That last rule is what
//! the Python client spells `model_dump(by_alias=True, exclude_none=True)`.
//!
//! Names drop the `Firebase` prefix the Python models carry: the crate name is
//! the namespace, so `FirebaseSleepIntervalData` is [`sleep::SleepInterval`].
//! `docs/api.md` has the full mapping.

pub mod activity;
pub mod child;
pub mod common;
pub mod diaper;
pub mod feed;
pub mod health;
pub mod milestone;
pub mod pump;
pub mod sleep;
pub mod solids;
pub mod user;

pub use activity::{ActivityInterval, ActivityMode};
pub use child::{ChildDocument, Gender};
pub use common::{MultiContainer, Number, ReminderV2, TextOrNumber, Timestamp};
pub use diaper::{
    DiaperAmount, DiaperDocument, DiaperEntry, DiaperMode, DiaperQuantity, PooColor,
    PooConsistency, PottyResult,
};
pub use feed::{
    BottleFeedInterval, BottleType, BreastFeedInterval, FeedDocument, FeedInterval, FeedSide,
    FeedTimer, SolidsFeedInterval, VolumeUnits,
};
pub use health::{
    GrowthEntry, HealthDocument, HealthEntry, MeasurementSystem, MedicationEntry, MedicationUnits,
    TemperatureEntry, TemperatureUnits,
};
pub use milestone::Milestone;
pub use pump::{PumpEntryMode, PumpInterval};
pub use sleep::{
    SleepCondition, SleepDetails, SleepDocument, SleepInterval, SleepLocations, SleepTimer,
};
pub use solids::{CuratedFood, CustomFood, FoodReference, SolidsFoodEntry, SolidsReaction};
pub use user::{UserChildRef, UserDocument};

use serde::Serialize;
use serde_json::{Map, Value as Json};

use crate::error::{Error, Result};

/// Turns a model into the Firestore document body that writes it.
///
/// This is the Python client's `to_firebase_dict`: serialize, drop nothing
/// that was set, drop everything that was not, and tag the result.
///
/// # Errors
///
/// [`Error::Decode`] when the model does not serialize to a JSON object,
/// which would mean something other than a document had been passed.
pub fn to_fields<T: Serialize>(model: &T) -> Result<Map<String, Json>> {
    let value = serde_json::to_value(model)
        .map_err(|error| Error::decode("preparing a document", error))?;
    let Json::Object(object) = value else {
        return Err(Error::decode(
            "preparing a document",
            "a document body has to be an object",
        ));
    };
    Ok(crate::firestore::value::fields_from_json(&object))
}

/// Serializes a model to plain JSON, dropping everything that was not set.
///
/// This is what a field update writes: the value at one path, before the
/// Firestore tagging that [`crate::firestore::value`] adds.
///
/// # Errors
///
/// [`Error::Decode`] when the model does not serialize, which in practice
/// means a map with a non-string key.
pub fn to_json<T: Serialize>(model: &T) -> Result<Json> {
    serde_json::to_value(model).map_err(|error| Error::decode("preparing a field", error))
}

/// Reads a model out of a decoded document.
///
/// # Errors
///
/// [`Error::Decode`] when the document does not match the model.
pub fn from_json<T: serde::de::DeserializeOwned>(json: Json, operation: &str) -> Result<T> {
    serde_json::from_value(json).map_err(|error| Error::decode(operation, error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_model_writes_only_the_fields_that_were_set() {
        let entry = diaper::DiaperEntry {
            mode: diaper::DiaperMode::Pee,
            start: Number::Float(1.0),
            last_updated: None,
            offset: Number::Float(-240.0),
            quantity: None,
            color: None,
            consistency: None,
            diaper_rash: None,
            notes: None,
            is_potty: None,
            how_it_happened: None,
        };
        let fields = to_fields(&entry).expect("a document body");
        assert_eq!(fields.len(), 3, "mode, start and offset, and nothing else");
        assert_eq!(fields["mode"], json!({ "stringValue": "pee" }));
        assert_eq!(fields["offset"], json!({ "doubleValue": -240.0 }));
    }

    #[test]
    fn a_decoded_document_reads_back_as_a_model() {
        let profile: child::ChildDocument =
            from_json(json!({ "childsName": "Bear" }), "reading a child").expect("a profile");
        assert_eq!(profile.childs_name.as_deref(), Some("Bear"));
    }
}
