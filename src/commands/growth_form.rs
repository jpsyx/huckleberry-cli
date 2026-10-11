//! Retained growth answers use kg and cm internally, including across unit changes.
use crate::{
    cli::System,
    domain::Calendar,
    prompt::flow::{
        FormModel, FormValue, View, ViewId, fields,
        measurement::{Dimension, MeasurementState},
    },
};
use anyhow::{Result, anyhow, bail};
use huckleberry_api::GrowthMeasurements;
/// Explicit growth values suppress only the questions they answer.
pub struct GrowthInput {
    /// Event time.
    pub at: Option<f64>,
    /// Measurements in the explicit or configured system.
    pub measurements: GrowthMeasurements,
    /// Explicit unit system.
    pub system: Option<System>,
}
/// One growth record in its chosen display units.
pub struct GrowthSubmission {
    /// Exact event time.
    pub at: f64,
    /// Measurements converted from their canonical quantities.
    pub measurements: GrowthMeasurements,
    /// The units used by the API payload.
    pub system: System,
}
/// A growth draft retains skipped measurements separately from defaults.
pub struct GrowthForm {
    calendar: Calendar,
    given: GrowthInput,
    at: Option<f64>,
    canonical: GrowthMeasurements,
    system: System,
}
impl GrowthForm {
    /// Creates a draft, converting explicit measurements only once.
    pub fn create_growth(calendar: Calendar, given: GrowthInput, default: System) -> Result<Self> {
        super::growth::validate(&given.measurements)?;
        let system = given.system.unwrap_or(default);
        let canonical = convert(&given.measurements, system, true);
        Ok(Self {
            calendar,
            at: given.at,
            given,
            canonical,
            system,
        })
    }
}
impl FormModel for GrowthForm {
    type Output = GrowthSubmission;
    fn route(&self) -> Vec<ViewId> {
        let mut route = Vec::new();
        if self.given.at.is_none() {
            route.push("at");
        }
        if self.given.system.is_none() {
            route.push("system");
        }
        if self.given.measurements == GrowthMeasurements::default() {
            route.push("weight");
        }
        if self.given.measurements.height.is_none() {
            route.push("height");
        }
        if self.given.measurements.head.is_none() {
            route.push("head");
        }
        route.into_iter().map(fields::create_id).collect()
    }
    fn create_view(&self, id: &ViewId, now: f64) -> Result<View> {
        Ok(match id.field {
            "at" => fields::create_time("When?", &self.calendar, self.at, now),
            "system" => fields::create_choices(
                "In which units?",
                &[
                    ("metric", "Kilograms and centimetres"),
                    ("imperial", "Pounds and inches"),
                ],
                usize::from(self.system == System::Imperial),
            ),
            field => {
                let (label, value, dimension, optional) = match field {
                    "weight" => ("Weight?", self.canonical.weight, Dimension::Weight, false),
                    "height" => ("Length?", self.canonical.height, Dimension::Length, true),
                    "head" => (
                        "Head circumference?",
                        self.canonical.head,
                        Dimension::Length,
                        true,
                    ),
                    _ => bail!("unknown growth field"),
                };
                View::create_measurement(
                    label,
                    MeasurementState::create_measurement(value, self.system, dimension, optional),
                )
            }
        })
    }
    fn accept(&mut self, id: &ViewId, value: FormValue, _: f64) -> Result<()> {
        match id.field {
            "at" => self.at = Some(fields::instant(&value)?),
            "system" => self.system = fields::parse_choice(value)?,
            field => {
                let number = match value {
                    FormValue::Number(value) => Some(value),
                    FormValue::Skipped => None,
                    _ => bail!("enter a number"),
                };
                let mut candidate = self.canonical.clone();
                match field {
                    "weight" => candidate.weight = number,
                    "height" => candidate.height = number,
                    "head" => candidate.head = number,
                    _ => bail!("unknown growth field"),
                }
                if field == "weight" && number.is_none() {
                    bail!("provide --weight <NUMBER>");
                }
                super::growth::validate(&convert(&candidate, self.system, false))?;
                self.canonical = candidate;
            }
        }
        Ok(())
    }
    fn complete(&self) -> Result<GrowthSubmission> {
        let measurements = convert(&self.canonical, self.system, false);
        if measurements == GrowthMeasurements::default() {
            bail!("provide --weight <NUMBER>");
        }
        super::growth::validate(&measurements)?;
        Ok(GrowthSubmission {
            at: self.at.ok_or_else(|| anyhow!("choose a time"))?,
            measurements,
            system: self.system,
        })
    }
}
fn convert(values: &GrowthMeasurements, system: System, to_canonical: bool) -> GrowthMeasurements {
    let amount = |value: Option<f64>, dimension: Dimension| {
        value.map(|value| {
            if to_canonical {
                value * dimension.factor(system)
            } else {
                value / dimension.factor(system)
            }
        })
    };
    GrowthMeasurements {
        weight: amount(values.weight, Dimension::Weight),
        height: amount(values.height, Dimension::Length),
        head: amount(values.head, Dimension::Length),
    }
}
