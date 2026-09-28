//! Event timestamps are independent of the time a write is synchronized.
use crate::client::Huckleberry;
use crate::error::{Error, Result};
use crate::paths;

/// Rejects invalid timestamps and transitions preceding the current timer segment.
pub fn validate_time(at: f64, earliest: Option<f64>) -> Result<()> {
    if !at.is_finite() || at < 0.0 {
        return Err(Error::Invalid(
            "event time must be a finite Unix timestamp".into(),
        ));
    }
    if earliest.is_some_and(|earliest| at < earliest) {
        return Err(Error::Invalid(
            "that time precedes the current timer segment; choose a later time".into(),
        ));
    }
    Ok(())
}

impl Huckleberry {
    /// Backfilling history must not move the home-screen summary backwards.
    pub(crate) async fn replaces_summary(
        &self,
        tracker: &str,
        cid: &str,
        field: &str,
        at: f64,
    ) -> Result<bool> {
        let document: Option<serde_json::Value> = self
            .document(&paths::tracker(tracker, cid), "reading the latest entry")
            .await?;
        let previous = document
            .as_ref()
            .and_then(|document| document.get("prefs"))
            .and_then(|prefs| prefs.get(field))
            .and_then(|entry| entry.get("start"))
            .and_then(serde_json::Value::as_f64);
        Ok(super::sleep::replaces_last(previous, at))
    }
}
