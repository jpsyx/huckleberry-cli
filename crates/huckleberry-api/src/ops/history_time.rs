//! Atomic corrections of history times, preserving the original wire fields.

use serde_json::{Map, Value as Json, json};

use super::removal::{newest_filling, summaries_for, summary_of};
use crate::client::{Huckleberry, now_seconds};
use crate::error::{Error, Result};
use crate::firestore::{FieldUpdate, field_path, value};
use crate::rows::RowRef;
use crate::{ids, paths};

const OPERATION: &str = "changing the entry time (if it changed, select it again)";

impl Huckleberry {
    /// Corrects a history row's start while preserving its duration and other fields.
    /// Returns its new location; timestamp-based IDs move in the same atomic commit
    /// as source removal and last-entry summary repair. An unchanged time writes nothing.
    ///
    /// # Errors
    ///
    /// Invalid times, missing rows or revisions, concurrent changes, or refused requests.
    /// A failed conditional commit is never retried against a changed row.
    pub async fn update_history_time(
        &self,
        cid: &str,
        at: &RowRef,
        started: f64,
    ) -> Result<RowRef> {
        super::timing::validate_time(started, None)?;
        if started > now_seconds() {
            return Err(Error::Invalid("an entry cannot start in the future".into()));
        }
        let token = self.token().await?;
        let source = self
            .firestore()
            .get_raw(&token, &at.document_path(cid), OPERATION)
            .await?
            .ok_or_else(|| Error::Invalid("that entry no longer exists".into()))?;
        let mut fields = row_fields(&source, at)?.clone();
        let original_start = fields
            .get("start")
            .map(value::to_json)
            .and_then(|start| start.as_f64())
            .ok_or_else(|| Error::Invalid("the entry has no readable start time".into()))?;
        if (original_start - started).abs() < f64::EPSILON {
            return Ok(at.clone());
        }
        let destination = destination(at, started);
        let now = now_seconds();
        self.change_time_fields(&mut fields, &destination, started, now);
        let mut writes = row_writes(&source, at, &destination, &fields)?;
        if let Some(summary) = self.time_summary_write(cid, at, &fields, now).await? {
            writes.push(summary);
        }
        self.firestore().commit(&token, &writes, OPERATION).await?;
        Ok(destination)
    }

    /// Changes only event metadata; durations and unknown tagged values remain intact.
    fn change_time_fields(
        &self,
        fields: &mut Map<String, Json>,
        destination: &RowRef,
        started: f64,
        now: f64,
    ) {
        set(fields, "start", &json!(started));
        set(
            fields,
            "offset",
            &json!(self.zone().offset_minutes(started)),
        );
        if let Some(duration) = duration(fields) {
            set(
                fields,
                "end_offset",
                &json!(self.zone().offset_minutes(started + duration)),
            );
        } else if fields.contains_key("end_offset") {
            set(
                fields,
                "end_offset",
                &json!(self.zone().offset_minutes(started)),
            );
        }
        set(fields, "lastUpdated", &json!(now));
        if fields.contains_key("timestamp") {
            set(fields, "timestamp", &json!({"seconds": now}));
        }
        if fields.contains_key("local_timestamp") {
            set(fields, "local_timestamp", &json!(now));
        }
        if fields.contains_key("_id") {
            set(
                fields,
                "_id",
                &json!(
                    destination
                        .batch_key
                        .as_ref()
                        .unwrap_or(&destination.document_id)
                ),
            );
        }
    }

    /// Recomputes the affected summaries from history with the corrected row substituted.
    async fn time_summary_write(
        &self,
        cid: &str,
        at: &RowRef,
        fields: &Map<String, Json>,
        now: f64,
    ) -> Result<Option<Json>> {
        let row = Json::Object(value::fields_to_json(fields));
        let summaries: Vec<_> = summaries_for(&at.tracker)
            .iter()
            .filter(|summary| summary.fills.matches(&row))
            .collect();
        if summaries.is_empty() {
            return Ok(None);
        }
        let token = self.token().await?;
        let tracker = self
            .firestore()
            .get_raw(&token, &paths::tracker(&at.tracker, cid), OPERATION)
            .await?;
        let Some(tracker) = tracker else {
            return Err(Error::Invalid(
                "the tracker is missing; refresh before editing".into(),
            ));
        };
        let mut rows = self.located_rows(&at.tracker, cid, OPERATION).await?;
        rows.retain(|(location, _)| location != at);
        rows.push((at.clone(), row));
        let updates = summary_updates(&rows, &summaries, now);
        let (fields, mask) = field_path::document_and_mask(&updates);
        Ok(Some(
            json!({"update": {"name": name(&tracker)?, "fields": fields},
            "updateMask": {"fieldPaths": mask}, "currentDocument": revision(&tracker)?}),
        ))
    }
}

/// The exact row's tagged fields, rejecting a stale loose reference to a batch.
fn row_fields<'a>(source: &'a Json, at: &RowRef) -> Result<&'a Map<String, Json>> {
    let fields = source
        .get("fields")
        .and_then(Json::as_object)
        .ok_or_else(|| Error::Invalid("the entry has no fields".into()))?;
    if let Some(key) = &at.batch_key {
        return fields
            .get("data")
            .and_then(|data| data.get("mapValue"))
            .and_then(|map| map.get("fields"))
            .and_then(|data| data.get(key))
            .and_then(|row| row.get("mapValue"))
            .and_then(|map| map.get("fields"))
            .and_then(Json::as_object)
            .ok_or_else(|| Error::Invalid("that batched entry no longer exists".into()));
    }
    if fields.get("multi").map(value::to_json) == Some(json!(true)) {
        return Err(Error::Invalid(
            "that entry is now a batch; select it again".into(),
        ));
    }
    Ok(fields)
}

/// Timestamp-shaped identifiers retain their random suffix; opaque IDs stay opaque.
fn destination(at: &RowRef, started: f64) -> RowRef {
    if at.tracker == paths::SLEEP {
        return at.clone();
    }
    let id = at.batch_key.as_ref().unwrap_or(&at.document_id);
    let timestamp_suffix = id
        .split_once('-')
        .filter(|(prefix, suffix)| !suffix.is_empty() && prefix.parse::<i64>().is_ok());
    let new_id = if let Some((_, suffix)) = timestamp_suffix {
        format!("{}-{suffix}", (started * 1000.0) as i64)
    } else if matches!(
        at.tracker.as_str(),
        paths::FEED | paths::DIAPER | paths::HEALTH
    ) {
        ids::interval_id(started)
    } else {
        return at.clone();
    };
    if &new_id == id {
        return at.clone();
    }
    RowRef::loose(&at.tracker, &new_id)
}

/// Builds one revision-guarded replacement, or a create and conditional source removal.
fn row_writes(
    source: &Json,
    at: &RowRef,
    destination: &RowRef,
    fields: &Map<String, Json>,
) -> Result<Vec<Json>> {
    let source_name = name(source)?;
    let condition = revision(source)?;
    if destination == at {
        let mut write = json!({"update": {"name": source_name, "fields": fields}, "currentDocument": condition});
        if let Some(key) = &at.batch_key {
            write["update"]["fields"] = json!({"data": {"mapValue": {"fields": {
                key: {"mapValue": {"fields": fields}}
            }}}});
            write["updateMask"] = json!({"fieldPaths": [field_path::render(&at.field_prefix())]});
        }
        return Ok(vec![write]);
    }
    let parent = source_name
        .rsplit_once('/')
        .map_or("", |(parent, _)| parent);
    let create = json!({"update": {"name": format!("{parent}/{}", destination.document_id), "fields": fields},
        "currentDocument": {"exists": false}});
    let remove = if at.is_batched() {
        json!({"update": {"name": source_name, "fields": {}}, "currentDocument": condition,
            "updateMask": {"fieldPaths": [field_path::render(&at.field_prefix())]}})
    } else {
        json!({"delete": source_name, "currentDocument": condition})
    };
    Ok(vec![create, remove])
}

/// A document's server identity is required by the commit protocol.
fn name(document: &Json) -> Result<&str> {
    document
        .get("name")
        .and_then(Json::as_str)
        .ok_or_else(|| Error::Invalid("the entry has no server name; read it again".into()))
}

/// Missing revisions must never silently weaken the concurrency guard.
fn revision(document: &Json) -> Result<Json> {
    document
        .get("updateTime")
        .and_then(Json::as_str)
        .map(|revision| json!({"updateTime": revision}))
        .ok_or_else(|| Error::Invalid("the entry has no server revision; read it again".into()))
}

/// Encodes only changed values, leaving all other Firestore tags untouched.
fn set(fields: &mut Map<String, Json>, key: &str, value: &Json) {
    fields.insert(key.into(), value::from_json(value));
}

/// Nursing stores its duration as two side totals instead of one duration field.
fn duration(fields: &Map<String, Json>) -> Option<f64> {
    let number = |key| {
        fields
            .get(key)
            .map(value::to_json)
            .and_then(|value| value.as_f64())
    };
    number("duration").or_else(|| Some(number("leftDuration")? + number("rightDuration")?))
}

/// The last-nursing summary includes the total in addition to both side durations.
fn time_summary(row: &Json, keys: &[(&str, &str)]) -> Json {
    let mut summary = summary_of(row, keys);
    if row.get("mode").and_then(Json::as_str) == Some("breast")
        && let (Some(left), Some(right)) =
            (row["leftDuration"].as_f64(), row["rightDuration"].as_f64())
    {
        summary["duration"] = json!(left + right);
    }
    summary
}

/// Repairs only this row's summary kind and its related nursing-side hint.
fn summary_updates(
    rows: &[(RowRef, Json)],
    summaries: &[&super::removal::Summary],
    now: f64,
) -> Vec<FieldUpdate> {
    let mut updates = Vec::new();
    for summary in summaries {
        if let Some(row) = newest_filling(rows, summary.fills) {
            updates.push(FieldUpdate::set(
                summary.field,
                time_summary(row, summary.keys),
            ));
            if summary.field == "prefs.lastNursing" {
                updates.push(FieldUpdate::set_or_clear(
                    "prefs.lastSide",
                    row.get("lastSide")
                        .map(|side| json!({"start": row["start"], "lastSide": side})),
                ));
            }
        }
    }
    updates.push(FieldUpdate::set("prefs.timestamp", json!({"seconds": now})));
    updates.push(FieldUpdate::set("prefs.local_timestamp", json!(now)));
    updates
}
