//! Carry recorded details through completion without narrowing them to modeled fields.
use super::sleep::CompletedSleep;
use crate::{
    Huckleberry,
    error::{Error, Result},
    firestore::client::decode_document,
    ids,
    models::{
        common::Number,
        sleep::{SleepDocument, SleepInterval, SleepTimer},
        to_fields,
    },
    paths,
};
use serde_json::Value as Json;

impl Huckleberry {
    /// Read typed timer arithmetic and original tagged details from the same response.
    pub(super) async fn sleep_for_completion(
        &self,
        cid: &str,
    ) -> Result<Option<(SleepTimer, Option<Json>)>> {
        let operation = "reading the sleep timer";
        let token = self.token().await?;
        let Some(raw) = self
            .firestore()
            .get_raw(&token, &paths::tracker(paths::SLEEP, cid), operation)
            .await?
        else {
            return Ok(None);
        };
        let document: SleepDocument = serde_json::from_value(decode_document(&raw).into_json())
            .map_err(|error| Error::decode(operation, error))?;
        let details = raw
            .pointer("/fields/timer/mapValue/fields/details")
            .cloned();
        Ok(document
            .timer
            .filter(|timer| timer.active)
            .map(|timer| (timer, details)))
    }

    /// Save the measured interval with the raw details, including unknown nested fields.
    pub(super) async fn write_completed_sleep(
        &self,
        cid: &str,
        completed: &CompletedSleep,
        details: Option<Json>,
        now: f64,
    ) -> Result<f64> {
        let offset = self.zone().offset_minutes(completed.start as f64);
        let interval = SleepInterval {
            id: None,
            start: Number::Integer(completed.start),
            duration: Number::Integer(completed.duration),
            offset: Number::Float(offset),
            end_offset: Some(Number::Float(
                self.zone()
                    .offset_minutes((completed.start + completed.duration) as f64),
            )),
            details: completed.details.clone(),
            last_updated: Some(Number::Float(now)),
        };
        let mut fields = to_fields(&interval)?;
        if let Some(details) = details {
            fields.insert("details".into(), details);
        }
        let token = self.token().await?;
        self.firestore()
            .set(
                &token,
                &paths::history_row(paths::SLEEP, cid, &ids::sleep_interval_id()),
                &fields,
                "writing the sleep to history",
            )
            .await?;
        Ok(offset)
    }
}
