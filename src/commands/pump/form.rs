//! Pump drafts retain total and each side as distinct answers.
mod questions;
use crate::{
    cli::{PumpValues, Units},
    domain::Calendar,
    prompt::flow::{FormModel, FormValue, View, ViewId, fields},
    render::format,
};
use anyhow::{Result, anyhow, bail};
use huckleberry_api::models::pump::PumpAmounts;

/// A pump mutation, separate from its read-only status viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PumpOperation {
    /// Record a completed session.
    Log,
    /// Finish a running timer.
    Stop,
    /// Begin a timer.
    Start,
    /// Pause a timer.
    Pause,
    /// Continue a timer.
    Resume,
    /// Discard a timer.
    Cancel,
}
impl PumpOperation {
    const fn records(self) -> bool {
        matches!(self, Self::Log | Self::Stop)
    }
}
/// Explicit arguments determine the questions left to collect.
pub struct PumpInput {
    /// The requested mutation.
    pub operation: PumpOperation,
    /// Resolved event time, absent for cancel.
    pub at: Option<f64>,
    /// Explicit duration in seconds.
    pub duration: Option<f64>,
    /// Explicit quantities and note.
    pub values: PumpValues,
}
/// Suggested quantities, all expressed in the offered units.
pub struct PumpDefaults {
    /// Default dial units.
    pub units: Units,
    /// Suggested combined amount.
    pub total: Option<f64>,
    /// Suggested left amount.
    pub left: Option<f64>,
    /// Suggested right amount.
    pub right: Option<f64>,
}
/// The applicable answers consumed by one pump mutation.
pub struct PumpSubmission {
    /// The selected operation.
    pub operation: PumpOperation,
    /// Exact accepted event time.
    pub at: Option<f64>,
    /// Only the selected quantity mode, absent for timer-only operations.
    pub amounts: Option<PumpAmounts>,
    /// Common units in which quantities are recorded.
    pub units: Units,
    /// Manual duration in seconds, absent when omitted or stopping a timer.
    pub duration: Option<f64>,
    /// Optional note.
    pub notes: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Total,
    Sides,
}
#[derive(Clone, Copy)]
enum Duration {
    Value(Option<f64>),
    Custom,
}
type Quantity = (Units, f64);

/// Typed answers remain present when another mode hides them.
pub struct PumpForm {
    calendar: Calendar,
    given: PumpInput,
    defaults: PumpDefaults,
    at: Option<f64>,
    mode: Option<Mode>,
    total: Option<Quantity>,
    left: Option<Quantity>,
    right: Option<Quantity>,
    duration: Duration,
    custom_seconds: Option<f64>,
    notes: Option<String>,
}
impl PumpForm {
    /// Creates a draft without reading the clock, prompting, or fetching defaults.
    pub fn create_pump(
        calendar: Calendar,
        given: PumpInput,
        defaults: PumpDefaults,
    ) -> Result<Self> {
        for amount in [given.values.amount, given.values.left, given.values.right]
            .into_iter()
            .flatten()
        {
            validate_amount(amount)?;
        }
        let units = given.values.units.unwrap_or(defaults.units);
        let mode = if given.values.amount.is_some() {
            Some(Mode::Total)
        } else if given.values.left.is_some() || given.values.right.is_some() {
            Some(Mode::Sides)
        } else {
            None
        };
        Ok(Self {
            calendar,
            at: given.at,
            mode,
            total: given.values.amount.map(|amount| (units, amount)),
            left: given.values.left.map(|amount| (units, amount)),
            right: given.values.right.map(|amount| (units, amount)),
            duration: Duration::Value(given.duration),
            custom_seconds: None,
            notes: given.values.notes.clone(),
            given,
            defaults,
        })
    }
    fn quantity_fields(&self, route: &mut Vec<&'static str>) {
        if self.given.values.amount.is_none()
            && self.given.values.left.is_none()
            && self.given.values.right.is_none()
        {
            route.push("mode");
        }
        if self.mode == Some(Mode::Sides) {
            if self.given.values.left.is_none() {
                route.push("left");
            }
            if self.given.values.right.is_none() {
                route.push("right");
            }
        } else if self.given.values.amount.is_none() {
            route.push("total");
        }
    }
    fn volumes(&self) -> Result<(Option<PumpAmounts>, Units)> {
        let configured = self.given.values.units.unwrap_or(self.defaults.units);
        if !self.given.operation.records() {
            return Ok((None, configured));
        }
        match self.mode.ok_or_else(|| {
            anyhow!("provide --amount <NUMBER> or --left <NUMBER> --right <NUMBER>")
        })? {
            Mode::Total => {
                let (chosen, amount) = self
                    .total
                    .ok_or_else(|| anyhow!("provide --amount <NUMBER>"))?;
                let units = self.given.values.units.unwrap_or(chosen);
                Ok((
                    Some(PumpAmounts::Total(format::convert(amount, chosen, units))),
                    units,
                ))
            }
            Mode::Sides => {
                let (left_units, left) = self
                    .left
                    .ok_or_else(|| anyhow!("provide --left <NUMBER>"))?;
                let (right_units, right) = self
                    .right
                    .ok_or_else(|| anyhow!("provide --right <NUMBER>"))?;
                let units = self.given.values.units.unwrap_or(left_units);
                Ok((
                    Some(PumpAmounts::LeftRight {
                        left: format::convert(left, left_units, units),
                        right: format::convert(right, right_units, units),
                    }),
                    units,
                ))
            }
        }
    }
}
impl FormModel for PumpForm {
    type Output = PumpSubmission;
    fn route(&self) -> Vec<ViewId> {
        let mut route = Vec::new();
        if self.given.operation != PumpOperation::Cancel && self.given.at.is_none() {
            route.push("at");
        }
        if self.given.operation.records() {
            self.quantity_fields(&mut route);
            if self.given.operation == PumpOperation::Log && self.given.duration.is_none() {
                route.push("duration");
                if matches!(self.duration, Duration::Custom) {
                    route.push("custom_duration");
                }
            }
            if self.given.values.notes.is_none() {
                route.push("notes");
            }
        }
        route.into_iter().map(fields::create_id).collect()
    }
    fn create_view(&self, id: &ViewId, now: f64) -> Result<View> {
        questions::create_view(self, id.field, now)
    }
    fn accept(&mut self, id: &ViewId, value: FormValue, _: f64) -> Result<()> {
        match id.field {
            "at" => self.at = Some(fields::instant(&value)?),
            "mode" => {
                let FormValue::Choice(value) = value else {
                    bail!("choose total or sides");
                };
                self.mode = Some(if value == "leftright" {
                    Mode::Sides
                } else {
                    Mode::Total
                });
            }
            "total" | "left" | "right" => self.accept_quantity(id.field, &value)?,
            "duration" => self.accept_duration(value)?,
            "custom_duration" => {
                self.custom_seconds = fields::optional_text(value)?
                    .map(|text| super::prompts::parse_duration(&text))
                    .transpose()?;
            }
            "notes" => self.notes = fields::optional_text(value)?,
            _ => bail!("unknown pump field"),
        }
        Ok(())
    }
    fn complete(&self) -> Result<PumpSubmission> {
        if self.given.operation != PumpOperation::Cancel && self.at.is_none() {
            bail!("choose when this happened");
        }
        let (amounts, units) = self.volumes()?;
        Ok(PumpSubmission {
            operation: self.given.operation,
            at: self.at,
            amounts,
            units,
            duration: match self.duration {
                Duration::Value(value) => value,
                Duration::Custom => self.custom_seconds,
            },
            notes: self.notes.clone(),
        })
    }
}
impl PumpForm {
    fn accept_quantity(&mut self, field: &str, value: &FormValue) -> Result<()> {
        let FormValue::Volume { units, amount } = value else {
            bail!("choose a quantity");
        };
        validate_amount(*amount)?;
        let quantity = Some((*units, *amount));
        match field {
            "total" => self.total = quantity,
            "left" => self.left = quantity,
            _ => self.right = quantity,
        }
        Ok(())
    }
    fn accept_duration(&mut self, value: FormValue) -> Result<()> {
        let FormValue::Choice(value) = value else {
            bail!("choose a duration");
        };
        self.duration = match value.as_str() {
            "skip" => Duration::Value(None),
            "custom" => Duration::Custom,
            _ => Duration::Value(Some(super::prompts::parse_duration(&value)?)),
        };
        Ok(())
    }
}
fn validate_amount(amount: f64) -> Result<()> {
    if !amount.is_finite() || amount < 0.0 {
        bail!("pump amount must be a finite number at least zero");
    }
    Ok(())
}
