//! Correcting the start of an existing live sleep without completing it.

use serde_json::json;

use super::TimerChange;
use crate::client::{Huckleberry, now_seconds};
use crate::error::{Error, Result};
use crate::firestore::FieldUpdate;
use crate::models::sleep::{SleepDocument, SleepTimer};
use crate::paths;

impl Huckleberry {
    /// Changes only the start of the identified active sleep, in Unix seconds.
    /// Running or paused state, UUID, details, history and summaries are preserved.
    ///
    /// # Errors
    ///
    /// Invalid times, an inactive or replaced session, concurrent changes, or a refused write.
    pub async fn update_sleep_start(
        &self,
        cid: &str,
        session_id: &str,
        started: f64,
    ) -> Result<TimerChange> {
        super::timing::validate_time(started, None)?;
        let token = self.token().await?;
        let (timer, revision) = self.sleep_for_edit(&token, cid, session_id).await?;
        let now = now_seconds();
        validate_start(&timer, started, now)?;
        if timer
            .started_at()
            .is_some_and(|current| (current - started).abs() < f64::EPSILON)
        {
            return Ok(TimerChange::Unchanged);
        }
        self.firestore()
            .update_if_unchanged(
                &token,
                &paths::tracker(paths::SLEEP, cid),
                &[
                    FieldUpdate::set("timer.timerStartTime", json!(started * 1000.0)),
                    FieldUpdate::set("timer.timestamp", json!({"seconds": now})),
                    FieldUpdate::set("timer.local_timestamp", json!(now)),
                ],
                &revision,
                "changing the ongoing sleep start (if it changed, select it again)",
            )
            .await?;
        Ok(TimerChange::Applied)
    }

    /// Reads the session identity and server revision together, for one conditional patch.
    async fn sleep_for_edit(
        &self,
        token: &str,
        cid: &str,
        session_id: &str,
    ) -> Result<(SleepTimer, String)> {
        let operation = "reading the ongoing sleep for editing";
        let mut document = self
            .firestore()
            .get(token, &paths::tracker(paths::SLEEP, cid), operation)
            .await?
            .ok_or_else(|| Error::Invalid("no ongoing sleep to edit".into()))?;
        let revision = document.update_time.take().ok_or_else(|| {
            Error::Invalid("the sleep has no server revision; read it again before editing".into())
        })?;
        let sleep: SleepDocument =
            serde_json::from_value(document.into_json()).map_err(|error| Error::Decode {
                operation: operation.into(),
                detail: error.to_string(),
            })?;
        let timer = sleep.timer.filter(|timer| timer.active && timer.uuid == session_id)
            .ok_or_else(|| Error::Invalid("that sleep is no longer active or has been replaced; select the current sleep again".into()))?;
        Ok((timer, revision))
    }
}

/// A correction cannot create a future or negative-duration sleep.
fn validate_start(timer: &SleepTimer, started: f64, now: f64) -> Result<()> {
    let latest = if timer.paused {
        timer.paused_at().unwrap_or(now).min(now)
    } else {
        now
    };
    if started > latest {
        return Err(Error::Invalid(
            "a sleep cannot start in the future or after it was paused".into(),
        ));
    }
    Ok(())
}
