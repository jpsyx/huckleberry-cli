//! Field paths and update masks: which parts of a document a write touches.
//!
//! A Firestore write does not say "merge this in". It sends a document body
//! and an `updateMask`, and the mask is the whole of the semantics: a field
//! named in the mask and present in the body is written, a field named in the
//! mask and absent from the body is **deleted**, and a field in neither is
//! left alone. Both operations this crate needs are expressed that way:
//!
//! - a merge write (`set(..., merge=True)` in the Python client) names every
//!   leaf of the payload, so sibling fields the payload does not mention
//!   survive;
//! - a field update (`update({"timer.paused": True})`) names dotted paths, and
//!   deleting a field is that path in the mask with nothing in the body.
//!
//! The escaping matters more than it looks. Huckleberry stores a sleep
//! condition under the key `10-20_minutes`, and a mask entry that is not a
//! plain identifier has to be backtick-quoted or the write is rejected.

use serde_json::{Map, Value as Json};

use super::value;

/// One field to write, or to delete.
///
/// A `value` of `None` is a deletion: the path goes into the mask and nothing
/// goes into the body, which is how Firestore spells "remove this field".
#[derive(Debug, Clone, PartialEq)]
pub struct FieldUpdate {
    /// The path, already split: `["timer", "paused"]`, never `"timer.paused"`.
    /// Splitting at the call site is what keeps a key containing a dot from
    /// silently becoming two segments.
    pub path: Vec<String>,
    /// What to write, or `None` to delete the field.
    pub value: Option<Json>,
}

impl FieldUpdate {
    /// Writes `value` at a dotted path.
    #[must_use]
    pub fn set(path: &str, value: Json) -> Self {
        Self {
            path: split(path),
            value: Some(value),
        }
    }

    /// Writes a value, or removes the field when there is none.
    ///
    /// What an edit of an optional field is: a colour nobody gave this time is
    /// a colour that is no longer on the row, not a colour left over from the
    /// last time somebody answered.
    #[must_use]
    pub fn set_or_clear(path: &str, value: Option<Json>) -> Self {
        Self {
            path: split(path),
            value,
        }
    }

    /// Deletes whatever is at a dotted path.
    #[must_use]
    pub fn delete(path: &str) -> Self {
        Self {
            path: split(path),
            value: None,
        }
    }
}

/// Splits a dotted path into segments.
#[must_use]
pub fn split(path: &str) -> Vec<String> {
    path.split('.').map(ToOwned::to_owned).collect()
}

/// Renders one path segment as Firestore spells it in a mask: bare when it is
/// a plain identifier, backtick-quoted when it is anything else.
#[must_use]
pub fn escape(segment: &str) -> String {
    let mut characters = segment.chars();
    let identifier = matches!(characters.next(), Some(first) if first == '_' || first.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric());
    if identifier {
        return segment.to_owned();
    }
    let escaped = segment.replace('\\', "\\\\").replace('`', "\\`");
    format!("`{escaped}`")
}

/// Renders a split path as one mask entry.
#[must_use]
pub fn render(path: &[String]) -> String {
    path.iter()
        .map(|segment| escape(segment))
        .collect::<Vec<_>>()
        .join(".")
}

/// The leaf paths of a plain JSON object.
///
/// The order is `serde_json`'s own, which is by key: a mask is a set, so the
/// only thing that matters is that the order is stable from one run to the
/// next.
///
/// An empty object is itself a leaf: writing `{}` at a path means "an empty
/// map lives here", and recursing past it would name nothing at all.
#[must_use]
pub fn leaves(object: &Map<String, Json>) -> Vec<String> {
    let mut paths = Vec::new();
    collect_leaves(object, &mut Vec::new(), &mut paths);
    paths
}

fn collect_leaves(object: &Map<String, Json>, prefix: &mut Vec<String>, paths: &mut Vec<String>) {
    for (key, value) in object {
        prefix.push(key.clone());
        match value.as_object() {
            Some(nested) if !nested.is_empty() => collect_leaves(nested, prefix, paths),
            _ => paths.push(render(prefix)),
        }
        prefix.pop();
    }
}

/// The document body and the update mask a set of field writes implies.
///
/// Writes that share a prefix are merged into one nested map, because the body
/// is a document and a document cannot hold the same key twice.
#[must_use]
pub fn document_and_mask(updates: &[FieldUpdate]) -> (Map<String, Json>, Vec<String>) {
    let mut tree = Map::new();
    let mut mask = Vec::new();
    for update in updates {
        let entry = render(&update.path);
        if !mask.contains(&entry) {
            mask.push(entry);
        }
        if let Some(value) = &update.value {
            graft(&mut tree, &update.path, value.clone());
        }
    }
    (value::fields_from_json(&tree), mask)
}

/// Plants `value` at `path` inside `tree`, creating the maps along the way.
fn graft(tree: &mut Map<String, Json>, path: &[String], value: Json) {
    let Some((key, rest)) = path.split_first() else {
        return;
    };
    if rest.is_empty() {
        tree.insert(key.clone(), value);
        return;
    }
    let branch = tree
        .entry(key.clone())
        .or_insert_with(|| Json::Object(Map::new()));
    if !branch.is_object() {
        *branch = Json::Object(Map::new());
    }
    if let Some(nested) = branch.as_object_mut() {
        graft(nested, rest, value);
    }
}

#[cfg(test)]
mod escaping {
    use super::*;

    #[test]
    fn an_ordinary_segment_needs_no_quoting() {
        assert_eq!(escape("timerStartTime"), "timerStartTime");
    }

    #[test]
    fn an_underscore_may_lead() {
        assert_eq!(escape("_id"), "_id");
    }

    #[test]
    fn a_segment_that_is_not_an_identifier_is_backticked() {
        assert_eq!(escape("10-20_minutes"), "`10-20_minutes`");
    }

    #[test]
    fn a_backtick_inside_a_segment_is_escaped() {
        assert_eq!(escape("a`b"), "`a\\`b`");
    }

    #[test]
    fn an_empty_segment_is_backticked_rather_than_dropped() {
        assert_eq!(escape(""), "``");
    }

    #[test]
    fn a_path_escapes_segment_by_segment() {
        let path = split("details.startSleepCondition.10-20_minutes");
        assert_eq!(render(&path), "details.startSleepCondition.`10-20_minutes`");
    }
}

#[cfg(test)]
mod merge_masks {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_merge_writes_one_mask_entry_per_leaf() {
        let payload = json!({ "prefs": { "bottleType": "Formula", "bottleAmount": 90.0 } });
        assert_eq!(
            leaves(payload.as_object().expect("an object")),
            vec!["prefs.bottleAmount", "prefs.bottleType"]
        );
    }

    #[test]
    fn an_empty_map_is_a_leaf_so_it_is_still_written() {
        let payload = json!({ "timer": { "details": {} } });
        assert_eq!(
            leaves(payload.as_object().expect("an object")),
            vec!["timer.details"]
        );
    }

    #[test]
    fn an_array_is_a_leaf_because_firestore_replaces_it_whole() {
        let payload = json!({ "days": ["Mon", "Tue"] });
        assert_eq!(
            leaves(payload.as_object().expect("an object")),
            vec!["days"]
        );
    }

    #[test]
    fn a_leaf_key_that_is_not_an_identifier_is_escaped_in_the_mask() {
        let payload = json!({ "startSleepCondition": { "10-20_minutes": false } });
        assert_eq!(
            leaves(payload.as_object().expect("an object")),
            vec!["startSleepCondition.`10-20_minutes`"]
        );
    }
}

#[cfg(test)]
mod update_bodies {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_dotted_write_becomes_a_nested_document() {
        let updates = [FieldUpdate::set("timer.paused", json!(true))];
        let (fields, mask) = document_and_mask(&updates);
        assert_eq!(
            Json::Object(fields),
            json!({ "timer": { "mapValue": { "fields": {
                "paused": { "booleanValue": true }
            } } } })
        );
        assert_eq!(mask, vec!["timer.paused"]);
    }

    #[test]
    fn writes_sharing_a_prefix_land_in_one_map() {
        let updates = [
            FieldUpdate::set("timer.paused", json!(true)),
            FieldUpdate::set("timer.active", json!(true)),
        ];
        let (fields, mask) = document_and_mask(&updates);
        let timer = &fields["timer"]["mapValue"]["fields"];
        assert!(timer.get("paused").is_some() && timer.get("active").is_some());
        assert_eq!(mask, vec!["timer.paused", "timer.active"]);
    }

    #[test]
    fn a_deleted_field_is_in_the_mask_and_not_in_the_body() {
        let updates = [FieldUpdate::delete("timer.activeSide")];
        let (fields, mask) = document_and_mask(&updates);
        assert!(fields.is_empty(), "nothing is written");
        assert_eq!(mask, vec!["timer.activeSide"]);
    }

    #[test]
    fn a_write_and_a_delete_under_one_prefix_coexist() {
        let updates = [
            FieldUpdate::set("timer.active", json!(false)),
            FieldUpdate::delete("timer.leftDuration"),
        ];
        let (fields, mask) = document_and_mask(&updates);
        let timer = &fields["timer"]["mapValue"]["fields"];
        assert!(timer.get("active").is_some());
        assert!(
            timer.get("leftDuration").is_none(),
            "a delete writes nothing"
        );
        assert_eq!(mask, vec!["timer.active", "timer.leftDuration"]);
    }

    #[test]
    fn a_null_is_written_rather_than_treated_as_a_delete() {
        // `timer.timerStartTime: None` in the Python client is a stored null,
        // not a removal: the app reads the field and expects it to be there.
        let updates = [FieldUpdate::set("timer.timerStartTime", Json::Null)];
        let (fields, _) = document_and_mask(&updates);
        assert_eq!(
            fields["timer"]["mapValue"]["fields"]["timerStartTime"],
            json!({ "nullValue": null })
        );
    }
}
