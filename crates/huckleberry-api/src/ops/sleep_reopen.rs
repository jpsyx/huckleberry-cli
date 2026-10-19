//! Reopen history and restore the timer in one conditional commit.
mod eligibility;

use super::removal::{Fills, newest_filling, summaries_for, summary_of};
use crate::{
    Huckleberry, RowRef,
    client::now_seconds,
    error::{Error, Result},
    firestore::{FieldUpdate, field_path, value},
    ids, paths,
};
use serde_json::{Value as Json, json};

const OPERATION: &str = "resuming the completed sleep (refresh and select it again if it changed)";
const OTHER_TRACKERS: [&str; 6] = [
    paths::FEED,
    paths::DIAPER,
    paths::HEALTH,
    paths::PUMP,
    paths::MILESTONES,
    paths::ACTIVITIES,
];

/// Read context retained until the conditional write, never shared with the CLI.
struct Reopening {
    tracker: Json,
    sleeps: Vec<(RowRef, Json)>,
    at: RowRef,
    row: Json,
}

impl Huckleberry {
    /// Finds the latest completed sleep when no later event or active timer blocks it.
    /// Checks every supported history tracker, independently of display windows/limits.
    ///
    /// # Errors
    /// Any refused or unreadable history read; absence is never inferred from a failure.
    pub async fn resumable_sleep(&self, cid: &str) -> Result<Option<RowRef>> {
        Ok(self.sleep_reopening(cid).await?.map(|reading| reading.at))
    }

    /// Rechecks eligibility, then atomically replaces a completed row with a running
    /// sleep from its original start, preserving details and repairing last sleep.
    ///
    /// # Errors
    /// Ineligible/replaced sleep, missing revisions, concurrent edits or refused requests.
    /// A failed commit is not retried. Other trackers are checked immediately before
    /// writing, but their histories are not locked against simultaneous new events.
    pub async fn resume_completed_sleep(&self, cid: &str, at: &RowRef) -> Result<()> {
        if at.tracker != paths::SLEEP {
            return Err(Error::Invalid(
                "only a completed sleep can be resumed".into(),
            ));
        }
        let reading = self.sleep_reopening(cid).await?
            .filter(|reading| &reading.at == at)
            .ok_or_else(|| Error::Invalid("this is not the latest completed sleep, another timer is active, or a later event prevents resuming it".into()))?;
        let token = self.token().await?;
        let source = self
            .firestore()
            .get_raw(&token, &at.document_path(cid), OPERATION)
            .await?
            .ok_or_else(|| Error::Invalid("that sleep no longer exists".into()))?;
        let fields = source_fields(&source, at)?;
        if Json::Object(value::fields_to_json(fields)) != reading.row {
            return Err(Error::Invalid("the sleep changed; select it again".into()));
        }
        let writes = [
            remove_write(&source, at)?,
            timer_write(&reading, fields, now_seconds())?,
        ];
        self.firestore().commit(&token, &writes, OPERATION).await
    }

    /// Unlike general timeline reads, never silently omit an unreadable batch.
    async fn reopening_rows(&self, tracker: &str, cid: &str) -> Result<Vec<(RowRef, Json)>> {
        let token = self.token().await?;
        let documents = self
            .firestore()
            .list(
                &token,
                &paths::tracker(tracker, cid),
                paths::history_collection(tracker),
                OPERATION,
            )
            .await?;
        let mut rows = Vec::new();
        for document in documents {
            let id = document.id.clone();
            let row = document.into_json();
            if row.get("multi") == Some(&Json::Bool(true)) {
                let batch = row.get("data").and_then(Json::as_object).ok_or_else(|| {
                    Error::Invalid("unreadable history batch; cannot check sleep resumption".into())
                })?;
                rows.extend(
                    batch
                        .iter()
                        .map(|(key, row)| (RowRef::batched(tracker, &id, key), row.clone())),
                );
            } else {
                rows.push((RowRef::loose(tracker, &id), row));
            }
        }
        Ok(rows)
    }

    /// All reads must succeed before an option can be offered or a save attempted.
    async fn sleep_reopening(&self, cid: &str) -> Result<Option<Reopening>> {
        let token = self.token().await?;
        let Some(tracker) = self
            .firestore()
            .get_raw(&token, &paths::tracker(paths::SLEEP, cid), OPERATION)
            .await?
        else {
            return Ok(None);
        };
        if eligibility::active(&plain_document(&tracker)) {
            return Ok(None);
        }
        let sleeps = self.reopening_rows(paths::SLEEP, cid).await?;
        let Some((at, row)) = eligibility::candidate(&sleeps, now_seconds()) else {
            return Ok(None);
        };
        let end = eligibility::endpoint(row).expect("candidate has a valid endpoint");
        if self.has_resume_blockers(cid, end).await? {
            return Ok(None);
        }
        Ok(Some(Reopening {
            tracker,
            at: at.clone(),
            row: row.clone(),
            sleeps,
        }))
    }
    /// Existing live activity and later history share the same refusal policy.
    async fn has_resume_blockers(&self, cid: &str, end: f64) -> Result<bool> {
        for tracker in OTHER_TRACKERS {
            let rows = self.reopening_rows(tracker, cid).await?;
            if rows.iter().any(|(_, row)| eligibility::blocks(row, end)) {
                return Ok(true);
            }
        }
        for tracker in [paths::FEED, paths::PUMP] {
            let document: Option<Json> = self
                .document(&paths::tracker(tracker, cid), OPERATION)
                .await?;
            if document.as_ref().is_some_and(eligibility::active) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Decode only for comparison; the source fields remain tagged for the write.
fn plain_document(document: &Json) -> Json {
    document
        .get("fields")
        .and_then(Json::as_object)
        .map_or(Json::Null, |fields| {
            Json::Object(value::fields_to_json(fields))
        })
}

/// Select one row without ever interpreting a batch as a loose entry.
fn source_fields<'a>(document: &'a Json, at: &RowRef) -> Result<&'a serde_json::Map<String, Json>> {
    let fields = document
        .get("fields")
        .and_then(Json::as_object)
        .ok_or_else(|| Error::Invalid("the sleep has no fields".into()))?;
    if let Some(key) = &at.batch_key {
        return fields
            .get("data")
            .and_then(|data| data.get("mapValue"))
            .and_then(|map| map.get("fields"))
            .and_then(|data| data.get(key))
            .and_then(|row| row.get("mapValue"))
            .and_then(|map| map.get("fields"))
            .and_then(Json::as_object)
            .ok_or_else(|| Error::Invalid("that batched sleep no longer exists".into()));
    }
    if fields.get("multi").map(value::to_json) == Some(json!(true)) {
        return Err(Error::Invalid(
            "the sleep moved into a batch; select it again".into(),
        ));
    }
    Ok(fields)
}

/// Both writes require a server revision; missing metadata cannot weaken the guard.
fn revision(document: &Json) -> Result<Json> {
    document
        .get("updateTime")
        .and_then(Json::as_str)
        .map(|revision| json!({"updateTime":revision}))
        .ok_or_else(|| Error::Invalid("missing server revision; refresh before resuming".into()))
}
fn name(document: &Json) -> Result<&str> {
    document
        .get("name")
        .and_then(Json::as_str)
        .ok_or_else(|| Error::Invalid("missing server name; refresh before resuming".into()))
}

/// A batch deletion masks only the selected key, preserving neighboring entries.
fn remove_write(source: &Json, at: &RowRef) -> Result<Json> {
    let condition = revision(source)?;
    let name = name(source)?;
    Ok(if at.is_batched() {
        json!({"update":{"name":name,"fields":{}},"currentDocument":condition,
            "updateMask":{"fieldPaths":[field_path::render(&at.field_prefix())]}})
    } else {
        json!({"delete":name,"currentDocument":condition})
    })
}

/// Replace the idle timer and last-sleep summary while preserving tracker siblings.
fn timer_write(
    reading: &Reopening,
    source: &serde_json::Map<String, Json>,
    now: f64,
) -> Result<Json> {
    let remaining: Vec<_> = reading
        .sleeps
        .iter()
        .filter(|(at, _)| at != &reading.at)
        .cloned()
        .collect();
    let previous = newest_filling(&remaining, Fills::Any)
        .map(|row| summary_of(row, summaries_for(paths::SLEEP)[0].keys));
    let updates = [
        FieldUpdate::set(
            "timer",
            json!({"active":true,"paused":false,
            "timerStartTime":reading.row["start"].as_f64().expect("validated start") * 1000.0,
            "uuid":ids::session_id(),"timestamp":{"seconds":now},"local_timestamp":now}),
        ),
        FieldUpdate::set_or_clear("prefs.lastSleep", previous),
        FieldUpdate::set("prefs.timestamp", json!({"seconds":now})),
        FieldUpdate::set("prefs.local_timestamp", json!(now)),
    ];
    let (mut fields, mask) = field_path::document_and_mask(&updates);
    // Preserve even unmodeled detail fields and Firestore wire types.
    if let Some(details) = source.get("details") {
        fields.get_mut("timer").expect("timer update")["mapValue"]["fields"]["details"] =
            details.clone();
    }
    Ok(
        json!({"update":{"name":name(&reading.tracker)?,"fields":fields},
        "updateMask":{"fieldPaths":mask},"currentDocument":revision(&reading.tracker)?}),
    )
}
