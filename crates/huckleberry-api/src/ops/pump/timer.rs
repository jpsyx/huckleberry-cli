//! Pump timer transitions, preserving UUIDs and unknown tracker fields.

use serde_json::json;

use super::{CompletedPump, PumpEntry, resolve_amounts, validate_time, validate_units};
use crate::client::{Huckleberry, now_seconds};
use crate::error::{Error, Result};
use crate::firestore::{FieldUpdate, field_path};
use crate::models::feed::VolumeUnits;
use crate::models::pump::{PumpAmounts, PumpTimer};
use crate::{TimerChange, ids, paths};

impl Huckleberry {
    /// Starts a pumping timer at the current time, unless one is already active.
    ///
    /// # Errors
    /// A refused or unreachable Firestore.
    pub async fn start_pump(&self, cid: &str) -> Result<TimerChange> {
        self.start_pump_at(cid, now_seconds()).await
    }

    /// Starts a pumping timer at an explicit Unix timestamp in seconds.
    ///
    /// # Errors
    /// Invalid or future time, or a refused or unreachable Firestore.
    pub async fn start_pump_at(&self, cid: &str, at: f64) -> Result<TimerChange> {
        validate_time(at, None)?;
        let document = self.pump_document(cid).await?;
        if document
            .as_ref()
            .is_some_and(|document| document.running_timer().is_some())
        {
            return Ok(TimerChange::Unchanged);
        }
        let session = document
            .as_ref()
            .and_then(|document| document.timer.as_ref())
            .map_or_else(ids::session_id, |timer| timer.uuid.clone());
        let mut updates = timestamp_updates(now_seconds());
        updates.extend([
            FieldUpdate::set("timer.active", json!(true)),
            FieldUpdate::set("timer.paused", json!(false)),
            FieldUpdate::set("timer.startTime", json!(at * 1000.0)),
            FieldUpdate::set("timer.uuid", json!(session)),
            FieldUpdate::delete("timer.endTime"),
        ]);
        if document.is_none() {
            let token = self.token().await?;
            let (fields, _) = field_path::document_and_mask(&updates);
            self.firestore()
                .merge(
                    &token,
                    &paths::tracker(paths::PUMP, cid),
                    &fields,
                    "starting a pumping session",
                )
                .await?;
            return Ok(TimerChange::Applied);
        }
        self.update_pump_timer(cid, &updates, "starting a pumping session")
            .await
    }

    /// Pauses the active pumping timer at the current time.
    ///
    /// # Errors
    /// As [`Huckleberry::pause_pump_at`].
    pub async fn pause_pump(&self, cid: &str) -> Result<TimerChange> {
        self.pause_pump_at(cid, now_seconds()).await
    }

    /// Pauses at an explicit Unix timestamp in seconds.
    ///
    /// # Errors
    /// Invalid timer times, or a refused or unreachable Firestore.
    pub async fn pause_pump_at(&self, cid: &str, at: f64) -> Result<TimerChange> {
        self.transition_pump(cid, at, true).await
    }

    /// Resumes the active paused pumping timer, retaining its original start.
    ///
    /// # Errors
    /// As [`Huckleberry::resume_pump_at`].
    pub async fn resume_pump(&self, cid: &str) -> Result<TimerChange> {
        self.resume_pump_at(cid, now_seconds()).await
    }

    /// Resumes at an explicit Unix timestamp in seconds.
    /// The pause remains part of the session duration, matching upstream.
    ///
    /// # Errors
    /// Invalid timer times, or a refused or unreachable Firestore.
    pub async fn resume_pump_at(&self, cid: &str, at: f64) -> Result<TimerChange> {
        self.transition_pump(cid, at, false).await
    }

    async fn transition_pump(&self, cid: &str, at: f64, pause: bool) -> Result<TimerChange> {
        validate_time(at, None)?;
        let Some(timer) = self.active_pump_timer(cid).await? else {
            return Ok(TimerChange::NotRunning);
        };
        if (timer.paused == Some(true)) == pause {
            return Ok(TimerChange::Unchanged);
        }
        validate_transition(&timer, at)?;
        let mut updates = timestamp_updates(now_seconds());
        updates.extend([
            FieldUpdate::set("timer.active", json!(true)),
            FieldUpdate::set("timer.paused", json!(pause)),
            FieldUpdate::set_or_clear("timer.endTime", pause.then(|| json!(at * 1000.0))),
        ]);
        self.update_pump_timer(cid, &updates, "changing the pumping timer")
            .await
    }

    /// Cancels the pumping session without saving history.
    ///
    /// # Errors
    /// A refused or unreachable Firestore.
    pub async fn cancel_pump(&self, cid: &str) -> Result<TimerChange> {
        let Some(timer) = self.active_pump_timer(cid).await? else {
            return Ok(TimerChange::NotRunning);
        };
        self.finish_pump_timer(cid, &timer.uuid).await
    }

    /// Completes the pumping session, using timer units or ml when omitted.
    ///
    /// # Errors
    /// Invalid amounts, units or timer state, or a refused or unreachable Firestore.
    pub async fn complete_pump(
        &self,
        cid: &str,
        amounts: PumpAmounts,
        units: Option<VolumeUnits>,
        notes: Option<&str>,
    ) -> Result<Option<CompletedPump>> {
        self.complete_pump_at(cid, amounts, units, notes, now_seconds())
            .await
    }

    /// Completes at an explicit Unix timestamp in seconds, or the pause endpoint.
    /// Returns `None` when there is no active pumping session.
    ///
    /// # Errors
    /// Invalid amounts, units or timer times, or a refused or unreachable Firestore.
    pub async fn complete_pump_at(
        &self,
        cid: &str,
        amounts: PumpAmounts,
        units: Option<VolumeUnits>,
        notes: Option<&str>,
        at: f64,
    ) -> Result<Option<CompletedPump>> {
        resolve_amounts(amounts)?;
        if let Some(units) = &units {
            validate_units(units)?;
        }
        validate_time(at, None)?;
        let Some(timer) = self.active_pump_timer(cid).await? else {
            return Ok(None);
        };
        let start = validate_transition(&timer, at)?;
        let duration = timer.elapsed_seconds(at).ok_or_else(|| {
            Error::Invalid("pump timer endpoint must not precede its start".into())
        })?;
        let entry = PumpEntry {
            amounts,
            duration: Some(duration),
            notes,
            units: units.or(timer.units).unwrap_or(VolumeUnits::Millilitres),
        };
        self.record_pump(cid, entry, start, false).await?;
        self.finish_pump_timer(cid, &timer.uuid).await?;
        Ok(Some(CompletedPump { start, duration }))
    }

    async fn active_pump_timer(&self, cid: &str) -> Result<Option<PumpTimer>> {
        Ok(self
            .pump_document(cid)
            .await?
            .and_then(|document| document.timer)
            .filter(|timer| timer.active))
    }

    async fn finish_pump_timer(&self, cid: &str, session: &str) -> Result<TimerChange> {
        let now = now_seconds();
        let mut updates = timestamp_updates(now);
        updates.extend([
            FieldUpdate::set("timer.active", json!(false)),
            FieldUpdate::set("timer.uuid", json!(session)),
            FieldUpdate::set("timer.startTime", json!(now * 1000.0)),
            FieldUpdate::delete("timer.paused"),
            FieldUpdate::delete("timer.endTime"),
            FieldUpdate::delete("timer.entryMode"),
            FieldUpdate::delete("timer.units"),
            FieldUpdate::delete("timer.notes"),
        ]);
        self.update_pump_timer(cid, &updates, "ending the pumping timer")
            .await
    }

    async fn update_pump_timer(
        &self,
        cid: &str,
        updates: &[FieldUpdate],
        operation: &str,
    ) -> Result<TimerChange> {
        let token = self.token().await?;
        self.firestore()
            .update(
                &token,
                &paths::tracker(paths::PUMP, cid),
                updates,
                operation,
            )
            .await?;
        Ok(TimerChange::Applied)
    }
}

fn timestamp_updates(now: f64) -> Vec<FieldUpdate> {
    vec![
        FieldUpdate::set("timer.timestamp", json!({"seconds":now})),
        FieldUpdate::set("timer.local_timestamp", json!(now)),
    ]
}

fn validate_transition(timer: &PumpTimer, at: f64) -> Result<f64> {
    let start = timer
        .started_at()
        .ok_or_else(|| Error::Invalid("active pump timer is missing startTime".into()))?;
    validate_time(start, None)?;
    let earliest = if timer.paused == Some(true) {
        timer.end_time.map_or(start, |time| time.as_f64() / 1000.0)
    } else {
        start
    };
    validate_time(earliest, Some(start))?;
    validate_time(at, Some(earliest))?;
    Ok(start)
}
