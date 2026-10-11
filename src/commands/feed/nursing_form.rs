//! Nursing transitions retain an exact event time and any selected side.
use crate::{
    cli::Side,
    domain::Calendar,
    prompt::flow::{FormModel, FormValue, View, ViewId, fields},
};
use anyhow::{Result, anyhow, bail};

/// A nursing mutation, excluding its read-only status view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NursingOperation {
    /// Begin a timer.
    Start,
    /// Bank the running side.
    Pause,
    /// Continue a paused timer.
    Resume,
    /// Bank and change sides.
    Switch,
    /// Finish and record the timer.
    Stop,
    /// Discard the timer without recording it.
    Cancel,
}
/// Explicit nursing arguments before collecting missing values.
pub struct NursingInput {
    /// Transition requested by the command.
    pub operation: NursingOperation,
    /// An explicit resolved time, absent for cancel.
    pub at: Option<f64>,
    /// Explicit side, or none to resume the previous side.
    pub side: Option<Side>,
}
/// Validated input to a single nursing operation.
pub type NursingSubmission = NursingInput;

/// A nursing draft and the questions its original flags left unanswered.
pub struct NursingForm {
    calendar: Calendar,
    answer: NursingInput,
    suggested: Side,
    route: Vec<ViewId>,
}
impl NursingForm {
    /// Seeds a form with a side suggestion read once during preparation.
    #[must_use]
    pub fn create_nursing(calendar: Calendar, input: NursingInput, suggested: Side) -> Self {
        let mut route = Vec::new();
        if input.operation != NursingOperation::Cancel && input.at.is_none() {
            route.push(fields::create_id("at"));
        }
        if input.operation == NursingOperation::Start && input.side.is_none() {
            route.push(fields::create_id("side"));
        }
        Self {
            calendar,
            answer: input,
            suggested,
            route,
        }
    }
}
impl FormModel for NursingForm {
    type Output = NursingSubmission;
    fn route(&self) -> Vec<ViewId> {
        self.route.clone()
    }
    fn create_view(&self, id: &ViewId, now: f64) -> Result<View> {
        Ok(match id.field {
            "at" => fields::create_time(
                if self.answer.operation == NursingOperation::Start {
                    "When did it start?"
                } else {
                    "When?"
                },
                &self.calendar,
                self.answer.at,
                now,
            ),
            "side" => fields::create_choices(
                "Which side?",
                &[("left", "The left side"), ("right", "The right side")],
                usize::from(self.suggested == Side::Right),
            ),
            _ => bail!("unknown nursing field"),
        })
    }
    fn accept(&mut self, id: &ViewId, value: FormValue, _: f64) -> Result<()> {
        match id.field {
            "at" => self.answer.at = Some(fields::instant(&value)?),
            "side" => self.answer.side = Some(fields::parse_choice(value)?),
            _ => bail!("unknown nursing field"),
        }
        Ok(())
    }
    fn complete(&self) -> Result<NursingSubmission> {
        if self.answer.operation != NursingOperation::Cancel && self.answer.at.is_none() {
            bail!("choose when this happened");
        }
        let side = if self.answer.operation == NursingOperation::Start {
            Some(
                self.answer
                    .side
                    .ok_or_else(|| anyhow!("provide --side <SIDE>"))?,
            )
        } else {
            self.answer.side
        };
        Ok(NursingSubmission {
            operation: self.answer.operation,
            at: self.answer.at,
            side,
        })
    }
}
