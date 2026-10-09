//! Pump details retain missing values and convert quantities when units change.
use anyhow::{Result, bail};
use huckleberry_api::models::pump::PumpEntryMode;

use crate::{cli::Units, domain::types::PumpEvent, render::format};

/// A pumping session in the units used by its edit form.
#[derive(Debug, Clone, PartialEq)]
pub struct PumpDraft {
    /// Whether amounts were recorded together or separately.
    pub mode: PumpEntryMode,
    /// Left quantity, or half the total, in `units`.
    pub left_amount: Option<f64>,
    /// Right quantity, or half the total, in `units`.
    pub right_amount: Option<f64>,
    /// Units of both stored quantities.
    pub units: Units,
    /// Optional session length, in minutes.
    pub duration_minutes: Option<f64>,
    /// Whatever the parent typed.
    pub notes: Option<String>,
}

impl PumpDraft {
    /// Builds a form without turning missing measurements into zero.
    pub(super) fn from_event(event: &PumpEvent, units: Units) -> Self {
        Self {
            mode: event.entry_mode.clone(),
            left_amount: event
                .left_ml
                .map(|amount| units.to_api().from_millilitres(amount)),
            right_amount: event
                .right_ml
                .map(|amount| units.to_api().from_millilitres(amount)),
            units,
            duration_minutes: event.duration_seconds.map(|seconds| seconds / 60.0),
            notes: event.notes.clone(),
        }
    }

    /// Both sides together, retaining an absent measurement as absent.
    #[must_use]
    pub fn total_amount(&self) -> Option<f64> {
        (self.left_amount.is_some() || self.right_amount.is_some())
            .then(|| self.left_amount.unwrap_or_default() + self.right_amount.unwrap_or_default())
    }

    /// The value a parent sees before deciding whether to change a field.
    #[must_use]
    pub fn current(&self, field: &str) -> String {
        match field {
            "mode" => match self.mode {
                PumpEntryMode::Total => "Total".into(),
                PumpEntryMode::LeftRight => "Left and right".into(),
                PumpEntryMode::Unknown(ref value) => value.clone(),
            },
            "amount" => self.volume(self.total_amount()),
            "left" => self.volume(self.left_amount),
            "right" => self.volume(self.right_amount),
            "units" => self.units.as_str().into(),
            "duration" => self.duration_minutes.map_or_else(
                || "not set".into(),
                |minutes| crate::domain::time::format_duration(minutes * 60.0),
            ),
            _ => self.notes.clone().unwrap_or_else(|| "not set".into()),
        }
    }

    /// Pump receipts distinguish a combined quantity from measured sides.
    pub(super) fn summary(&self) -> String {
        let mut parts = vec![if self.mode == PumpEntryMode::Total {
            self.volume(self.total_amount())
        } else {
            format!(
                "Left {} · Right {}",
                self.volume(self.left_amount),
                self.volume(self.right_amount)
            )
        }];
        if let Some(minutes) = self.duration_minutes {
            parts.push(crate::domain::time::format_duration(minutes * 60.0));
        }
        parts.join(" · ")
    }

    /// Validates a field before changing its value or its representation.
    pub(super) fn set(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "amount" => self.set_total(measurement(value, "amount")?),
            "left" | "right" => {
                let amount = measurement(value, "amount")?;
                if key == "left" {
                    self.left_amount = amount;
                } else {
                    self.right_amount = amount;
                }
                self.mode = PumpEntryMode::LeftRight;
            }
            "mode" => {
                let mode: PumpEntryMode = value.parse()?;
                if mode == PumpEntryMode::Total {
                    self.set_total(self.total_amount());
                }
                self.mode = mode;
            }
            "units" => self.set_units(value)?,
            "duration" => {
                let minutes = measurement(value, "duration in minutes")?;
                if minutes.is_some_and(|minutes| !(minutes * 60.0).is_finite()) {
                    bail!("duration is too large");
                }
                self.duration_minutes = minutes;
            }
            _ => self.notes = super::note(value),
        }
        Ok(())
    }

    fn set_total(&mut self, amount: Option<f64>) {
        self.left_amount = amount.map(|amount| amount / 2.0);
        self.right_amount = self.left_amount;
        self.mode = PumpEntryMode::Total;
    }

    fn set_units(&mut self, value: &str) -> Result<()> {
        let units = Units::from_stored(value)
            .ok_or_else(|| anyhow::anyhow!("`{value}` is not ml or oz"))?;
        self.left_amount = self
            .left_amount
            .map(|amount| format::convert(amount, self.units, units));
        self.right_amount = self
            .right_amount
            .map(|amount| format::convert(amount, self.units, units));
        self.units = units;
        Ok(())
    }

    fn volume(&self, amount: Option<f64>) -> String {
        amount.map_or_else(
            || "not set".into(),
            |amount| {
                format!(
                    "{} {}",
                    format::amount_in(amount, self.units),
                    self.units.as_str()
                )
            },
        )
    }
}

fn measurement(value: &str, field: &str) -> Result<Option<f64>> {
    if value.is_empty() {
        return Ok(None);
    }
    let amount = super::number(value, field)?;
    if !amount.is_finite() || amount < 0.0 {
        bail!("{field} must be a non-negative finite number");
    }
    Ok(Some(amount))
}
