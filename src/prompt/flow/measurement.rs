//! Numeric buffers carry their units, even while the text is unfinished.
use super::FormValue;
use crate::{
    cli::System,
    prompt::{Question, text},
    theme::Theme,
};
use anyhow::{Result, bail};

/// The physical dimension of a growth measurement.
#[derive(Clone, Copy)]
pub enum Dimension {
    /// A mass, stored as kilograms.
    Weight,
    /// A length, stored as centimetres.
    Length,
}
impl Dimension {
    /// Converts the selected units into canonical kg or cm.
    #[must_use]
    pub const fn factor(self, system: System) -> f64 {
        match (self, system) {
            (_, System::Metric) => 1.0,
            (Self::Weight, System::Imperial) => 0.453_592_37,
            (Self::Length, System::Imperial) => 2.54,
        }
    }
    const fn unit(self, system: System) -> &'static str {
        match (self, system) {
            (Self::Weight, System::Metric) => "kilograms",
            (Self::Weight, System::Imperial) => "pounds",
            (Self::Length, System::Metric) => "centimetres",
            (Self::Length, System::Imperial) => "inches",
        }
    }
}
/// An editable number whose stored physical quantity does not drift on conversion.
pub struct MeasurementState {
    pub(super) typed: String,
    system: System,
    dimension: Dimension,
    optional: bool,
    canonical: Option<f64>,
    displayed: String,
}
impl MeasurementState {
    /// Builds an input from its exact canonical quantity.
    #[must_use]
    pub fn create_measurement(
        value: Option<f64>,
        system: System,
        dimension: Dimension,
        optional: bool,
    ) -> Self {
        let typed = value.map_or_else(String::new, |value| {
            (value / dimension.factor(system)).to_string()
        });
        Self {
            displayed: typed.clone(),
            typed,
            system,
            dimension,
            optional,
            canonical: value,
        }
    }
    /// Reads the number in its original unit context, not the current form's label.
    pub fn value(&self) -> Result<FormValue> {
        if self.typed.trim().is_empty() && self.optional {
            return Ok(FormValue::Skipped);
        }
        if self.typed == self.displayed
            && let Some(value) = self.canonical
        {
            return Ok(FormValue::Number(value));
        }
        let number = self
            .typed
            .trim()
            .parse::<f64>()
            .map_err(|_| anyhow::anyhow!("`{}` is not a number", self.typed))?;
        if !number.is_finite() {
            bail!("choose a finite number");
        }
        Ok(FormValue::Number(
            number * self.dimension.factor(self.system),
        ))
    }
    pub(super) fn refresh(&mut self, fresh: &Self) {
        if self.system == fresh.system {
            return;
        }
        let Ok(value) = self.value() else {
            return;
        };
        let canonical = match value {
            FormValue::Number(value) => Some(value),
            FormValue::Skipped => None,
            _ => return,
        };
        *self = Self::create_measurement(canonical, fresh.system, self.dimension, self.optional);
    }
    pub(super) fn render(&self, label: &str, theme: Theme) -> Vec<String> {
        let label = format!(
            "{label} In {}.{}",
            self.dimension.unit(self.system),
            if self.optional {
                " Leave empty to skip."
            } else {
                ""
            }
        );
        text::lines(
            &Question::new("measurement", &label, ""),
            &self.typed,
            false,
            theme,
        )
    }
}
