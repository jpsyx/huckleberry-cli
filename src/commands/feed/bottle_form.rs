//! A bottle draft keeps the physical quantity apart from its displayed units.
use crate::{
    cli::{BottleKind, Units},
    domain::Calendar,
    prompt::flow::{FormModel, FormValue, View, ViewId, fields, volume::VolumeState},
};
use anyhow::{Result, anyhow, bail};

/// Explicit flags, each of which answers only its own question.
#[derive(Default)]
pub struct BottleInput {
    /// Resolved event time.
    pub at: Option<f64>,
    /// Amount in the explicitly supplied or chosen units.
    pub amount: Option<f64>,
    /// Explicit volume units.
    pub units: Option<Units>,
    /// Milk kind.
    pub kind: Option<BottleKind>,
    /// Explicit note, including an empty one.
    pub notes: Option<String>,
}
/// Suggestions read once before collecting answers.
pub struct BottleDefaults {
    /// Units offered for a new bottle.
    pub units: Units,
    /// Suggested amount in the offered units.
    pub amount: Option<f64>,
    /// Last known milk kind, or the usual first choice.
    pub kind: BottleKind,
}
/// A complete bottle, ready for the existing API operation.
pub struct BottleSubmission {
    /// Accepted instant.
    pub at: f64,
    /// Accepted quantity.
    pub amount: f64,
    /// Units in which to record the quantity.
    pub units: Units,
    /// Milk kind.
    pub kind: BottleKind,
    /// Optional note.
    pub notes: Option<String>,
}
/// Typed answers and the fixed set of unanswered flags.
pub struct BottleForm {
    calendar: Calendar,
    route: Vec<ViewId>,
    answer: BottleInput,
    defaults: BottleDefaults,
}
impl BottleForm {
    /// Opens a draft with suggestions distinct from accepted answers.
    pub fn create_bottle(
        calendar: Calendar,
        input: BottleInput,
        defaults: BottleDefaults,
    ) -> Result<Self> {
        if let Some(amount) = input.amount {
            validate_amount(amount)?;
        }
        let mut route = Vec::new();
        if input.at.is_none() {
            route.push(fields::create_id("at"));
        }
        match (input.amount, input.units) {
            (None, None) => route.push(fields::create_id("volume")),
            (None, Some(_)) => route.push(fields::create_id("amount")),
            (Some(_), None) => route.push(fields::create_id("units")),
            _ => {}
        }
        if input.kind.is_none() {
            route.push(fields::create_id("kind"));
        }
        if input.notes.is_none() {
            route.push(fields::create_id("notes"));
        }
        Ok(Self {
            calendar,
            route,
            answer: input,
            defaults,
        })
    }
}
impl FormModel for BottleForm {
    type Output = BottleSubmission;
    fn route(&self) -> Vec<ViewId> {
        self.route.clone()
    }
    fn create_view(&self, id: &ViewId, now: f64) -> Result<View> {
        Ok(match id.field {
            "at" => fields::create_time("When?", &self.calendar, self.answer.at, now),
            "volume" => View::create_volume(
                "How much?",
                VolumeState::create_volume(self.defaults.units, self.defaults.amount, 1.0),
            ),
            "units" => fields::create_choices(
                "In which units?",
                &[("ml", "Millilitres"), ("oz", "Fluid ounces")],
                usize::from(self.defaults.units == Units::Oz),
            ),
            "amount" => View::create_text(
                format!(
                    "How much, in {}?",
                    self.answer.units.unwrap_or(self.defaults.units).as_str()
                ),
                &self
                    .defaults
                    .amount
                    .map_or_else(String::new, |amount| amount.to_string()),
                false,
            ),
            "kind" => fields::create_choices(
                "What was in it?",
                &[
                    ("formula", "Formula"),
                    ("breast-milk", "Expressed breast milk"),
                    ("cow-milk", "Cow milk"),
                    ("other", "Something else"),
                ],
                kind_index(self.defaults.kind),
            ),
            "notes" => View::create_text("Anything to note?", "", false),
            _ => bail!("unknown bottle field"),
        })
    }
    fn accept(&mut self, id: &ViewId, value: FormValue, _: f64) -> Result<()> {
        match id.field {
            "at" => self.answer.at = Some(fields::instant(&value)?),
            "volume" => {
                let FormValue::Volume { units, amount } = value else {
                    bail!("choose an amount");
                };
                validate_amount(amount)?;
                self.answer.units = Some(units);
                self.answer.amount = Some(amount);
            }
            "units" => self.answer.units = Some(fields::parse_choice(value)?),
            "amount" => {
                let text = fields::text(value)?;
                let amount = text
                    .trim()
                    .parse()
                    .map_err(|_| anyhow!("`{text}` is not a number"))?;
                validate_amount(amount)?;
                self.answer.amount = Some(amount);
            }
            "kind" => self.answer.kind = Some(fields::parse_choice(value)?),
            "notes" => self.answer.notes = fields::optional_text(value)?,
            _ => bail!("unknown bottle field"),
        }
        Ok(())
    }
    fn complete(&self) -> Result<BottleSubmission> {
        Ok(BottleSubmission {
            at: self
                .answer
                .at
                .ok_or_else(|| anyhow!("choose when this happened"))?,
            units: self
                .answer
                .units
                .ok_or_else(|| anyhow!("provide --units <UNITS>"))?,
            amount: self
                .answer
                .amount
                .ok_or_else(|| anyhow!("provide --amount <NUMBER>"))?,
            kind: self
                .answer
                .kind
                .ok_or_else(|| anyhow!("provide --type <KIND>"))?,
            notes: self.answer.notes.clone(),
        })
    }
}
fn validate_amount(amount: f64) -> Result<()> {
    if !amount.is_finite() || amount <= 0.0 {
        bail!("a bottle needs an amount greater than zero, not {amount}");
    }
    Ok(())
}
const fn kind_index(kind: BottleKind) -> usize {
    match kind {
        BottleKind::Formula
        | BottleKind::GoatMilk
        | BottleKind::SoyMilk
        | BottleKind::TubeFeeding => 0,
        BottleKind::BreastMilk => 1,
        BottleKind::CowMilk => 2,
        BottleKind::Other => 3,
    }
}
