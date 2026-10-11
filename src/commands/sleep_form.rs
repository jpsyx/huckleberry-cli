//! Sleep questions retain exact instants before any timer mutation occurs.
use crate::{
    cli::Overlap,
    domain::{Calendar, clock},
    prompt::{
        self,
        dial::time::{Mode, Opening},
        flow::{FormModel, FormValue, View, ViewId, fields, time::TimeState},
    },
    render::{format, output},
    theme::Tone,
};
use anyhow::{Result, anyhow, bail};
use huckleberry_api::models::SleepTimer;

/// A sleep mutation, excluding the status viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepOperation {
    /// Begin a live timer.
    Start,
    /// Record a past interval.
    Manual,
    /// Pause a timer.
    Pause,
    /// Continue a timer.
    Resume,
    /// Finish a timer.
    Stop,
    /// Discard a timer.
    Cancel,
}
/// Explicit values and the manual-end flag whose date may depend on a missing start.
pub struct SleepInput {
    /// The requested action.
    pub operation: SleepOperation,
    /// Explicit start or transition time.
    pub start: Option<f64>,
    /// Explicit resolved manual end.
    pub end: Option<f64>,
    /// An end flag waiting for the first start answer.
    pub given_end: Option<String>,
    /// Explicit overlap decision.
    pub overlap: Option<Overlap>,
}
/// Validated values consumed by one sleep operation.
pub struct SleepSubmission {
    /// The requested mutation.
    pub operation: SleepOperation,
    /// Accepted start or transition time.
    pub start: Option<f64>,
    /// Accepted manual end.
    pub end: Option<f64>,
    /// Applicable overlap decision only.
    pub overlap: Option<Overlap>,
    /// Live state that the overlap question described.
    pub running: Option<SleepTimer>,
}
/// A draft whose route depends on its accepted interval and the prepared live state.
pub struct SleepForm {
    calendar: Calendar,
    given: SleepInput,
    start: Option<f64>,
    end: Option<f64>,
    overlap: Option<Overlap>,
    running: Option<SleepTimer>,
    now: f64,
}
impl SleepForm {
    /// Builds a draft from prepared context without performing any I/O.
    pub fn create_sleep(
        calendar: Calendar,
        given: SleepInput,
        running: Option<SleepTimer>,
        now: f64,
    ) -> Result<Self> {
        if let (Some(start), Some(end)) = (given.start, given.end) {
            validate_interval(&calendar, start, end, now)?;
        }
        Ok(Self {
            calendar,
            start: given.start,
            end: given.end,
            overlap: given.overlap,
            given,
            running,
            now,
        })
    }
    fn overlaps(&self) -> bool {
        let (Some(start), Some(end), Some(timer)) = (self.start, self.end, self.running.as_ref())
        else {
            return false;
        };
        let Some(running_start) = timer.started_at() else {
            return false;
        };
        let running_end = if timer.paused {
            timer.paused_at().unwrap_or(self.now)
        } else {
            self.now
        };
        timer.active && clock::overlaps(start, end, running_start, running_end)
    }
    fn describe(&self, id: &ViewId, view: View, now: f64) -> View {
        if id.field == "overlap" {
            let start = self
                .running
                .as_ref()
                .and_then(SleepTimer::started_at)
                .expect("overlapping timer");
            return view.with_description(
                format!(
                    "That overlaps the sleep in progress, which started at {} and has run {}.",
                    format::clock(start, &self.calendar),
                    crate::domain::time::format_duration(now - start)
                ),
                Tone::Attention,
            );
        }
        if self.given.operation == SleepOperation::Stop
            && let Some(message) =
                output::sleep_start_context(self.running.as_ref(), now, &self.calendar)
        {
            return view.with_description(message, Tone::Muted);
        }
        view
    }
    fn accept_start(&mut self, value: &FormValue, now: f64) -> Result<()> {
        let start = fields::instant(value)?;
        let end = if self.end.is_none() {
            self.given
                .given_end
                .as_deref()
                .map(|text| {
                    prompt::time::parse_explicit(&self.calendar, text, now, |time, _, calendar| {
                        clock::first_after(time, start, calendar)
                    })
                })
                .transpose()?
        } else {
            self.end
        };
        self.start = Some(start);
        self.end = end;
        Ok(())
    }
}
impl FormModel for SleepForm {
    type Output = SleepSubmission;
    fn route(&self) -> Vec<ViewId> {
        let mut route = Vec::new();
        if self.given.operation != SleepOperation::Cancel && self.given.start.is_none() {
            route.push("start");
        }
        if self.given.operation == SleepOperation::Manual {
            if self.given.end.is_none() && self.given.given_end.is_none() {
                route.push("end");
            }
            if self.overlaps() && self.given.overlap.is_none() {
                route.push("overlap");
            }
        }
        route.into_iter().map(fields::create_id).collect()
    }
    fn create_view(&self, id: &ViewId, now: f64) -> Result<View> {
        let view = match id.field {
            "start" => fields::create_time(
                match self.given.operation {
                    SleepOperation::Start => "When did it start?",
                    SleepOperation::Manual => "When did it begin?",
                    SleepOperation::Stop => "When did it end?",
                    _ => "When?",
                },
                &self.calendar,
                self.start,
                now,
            ),
            "end" => View::create_time(
                "When did it end?",
                TimeState::create_time(
                    self.calendar.clone(),
                    Opening {
                        at: self.end.unwrap_or(now),
                        mode: Mode::Relative,
                        not_before: self.start,
                    },
                    now,
                ),
            ),
            "overlap" => fields::create_choices(
                "Which one do you want to keep?",
                &super::sleep::overlap_choices()
                    .iter()
                    .map(|choice| (choice.value, choice.hint))
                    .collect::<Vec<_>>(),
                0,
            ),
            _ => bail!("unknown sleep field"),
        };
        Ok(self.describe(id, view, now))
    }
    fn accept(&mut self, id: &ViewId, value: FormValue, now: f64) -> Result<()> {
        match id.field {
            "start" => self.accept_start(&value, now)?,
            "end" => {
                let end = fields::instant(&value)?;
                validate_interval(
                    &self.calendar,
                    self.start.ok_or_else(|| anyhow!("choose a start"))?,
                    end,
                    now,
                )?;
                self.end = Some(end);
            }
            "overlap" => {
                if matches!(&value,FormValue::Choice(value) if value == "cancel") {
                    return Err(prompt::Cancelled.into());
                }
                self.overlap = Some(fields::parse_choice(value)?);
            }
            _ => bail!("unknown sleep field"),
        }
        self.now = now;
        Ok(())
    }
    fn complete(&self) -> Result<SleepSubmission> {
        if self.given.operation != SleepOperation::Cancel && self.start.is_none() {
            bail!("provide --start <TIME>");
        }
        if self.given.operation == SleepOperation::Manual {
            validate_interval(
                &self.calendar,
                self.start.expect("checked start"),
                self.end.ok_or_else(|| anyhow!("provide --end <TIME>"))?,
                self.now,
            )?;
        }
        let overlap = if self.overlaps() {
            Some(
                self.overlap
                    .ok_or_else(|| anyhow!("choose --overlap <CHOICE>"))?,
            )
        } else {
            None
        };
        Ok(SleepSubmission {
            operation: self.given.operation,
            start: self.start,
            end: self.end,
            overlap,
            running: self.running.clone(),
        })
    }
}

/// Rejects reversed or unfinished manual intervals before any live-state mutation.
pub(crate) fn validate_interval(calendar: &Calendar, start: f64, end: f64, now: f64) -> Result<()> {
    if !start.is_finite() || !end.is_finite() || start < 0.0 {
        bail!("choose a time between the Unix epoch and now");
    }
    if end <= start {
        bail!("a sleep's end must be after its start");
    }
    if end > now {
        bail!(
            "that sleep ends at {}, which is still to come: record it when it has finished",
            format::clock(end, calendar)
        );
    }
    Ok(())
}
