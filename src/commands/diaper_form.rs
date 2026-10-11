//! Typed diaper and potty drafts retain answers independently of their route.
mod questions;

use crate::{
    cli::{
        Amount, Colour, Consistency, DiaperKind, PottyOutcome,
        actions::{DiaperFlags, DiaperQuestion, unanswered},
    },
    domain::Calendar,
    prompt::flow::{FormModel, FormValue, View, ViewId},
};
use anyhow::{Result, anyhow};
use clap::ValueEnum;
use huckleberry_api::DiaperDetails;

/// Explicit arguments determine which questions still need asking.
pub struct DiaperInput {
    /// An explicitly resolved timestamp.
    pub at: Option<f64>,
    /// Contents, when supplied by a flag.
    pub mode: Option<DiaperKind>,
    /// Optional details supplied by flags.
    pub flags: DiaperFlags,
    /// An explicit note, including an intentionally empty note.
    pub notes: Option<String>,
    /// Whether this is a potty trip.
    pub potty: bool,
    /// The outcome supplied for a potty trip.
    pub outcome: Option<PottyOutcome>,
}
impl Default for DiaperInput {
    fn default() -> Self {
        Self {
            at: None,
            mode: None,
            flags: DiaperFlags {
                pee: None,
                poo: None,
                color: None,
                consistency: None,
                rash: false,
            },
            notes: None,
            potty: false,
            outcome: None,
        }
    }
}

/// Validated inputs consumed by the shared recording operation.
pub struct DiaperSubmission {
    /// The accepted instant, preserved when revisiting the form.
    pub at: f64,
    /// The selected contents.
    pub mode: DiaperKind,
    /// Applicable details only, plus explicit flags.
    pub details: DiaperDetails,
    /// A potty outcome, or None for a diaper change.
    pub potty: Option<PottyOutcome>,
}

/// Stores typed answers while the controller retains editable controls.
pub struct DiaperForm {
    calendar: Calendar,
    given: DiaperInput,
    at: Option<f64>,
    mode: Option<DiaperKind>,
    outcome: Option<PottyOutcome>,
    details: DiaperDetails,
}

impl DiaperForm {
    /// Seeds a draft from explicit flags without asking or reading the clock.
    #[must_use]
    pub fn create_diaper(calendar: Calendar, given: DiaperInput) -> Self {
        let details = DiaperDetails {
            pee_amount: given.flags.pee.map(Amount::to_api),
            poo_amount: given.flags.poo.map(Amount::to_api),
            color: given.flags.color.map(Colour::to_api),
            consistency: given.flags.consistency.map(Consistency::to_api),
            rash: given.flags.rash,
            notes: given.notes.clone(),
        };
        Self {
            calendar,
            at: given.at,
            mode: given.mode,
            outcome: given.outcome,
            details,
            given,
        }
    }

    fn applicable_details(&self, mode: DiaperKind) -> DiaperDetails {
        let mut details = self.details.clone();
        if !matches!(mode, DiaperKind::Pee | DiaperKind::Both) {
            details.pee_amount = self.given.flags.pee.map(Amount::to_api);
        }
        if !DiaperFlags::describes_dirt(mode) {
            details.poo_amount = self.given.flags.poo.map(Amount::to_api);
            details.color = self.given.flags.color.map(Colour::to_api);
            details.consistency = self.given.flags.consistency.map(Consistency::to_api);
        }
        details.rash &= !self.given.potty;
        details
    }
}

impl FormModel for DiaperForm {
    type Output = DiaperSubmission;
    fn route(&self) -> Vec<ViewId> {
        let mut fields = Vec::new();
        if self.given.at.is_none() {
            fields.push("at");
        }
        if self.given.mode.is_none() {
            fields.push("mode");
        }
        if self.given.potty && self.given.outcome.is_none() {
            fields.push("outcome");
        }
        fields.extend(
            unanswered(
                self.mode.unwrap_or(DiaperKind::Pee),
                &self.given.flags,
                self.given.notes.as_deref(),
            )
            .into_iter()
            .filter(|detail| !self.given.potty || *detail != DiaperQuestion::Rash)
            .map(field),
        );
        fields
            .into_iter()
            .map(|field| ViewId { field, owner: None })
            .collect()
    }
    fn create_view(&self, id: &ViewId, now: f64) -> Result<View> {
        questions::create_view(
            id.field,
            self.given.potty,
            &self.calendar,
            self.at.unwrap_or(now),
            now,
        )
    }
    fn accept(&mut self, id: &ViewId, value: FormValue, _: f64) -> Result<()> {
        match (id.field, value) {
            ("at", FormValue::Instant(at)) => self.at = Some(at),
            ("mode", FormValue::Choice(value)) => self.mode = Some(parse(&value)?),
            ("outcome", FormValue::Choice(value)) => self.outcome = Some(parse(&value)?),
            ("notes", FormValue::Text(value)) => {
                self.details.notes = (!value.trim().is_empty()).then(|| value.trim().to_owned());
            }
            ("rash", FormValue::Choice(value)) => self.details.rash = value == "yes",
            (field, value) => self.accept_detail(field, value)?,
        }
        Ok(())
    }
    fn forward(&self, id: &ViewId) -> Option<ViewId> {
        let route = self.route();
        route
            .iter()
            .position(|view| view == id)
            .and_then(|index| route.get(index + 1))
            .cloned()
    }
    fn complete(&self) -> Result<DiaperSubmission> {
        let mode = self.mode.ok_or_else(|| anyhow!("provide --mode <MODE>"))?;
        let potty = if self.given.potty {
            Some(
                self.outcome
                    .ok_or_else(|| anyhow!("provide --how <OUTCOME>"))?,
            )
        } else {
            None
        };
        Ok(DiaperSubmission {
            at: self
                .at
                .ok_or_else(|| anyhow!("choose when this happened"))?,
            mode,
            details: self.applicable_details(mode),
            potty,
        })
    }
}

impl DiaperForm {
    fn accept_detail(&mut self, field: &str, value: FormValue) -> Result<()> {
        let value = match value {
            FormValue::Skipped => None,
            FormValue::Choice(value) => Some(value),
            _ => return Err(anyhow!("invalid diaper detail")),
        };
        match field {
            "pee" => {
                self.details.pee_amount = value
                    .as_deref()
                    .map(parse::<Amount>)
                    .transpose()?
                    .map(Amount::to_api);
            }
            "poo" => {
                self.details.poo_amount = value
                    .as_deref()
                    .map(parse::<Amount>)
                    .transpose()?
                    .map(Amount::to_api);
            }
            "colour" => {
                self.details.color = value
                    .as_deref()
                    .map(parse::<Colour>)
                    .transpose()?
                    .map(Colour::to_api);
            }
            "consistency" => {
                self.details.consistency = value
                    .as_deref()
                    .map(parse::<Consistency>)
                    .transpose()?
                    .map(Consistency::to_api);
            }
            _ => return Err(anyhow!("unknown diaper field: {field}")),
        }
        Ok(())
    }
}

fn parse<Value: ValueEnum>(value: &str) -> Result<Value> {
    Value::from_str(value, true).map_err(anyhow::Error::msg)
}
const fn field(detail: DiaperQuestion) -> &'static str {
    match detail {
        DiaperQuestion::PeeAmount => "pee",
        DiaperQuestion::PooAmount => "poo",
        DiaperQuestion::Colour => "colour",
        DiaperQuestion::Consistency => "consistency",
        DiaperQuestion::Rash => "rash",
        DiaperQuestion::Notes => "notes",
    }
}
