//! The Firestore value codec: the wire shape in, plain JSON out, and back.
//!
//! Firestore's REST API does not carry JSON. It carries tagged values, so the
//! number `5` is `{"integerValue": "5"}` and a nested object is
//! `{"mapValue": {"fields": {...}}}`. Every model in this crate is an ordinary
//! serde struct, and this module is the one place that knows the difference.
//! Converting through `serde_json::Value` rather than implementing a
//! `Serializer` keeps the tagging in forty lines that can be read in one go.
//!
//! Two details are load-bearing:
//!
//! - **An integer arrives as a string.** `integerValue` is quoted on the wire
//!   because JSON cannot hold an int64 exactly. Reading it as a string and
//!   handing back a number is what lets `start: 1758572400` deserialize.
//! - **`1.0` is a double, not an integer.** Huckleberry writes durations and
//!   offsets as doubles, and a value the app expects as `120.0` must not go
//!   back as `120`. `serde_json` keeps the distinction, so this module does
//!   too, and nothing here ever narrows a float that happens to be whole.
//!
//! The Huckleberry documents use none of Firestore's exotic types: timestamps
//! are stored as a plain `{seconds: n}` map rather than a `timestampValue`.
//! They are decoded anyway, as strings, so an unfamiliar document reads
//! cleanly instead of failing.

use serde_json::{Map, Value as Json};

/// Turns one Firestore REST value into plain JSON.
///
/// Anything unrecognized decodes to `null` rather than failing: a document
/// that grows a field this crate has never seen should still be readable.
#[must_use]
pub fn to_json(value: &Json) -> Json {
    let Some(tagged) = value.as_object() else {
        return Json::Null;
    };
    // A Firestore value is a one-key object, and the key is the type. Reading
    // the first entry rather than looking each tag up in turn keeps the match
    // below the only place the tag names appear.
    let Some((tag, inner)) = tagged.iter().next() else {
        return Json::Null;
    };
    match tag.as_str() {
        "integerValue" => integer_to_json(inner),
        "arrayValue" => array_to_json(inner),
        "mapValue" => map_to_json(inner),
        // Booleans, doubles and every text-shaped type (including a timestamp,
        // a reference and base64 bytes) are already the JSON this crate wants.
        // A geo point is a two-field object and stays one.
        "booleanValue" | "doubleValue" | "stringValue" | "timestampValue" | "bytesValue"
        | "referenceValue" | "geoPointValue" => inner.clone(),
        // `nullValue` is null, and so is a tag no version of this crate has
        // met: a document that grows an unfamiliar field should still be
        // readable.
        _ => Json::Null,
    }
}

/// Turns plain JSON into one Firestore REST value.
#[must_use]
pub fn from_json(json: &Json) -> Json {
    match json {
        Json::Null => tagged("nullValue", Json::Null),
        Json::Bool(flag) => tagged("booleanValue", Json::Bool(*flag)),
        Json::Number(number) => number_from_json(number),
        Json::String(text) => tagged("stringValue", Json::String(text.clone())),
        Json::Array(items) => {
            let values: Vec<Json> = items.iter().map(from_json).collect();
            tagged("arrayValue", serde_json::json!({ "values": values }))
        }
        Json::Object(entries) => tagged(
            "mapValue",
            serde_json::json!({ "fields": fields_from_json(entries) }),
        ),
    }
}

/// The `fields` object of a Firestore document or map value, from a plain
/// JSON object. This is the shape a document body wants, without the
/// surrounding `mapValue` wrapper.
#[must_use]
pub fn fields_from_json(entries: &Map<String, Json>) -> Map<String, Json> {
    entries
        .iter()
        .map(|(key, value)| (key.clone(), from_json(value)))
        .collect()
}

/// A plain JSON object from the `fields` of a Firestore document.
#[must_use]
pub fn fields_to_json(fields: &Map<String, Json>) -> Map<String, Json> {
    fields
        .iter()
        .map(|(key, value)| (key.clone(), to_json(value)))
        .collect()
}

/// Wraps a decoded value in its Firestore tag.
fn tagged(tag: &str, inner: Json) -> Json {
    let mut wrapper = Map::new();
    wrapper.insert(tag.to_owned(), inner);
    Json::Object(wrapper)
}

/// `integerValue` is quoted on the wire. An unparsable one decodes to `null`
/// rather than to a silently wrong zero.
fn integer_to_json(inner: &Json) -> Json {
    inner
        .as_str()
        .and_then(|text| text.parse::<i64>().ok())
        .map_or_else(|| inner.clone(), Json::from)
}

fn array_to_json(inner: &Json) -> Json {
    let values = inner.get("values").and_then(Json::as_array);
    Json::Array(
        values
            .map(|items| items.iter().map(to_json).collect())
            .unwrap_or_default(),
    )
}

fn map_to_json(inner: &Json) -> Json {
    let fields = inner.get("fields").and_then(Json::as_object);
    Json::Object(fields.map(fields_to_json).unwrap_or_default())
}

/// Integers go out quoted, floats go out bare. A float that happens to be
/// whole stays a float: the app reads `offset` and `duration` as doubles.
fn number_from_json(number: &serde_json::Number) -> Json {
    number.as_i64().map_or_else(
        || {
            number.as_u64().map_or_else(
                || tagged("doubleValue", Json::Number(number.clone())),
                |unsigned| tagged("integerValue", Json::String(unsigned.to_string())),
            )
        },
        |signed| tagged("integerValue", Json::String(signed.to_string())),
    )
}

#[cfg(test)]
mod decoding {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_integer_arrives_as_a_string_and_leaves_as_a_number() {
        assert_eq!(
            to_json(&json!({ "integerValue": "1758572400" })),
            json!(1_758_572_400)
        );
    }

    #[test]
    fn a_negative_integer_survives_the_quotes() {
        assert_eq!(to_json(&json!({ "integerValue": "-240" })), json!(-240));
    }

    #[test]
    fn a_double_arrives_unquoted() {
        assert_eq!(to_json(&json!({ "doubleValue": 240.5 })), json!(240.5));
    }

    #[test]
    fn a_null_decodes_to_null() {
        assert_eq!(to_json(&json!({ "nullValue": null })), Json::Null);
    }

    #[test]
    fn a_map_decodes_to_an_object() {
        let wire = json!({
            "mapValue": { "fields": {
                "active": { "booleanValue": true },
                "uuid": { "stringValue": "a1b2" },
            } }
        });
        assert_eq!(to_json(&wire), json!({ "active": true, "uuid": "a1b2" }));
    }

    #[test]
    fn an_empty_map_decodes_to_an_empty_object() {
        assert_eq!(to_json(&json!({ "mapValue": {} })), json!({}));
    }

    #[test]
    fn an_array_decodes_element_by_element() {
        let wire = json!({ "arrayValue": { "values": [
            { "stringValue": "Mon" },
            { "integerValue": "2" },
        ] } });
        assert_eq!(to_json(&wire), json!(["Mon", 2]));
    }

    #[test]
    fn a_timestamp_decodes_as_its_text() {
        let wire = json!({ "timestampValue": "2025-09-22T18:00:00Z" });
        assert_eq!(to_json(&wire), json!("2025-09-22T18:00:00Z"));
    }

    #[test]
    fn a_tag_this_crate_has_never_seen_decodes_to_null_rather_than_failing() {
        assert_eq!(to_json(&json!({ "somethingNewValue": 1 })), Json::Null);
    }
}

#[cfg(test)]
mod encoding {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_whole_double_stays_a_double() {
        assert_eq!(from_json(&json!(120.0)), json!({ "doubleValue": 120.0 }));
    }

    #[test]
    fn an_integer_goes_out_quoted() {
        assert_eq!(from_json(&json!(5)), json!({ "integerValue": "5" }));
    }

    #[test]
    fn a_map_becomes_a_fields_object() {
        assert_eq!(
            from_json(&json!({ "active": true })),
            json!({ "mapValue": { "fields": { "active": { "booleanValue": true } } } })
        );
    }

    #[test]
    fn an_array_becomes_a_values_list() {
        assert_eq!(
            from_json(&json!(["Mon", "Tue"])),
            json!({ "arrayValue": { "values": [
                { "stringValue": "Mon" },
                { "stringValue": "Tue" },
            ] } })
        );
    }

    #[test]
    fn a_null_goes_out_tagged() {
        assert_eq!(from_json(&Json::Null), json!({ "nullValue": null }));
    }
}

#[cfg(test)]
mod round_trips {
    use super::*;
    use serde_json::json;

    /// The shape a sleep timer actually has, through both directions.
    #[test]
    fn a_sleep_timer_survives_the_round_trip() {
        let original = json!({
            "active": true,
            "paused": false,
            "timestamp": { "seconds": 1_758_572_400.0 },
            "local_timestamp": 1_758_572_400.0,
            "timerStartTime": 1_758_572_400_000.0,
            "uuid": "0123456789abcdef",
            "details": { "sleepLocations": { "onOwnInBed": true } },
        });
        assert_eq!(to_json(&from_json(&original)), original);
    }

    #[test]
    fn an_integer_and_a_whole_double_stay_apart() {
        let original = json!({ "count": 3, "duration": 3.0 });
        let round_tripped = to_json(&from_json(&original));
        assert!(
            round_tripped["count"].is_i64(),
            "an integer stayed an integer"
        );
        assert!(
            round_tripped["duration"].is_f64(),
            "a whole double stayed a double"
        );
    }
}
