//! Starting, pausing and finishing a sleep.
//!
//! The unit trap is here: the sleep timer's `timerStartTime` is in
//! **milliseconds**. Everything this module writes into an interval is in
//! seconds, and [`completed_sleep`] is where the division happens.

use serde_json::json;

use super::TimerChange;
use crate::client::{Huckleberry, now_seconds};
use crate::error::Result;
use crate::firestore::FieldUpdate;
use crate::ids;
use crate::models::common::{Number, Timestamp};
use crate::models::sleep::{LastSleep, SleepDetails, SleepDocument, SleepInterval, SleepTimer};
use crate::models::{to_fields, to_json};
use crate::ops::feed::bottle::note;
use crate::paths;
use crate::rows::RowRef;

/// What a finished sleep amounts to.
#[derive(Debug, Clone, PartialEq)]
pub struct CompletedSleep {
    /// When it started, in seconds.
    pub start: i64,
    /// How long it lasted, in seconds.
    pub duration: i64,
    /// What was recorded about it.
    pub details: Option<SleepDetails>,
    /// The session identifier to leave on the timer.
    pub session_id: String,
}

/// Turns a running timer into the sleep it represents.
///
/// `None` when there is nothing to finish: no timer, an inactive one, or one
/// with no start time to measure from. A **paused** sleep ends at the moment
/// it was paused, not now, which is what makes pausing at midnight and
/// stopping at breakfast record a sleep that ended at midnight.
#[must_use]
pub fn completed_sleep(timer: &SleepTimer, now: f64) -> Option<CompletedSleep> {
    if !timer.active {
        return None;
    }
    // The app has written timers with no `timerStartTime`. Falling back to the
    // document's own timestamp recovers a plausible start instead of losing
    // the sleep outright.
    let start_seconds = timer
        .started_at()
        .or_else(|| timer.timestamp.map(|stamp| stamp.seconds.as_f64()))?;
    let end_seconds = if timer.paused {
        timer.paused_at().unwrap_or(now)
    } else {
        now
    };
    Some(CompletedSleep {
        start: start_seconds as i64,
        duration: (end_seconds - start_seconds) as i64,
        details: timer.details.clone(),
        session_id: timer.uuid.clone(),
    })
}

/// Whether a sleep just entered is the most recent one on the record.
///
/// A manual entry for last Tuesday must not become the app's "last sleep":
/// that field is what the home screen reads, and moving it backwards would
/// make the tool look like it had lost this morning's nap.
#[must_use]
pub fn replaces_last(existing_start: Option<f64>, new_start: f64) -> bool {
    existing_start.is_none_or(|existing| new_start > existing)
}

/// The timer fields that mark a session finished or abandoned.
///
/// The timer is emptied rather than deleted: the app expects the field to be
/// there and reads `active` off it.
#[must_use]
pub fn idle_timer(session_id: &str, now: f64) -> serde_json::Value {
    json!({
        "active": false,
        "paused": false,
        "timestamp": { "seconds": now },
        "timerStartTime": null,
        "uuid": session_id,
        "local_timestamp": now,
    })
}

impl Huckleberry {
    /// Starts a sleep.
    ///
    /// # Errors
    ///
    /// As every write: a refused or unreachable Firestore.
    pub async fn start_sleep(&self, cid: &str) -> Result<()> {
        let now = now_seconds();
        let document = SleepDocument {
            timer: Some(SleepTimer {
                active: true,
                paused: false,
                timestamp: Some(Timestamp::at(now)),
                local_timestamp: Some(Number::Float(now)),
                timer_start_time: Some(Number::Float(now * 1000.0)),
                timer_end_time: None,
                uuid: ids::session_id(),
                details: Some(SleepDetails::blank()),
                sws_analytics: None,
            }),
            prefs: None,
        };
        let token = self.token().await?;
        self.firestore()
            .merge(
                &token,
                &paths::tracker(paths::SLEEP, cid),
                &to_fields(&document)?,
                "starting a sleep",
            )
            .await
    }

    /// Pauses the running sleep.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::start_sleep`].
    pub async fn pause_sleep(&self, cid: &str) -> Result<TimerChange> {
        let Some(timer) = self.sleep_timer(cid).await? else {
            return Ok(TimerChange::NotRunning);
        };
        if timer.paused {
            return Ok(TimerChange::Unchanged);
        }
        let now = now_seconds();
        self.update_sleep(
            cid,
            &[
                FieldUpdate::set("timer.paused", json!(true)),
                FieldUpdate::set("timer.active", json!(true)),
                // The app reads this to show when a paused sleep stopped.
                FieldUpdate::set("timer.timerEndTime", json!(now * 1000.0)),
                FieldUpdate::set("timer.timestamp", json!({ "seconds": now })),
                FieldUpdate::set("timer.local_timestamp", json!(now)),
            ],
            "pausing the sleep",
        )
        .await
    }

    /// Resumes a paused sleep.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::start_sleep`].
    pub async fn resume_sleep(&self, cid: &str) -> Result<TimerChange> {
        let Some(timer) = self.sleep_timer(cid).await? else {
            return Ok(TimerChange::NotRunning);
        };
        if !timer.paused {
            return Ok(TimerChange::Unchanged);
        }
        let now = now_seconds();
        self.update_sleep(
            cid,
            &[
                FieldUpdate::set("timer.paused", json!(false)),
                FieldUpdate::set("timer.active", json!(true)),
                FieldUpdate::set("timer.timestamp", json!({ "seconds": now })),
                FieldUpdate::set("timer.local_timestamp", json!(now)),
            ],
            "resuming the sleep",
        )
        .await
    }

    /// Throws away the running sleep without recording it.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::start_sleep`].
    pub async fn cancel_sleep(&self, cid: &str) -> Result<TimerChange> {
        let existing: Option<SleepDocument> = self
            .document(&paths::tracker(paths::SLEEP, cid), "cancelling the sleep")
            .await?;
        let session_id = existing
            .and_then(|document| document.timer)
            .map_or_else(ids::session_id, |timer| timer.uuid);
        let now = now_seconds();
        self.update_sleep(
            cid,
            &[FieldUpdate::set("timer", idle_timer(&session_id, now))],
            "cancelling the sleep",
        )
        .await
    }

    /// Finishes the running sleep and writes it to history.
    ///
    /// `None` when no sleep was running, which makes stopping twice harmless.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::start_sleep`].
    pub async fn complete_sleep(&self, cid: &str) -> Result<Option<CompletedSleep>> {
        let Some(timer) = self.sleep_timer(cid).await? else {
            return Ok(None);
        };
        let now = now_seconds();
        let Some(completed) = completed_sleep(&timer, now) else {
            // A running timer with nothing to measure from: clear it rather
            // than leave a sleep that can never be finished.
            self.update_sleep(
                cid,
                &[FieldUpdate::set("timer", idle_timer(&timer.uuid, now))],
                "finishing the sleep",
            )
            .await?;
            return Ok(None);
        };

        let offset = self.zone().offset_minutes(now);
        let interval = SleepInterval {
            id: None,
            start: Number::Integer(completed.start),
            duration: Number::Integer(completed.duration),
            offset: Number::Float(offset),
            end_offset: Some(Number::Float(offset)),
            details: completed.details.clone(),
            last_updated: Some(Number::Float(now)),
        };
        let token = self.token().await?;
        self.firestore()
            .set(
                &token,
                &paths::history_row(paths::SLEEP, cid, &ids::sleep_interval_id()),
                &to_fields(&interval)?,
                "writing the sleep to history",
            )
            .await?;

        let last_sleep = LastSleep {
            start: Some(Number::Integer(completed.start)),
            duration: Some(Number::Integer(completed.duration)),
            offset: Some(Number::Float(offset)),
        };
        self.update_sleep(
            cid,
            &[
                FieldUpdate::set("timer", idle_timer(&completed.session_id, now)),
                FieldUpdate::set("prefs.lastSleep", to_json(&last_sleep)?),
                FieldUpdate::set("prefs.timestamp", json!({ "seconds": now })),
                FieldUpdate::set("prefs.local_timestamp", json!(now)),
            ],
            "finishing the sleep",
        )
        .await?;
        Ok(Some(completed))
    }

    /// Records a sleep that has already happened.
    ///
    /// History only: the timer is not read and not written, so a sleep in
    /// progress stays in progress. The tracker's "last sleep" is moved only
    /// when this one is more recent than what is there, because entering an
    /// older sleep after the fact is not a claim about the last one.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::start_sleep`].
    pub async fn log_sleep(&self, cid: &str, start: f64, duration: f64) -> Result<()> {
        let operation = "recording the sleep";
        let last = self
            .sleep_document(cid)
            .await?
            .and_then(|document| document.prefs)
            .and_then(|prefs| prefs.last_sleep)
            .and_then(|last| last.start)
            .map(Number::as_f64);

        let offset = self.zone().offset_minutes(start);
        let interval = SleepInterval {
            id: None,
            start: Number::Float(start),
            duration: Number::Float(duration),
            offset: Number::Float(offset),
            end_offset: Some(Number::Float(self.zone().offset_minutes(start + duration))),
            details: None,
            last_updated: Some(Number::Float(now_seconds())),
        };
        let token = self.token().await?;
        self.firestore()
            .set(
                &token,
                &paths::history_row(paths::SLEEP, cid, &ids::sleep_interval_id()),
                &to_fields(&interval)?,
                operation,
            )
            .await?;

        if !replaces_last(last, start) {
            return Ok(());
        }
        let now = now_seconds();
        let last_sleep = LastSleep {
            start: Some(Number::Float(start)),
            duration: Some(Number::Float(duration)),
            offset: Some(Number::Float(offset)),
        };
        self.firestore()
            .update(
                &token,
                &paths::tracker(paths::SLEEP, cid),
                &[
                    FieldUpdate::set("prefs.lastSleep", to_json(&last_sleep)?),
                    FieldUpdate::set("prefs.timestamp", json!({ "seconds": now })),
                    FieldUpdate::set("prefs.local_timestamp", json!(now)),
                ],
                operation,
            )
            .await
    }

    /// The sleep tracker's document.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::start_sleep`].
    pub async fn sleep_document(&self, cid: &str) -> Result<Option<SleepDocument>> {
        self.document(
            &paths::tracker(paths::SLEEP, cid),
            "reading the sleep timer",
        )
        .await
    }

    /// The running sleep timer, if a sleep is in progress.
    async fn sleep_timer(&self, cid: &str) -> Result<Option<SleepTimer>> {
        Ok(self
            .sleep_document(cid)
            .await?
            .and_then(|document| document.timer)
            .filter(|timer| timer.active))
    }

    async fn update_sleep(
        &self,
        cid: &str,
        updates: &[FieldUpdate],
        operation: &str,
    ) -> Result<TimerChange> {
        let token = self.token().await?;
        self.firestore()
            .update(
                &token,
                &paths::tracker(paths::SLEEP, cid),
                updates,
                operation,
            )
            .await?;
        Ok(TimerChange::Applied)
    }

    /// Changes a sleep that is already on the record.
    ///
    /// How long it lasted, and what was noted. Not when it began: the row
    /// keeps the moment it happened, and the note is written inside `details`
    /// one field at a time so that everything else recorded about the sleep
    /// (where it happened, how it started, how it ended) survives the edit.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::update_history_row`].
    pub async fn update_sleep_entry(
        &self,
        cid: &str,
        at: &RowRef,
        duration_seconds: f64,
        notes: Option<&str>,
    ) -> Result<()> {
        let updates = [
            FieldUpdate::set("duration", json!(duration_seconds)),
            FieldUpdate::set_or_clear("details.notes", note(notes)),
            FieldUpdate::set("lastUpdated", json!(now_seconds())),
        ];
        self.update_history_row(cid, at, &updates, "changing the sleep")
            .await
    }
}

#[cfg(test)]
mod last_sleep {
    use super::*;

    #[test]
    fn the_first_sleep_on_the_record_becomes_the_last_one() {
        assert!(replaces_last(None, 100.0));
    }

    #[test]
    fn a_newer_sleep_replaces_it_and_an_older_one_does_not() {
        assert!(replaces_last(Some(100.0), 200.0));
        assert!(!replaces_last(Some(200.0), 100.0));
        assert!(
            !replaces_last(Some(100.0), 100.0),
            "the same one is not newer"
        );
    }
}

#[cfg(test)]
mod completion {
    use super::*;

    fn timer(active: bool, paused: bool, start_ms: Option<f64>, end_ms: Option<f64>) -> SleepTimer {
        SleepTimer {
            active,
            paused,
            timestamp: None,
            local_timestamp: None,
            timer_start_time: start_ms.map(Number::Float),
            timer_end_time: end_ms.map(Number::Float),
            uuid: "0123456789abcdef".to_owned(),
            details: None,
            sws_analytics: None,
        }
    }

    #[test]
    fn a_running_sleep_lasts_until_now() {
        let completed = completed_sleep(&timer(true, false, Some(1_000_000.0), None), 4_600.0)
            .expect("a completed sleep");
        assert_eq!(completed.start, 1_000);
        assert_eq!(completed.duration, 3_600);
    }

    #[test]
    fn a_paused_sleep_ends_when_it_was_paused_not_now() {
        let completed = completed_sleep(
            &timer(true, true, Some(1_000_000.0), Some(2_800_000.0)),
            999_999.0,
        )
        .expect("a completed sleep");
        assert_eq!(completed.duration, 1_800);
    }

    #[test]
    fn a_paused_sleep_with_no_pause_time_falls_back_to_now() {
        let completed = completed_sleep(&timer(true, true, Some(1_000_000.0), None), 4_600.0)
            .expect("a completed sleep");
        assert_eq!(completed.duration, 3_600);
    }

    #[test]
    fn an_inactive_timer_completes_nothing() {
        assert!(completed_sleep(&timer(false, false, Some(1_000_000.0), None), 4_600.0).is_none());
    }

    #[test]
    fn a_timer_with_no_start_falls_back_to_the_documents_timestamp() {
        let mut bare = timer(true, false, None, None);
        bare.timestamp = Some(Timestamp::at(1_000.0));
        let completed = completed_sleep(&bare, 4_600.0).expect("a completed sleep");
        assert_eq!(completed.start, 1_000);
        assert_eq!(completed.duration, 3_600);
    }

    #[test]
    fn a_timer_with_nothing_to_measure_from_completes_nothing() {
        assert!(completed_sleep(&timer(true, false, None, None), 4_600.0).is_none());
    }

    #[test]
    fn the_session_id_is_carried_onto_the_idle_timer() {
        let idle = idle_timer("0123456789abcdef", 4_600.0);
        assert_eq!(idle["active"], json!(false));
        assert_eq!(idle["uuid"], json!("0123456789abcdef"));
        assert_eq!(idle["timerStartTime"], serde_json::Value::Null);
    }
}
