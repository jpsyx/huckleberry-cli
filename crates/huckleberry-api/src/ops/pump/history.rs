//! Pump history writes and the matching last-session summary.

use serde_json::{Value, json};

use super::{PumpEntry, validate_entry, validate_number, validate_time};
use crate::client::{Huckleberry, now_seconds};
use crate::error::{Error, Result};
use crate::firestore::FieldUpdate;
use crate::models::common::Number;
use crate::models::pump::{PumpDocument, PumpInterval};
use crate::models::{to_fields, to_json};
use crate::ops::feed::bottle::note;
use crate::ops::removal::{summaries_for, summary_of};
use crate::{RowRef, ids, paths};

impl Huckleberry {
    /// Records expressed milk with the current time as the session start.
    ///
    /// # Errors
    /// Invalid amounts, duration or units, or a refused or unreachable Firestore.
    pub async fn log_pump(&self, cid: &str, entry: PumpEntry<'_>) -> Result<()> {
        self.log_pump_at(cid, entry, now_seconds()).await
    }

    /// Records a pumping session at an explicit Unix timestamp in seconds.
    /// Event time governs row IDs and offsets; synchronization fields use now.
    ///
    /// # Errors
    /// As [`Huckleberry::log_pump`], including invalid or future start times.
    pub async fn log_pump_at(&self, cid: &str, entry: PumpEntry<'_>, at: f64) -> Result<()> {
        self.record_pump(cid, entry, at, true).await
    }

    pub(super) async fn record_pump(
        &self,
        cid: &str,
        entry: PumpEntry<'_>,
        at: f64,
        update_pref_timestamps: bool,
    ) -> Result<()> {
        let (mode, left, right) = validate_entry(&entry)?;
        validate_time(at, None)?;
        if let Some(duration) = entry.duration {
            validate_number(at + duration, "session end")?;
        }
        let document = self.pump_document(cid).await?;
        let now = now_seconds();
        let interval = PumpInterval {
            start: at.into(),
            entry_mode: mode,
            left_amount: Some(left.into()),
            right_amount: Some(right.into()),
            units: entry.units,
            offset: self.zone().offset_minutes(at).into(),
            duration: entry.duration.map(Number::from),
            end_offset: entry
                .duration
                .map(|duration| self.zone().offset_minutes(at + duration).into()),
            last_updated: Some(now.into()),
            notes: note(entry.notes).and_then(|note| note.as_str().map(str::to_owned)),
        };
        let token = self.token().await?;
        self.firestore()
            .set(
                &token,
                &paths::history_row(paths::PUMP, cid, &ids::interval_id(at)),
                &to_fields(&interval)?,
                "recording the pumping session",
            )
            .await?;
        self.update_last_pump(
            cid,
            document.as_ref(),
            &interval,
            now,
            update_pref_timestamps,
        )
        .await
    }

    async fn update_last_pump(
        &self,
        cid: &str,
        document: Option<&PumpDocument>,
        interval: &PumpInterval,
        now: f64,
        update_timestamps: bool,
    ) -> Result<()> {
        let previous = document
            .and_then(|doc| doc.prefs.as_ref())
            .and_then(|prefs| prefs.last_pump.as_ref())
            .and_then(|last| last.start)
            .map(Number::as_f64);
        if !crate::ops::sleep::replaces_last(previous, interval.start.as_f64()) {
            return Ok(());
        }
        let summary = summary_of(&to_json(interval)?, summaries_for(paths::PUMP)[0].keys);
        let mut updates = vec![FieldUpdate::set("prefs.lastPump", summary)];
        if update_timestamps {
            updates.push(FieldUpdate::set("prefs.timestamp", json!({"seconds":now})));
            updates.push(FieldUpdate::set("prefs.local_timestamp", json!(now)));
        }
        let token = self.token().await?;
        if document.is_none() {
            let (fields, _) = crate::firestore::field_path::document_and_mask(&updates);
            return self
                .firestore()
                .merge(
                    &token,
                    &paths::tracker(paths::PUMP, cid),
                    &fields,
                    "recording the latest pumping session",
                )
                .await;
        }
        self.firestore()
            .update(
                &token,
                &paths::tracker(paths::PUMP, cid),
                &updates,
                "recording the latest pumping session",
            )
            .await
    }

    /// Changes measurements on a recorded pump and repairs its latest summary.
    /// Omitted duration and notes remove the old values; the start stays unchanged.
    ///
    /// # Errors
    /// Invalid values, a missing row, or a refused or unreachable Firestore.
    pub async fn update_pump_entry(
        &self,
        cid: &str,
        at: &RowRef,
        entry: PumpEntry<'_>,
    ) -> Result<()> {
        if at.tracker != paths::PUMP {
            return Err(Error::Invalid("that entry is not a pump".into()));
        }
        let (mode, left, right) = validate_entry(&entry)?;
        let updates = [
            FieldUpdate::set("entryMode", json!(mode.as_str())),
            FieldUpdate::set("leftAmount", json!(left)),
            FieldUpdate::set("rightAmount", json!(right)),
            FieldUpdate::set("units", json!(entry.units.as_str())),
            FieldUpdate::set_or_clear("duration", entry.duration.map(|duration| json!(duration))),
            FieldUpdate::set_or_clear("notes", note(entry.notes)),
            FieldUpdate::set("lastUpdated", json!(now_seconds())),
        ];
        self.update_history_row(cid, at, &updates, "changing the pumping session")
            .await
    }

    pub(crate) async fn pump_history_updates(
        &self,
        cid: &str,
        at: &RowRef,
        updates: &[FieldUpdate],
    ) -> Result<(f64, Vec<FieldUpdate>)> {
        let document: Value = self
            .document(&at.document_path(cid), "reading the pumping entry")
            .await?
            .ok_or_else(|| Error::Invalid("that pumping entry no longer exists".into()))?;
        let row = at
            .batch_key
            .as_ref()
            .map_or(Some(&document), |key| document.get("data")?.get(key))
            .ok_or_else(|| Error::Invalid("that pumping entry no longer exists".into()))?;
        let started = row
            .get("start")
            .and_then(Value::as_f64)
            .ok_or_else(|| Error::Invalid("the pumping entry has no readable start time".into()))?;
        let mut changed = updates.to_vec();
        if let Some(duration) = updates.iter().find(|update| update.path == ["duration"]) {
            let end_offset = duration
                .value
                .as_ref()
                .map(|duration| {
                    let duration = duration
                        .as_f64()
                        .ok_or_else(|| Error::Invalid("invalid pump duration".into()))?;
                    validate_number(duration, "duration")?;
                    validate_number(started + duration, "session end")?;
                    Ok(json!(self.zone().offset_minutes(started + duration)))
                })
                .transpose()?;
            changed.retain(|update| update.path != ["end_offset"]);
            changed.push(FieldUpdate::set_or_clear("end_offset", end_offset));
        }
        Ok((started, changed))
    }
}
