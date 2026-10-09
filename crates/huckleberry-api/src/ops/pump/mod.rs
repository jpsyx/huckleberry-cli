//! Expressed milk and timer operations ported from Woyken's Python client.

mod history;
mod timer;

use crate::client::{Huckleberry, now_seconds};
use crate::error::{Error, Result};
use crate::models::feed::VolumeUnits;
use crate::models::pump::{LastPump, PumpAmounts, PumpDocument, PumpEntryMode};
use crate::paths;

/// Measurements for a completed pumping session. Duration is in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct PumpEntry<'a> {
    /// One combined amount or both side amounts.
    pub amounts: PumpAmounts,
    /// Session duration when known; absent when only an amount was recorded.
    pub duration: Option<f64>,
    /// Units shared by both amounts.
    pub units: VolumeUnits,
    /// An optional note; surrounding whitespace is removed.
    pub notes: Option<&'a str>,
}

/// The timing of the session saved by timer completion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompletedPump {
    /// Session start as Unix seconds.
    pub start: f64,
    /// Session duration in seconds, ending at the pause time when paused.
    pub duration: f64,
}

impl Huckleberry {
    /// Reads the pump tracker, including any active timer and last-session summary.
    ///
    /// # Errors
    /// A refused or unreachable Firestore, or an unreadable document.
    pub async fn pump_document(&self, cid: &str) -> Result<Option<PumpDocument>> {
        self.document(
            &paths::tracker(paths::PUMP, cid),
            "reading the pump tracker",
        )
        .await
    }

    /// Reads the last pump directly from the tracker summary.
    ///
    /// # Errors
    /// As [`Huckleberry::pump_document`].
    pub async fn latest_pump(&self, cid: &str) -> Result<Option<LastPump>> {
        Ok(self
            .pump_document(cid)
            .await?
            .and_then(|document| document.prefs)
            .and_then(|prefs| prefs.last_pump))
    }
}

fn validate_number(value: f64, field: &str) -> Result<()> {
    if !value.is_finite() || value < 0.0 {
        return Err(Error::Invalid(format!(
            "{field} must be a non-negative finite number"
        )));
    }
    Ok(())
}

fn resolve_amounts(amounts: PumpAmounts) -> Result<(PumpEntryMode, f64, f64)> {
    match amounts {
        PumpAmounts::Total(amount) => {
            validate_number(amount, "total amount")?;
            Ok((PumpEntryMode::Total, amount / 2.0, amount / 2.0))
        }
        PumpAmounts::LeftRight { left, right } => {
            validate_number(left, "left amount")?;
            validate_number(right, "right amount")?;
            Ok((PumpEntryMode::LeftRight, left, right))
        }
    }
}

fn validate_units(units: &VolumeUnits) -> Result<()> {
    if !units.is_known() {
        return Err(Error::Invalid("pump units must be ml or oz".into()));
    }
    Ok(())
}

fn validate_entry(entry: &PumpEntry<'_>) -> Result<(PumpEntryMode, f64, f64)> {
    validate_units(&entry.units)?;
    if let Some(duration) = entry.duration {
        validate_number(duration, "duration")?;
    }
    resolve_amounts(entry.amounts)
}

fn validate_time(at: f64, earliest: Option<f64>) -> Result<()> {
    super::timing::validate_time(at, earliest)?;
    if at > now_seconds() {
        return Err(Error::Invalid(
            "a pump event cannot be in the future".into(),
        ));
    }
    Ok(())
}
