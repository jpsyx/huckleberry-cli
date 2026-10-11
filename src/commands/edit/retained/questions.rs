//! Owned controls for a selected edit field, with original values already filled.
use super::entry::EntryDraft;
use crate::{
    commands::edit::{form, form_pump},
    domain::Calendar,
    edit::Draft,
    interactive::options::fields,
    prompt::{
        dial::time::{Mode, Opening},
        flow::{FormValue, View, time::TimeState, volume::VolumeState},
    },
};
use anyhow::{Result, anyhow};
impl EntryDraft {
    pub fn question(&self, calendar: &Calendar, now: f64) -> Result<View> {
        let field = self.field.ok_or_else(|| anyhow!("choose a field"))?;
        let shown = self.shown();
        if matches!(field, "at" | "start") || form::is_stop(shown.as_ref(), field) {
            let at = if field == "duration" {
                self.stopped
                    .unwrap_or_else(|| form::stop_of(shown.as_ref(), self.started))
            } else {
                self.started
            };
            return Ok(View::create_time(
                form::label_for(shown.as_ref(), field),
                TimeState::create_time(
                    calendar.clone(),
                    Opening {
                        at,
                        mode: Mode::Absolute,
                        not_before: (field == "duration").then_some(self.started),
                    },
                    now,
                ),
            ));
        }
        if field == "amount"
            && let Some(Draft::Bottle(bottle)) = &shown
        {
            return Ok(View::create_volume(
                "How much?",
                VolumeState::create_volume(bottle.units, Some(bottle.amount), 1.0),
            ));
        }
        if field == "duration" && matches!(shown, Some(Draft::Pump(_))) {
            let mut choices = vec![
                (
                    "Keep current value".into(),
                    FormValue::Choice("keep".into()),
                ),
                ("Clear value".into(), FormValue::Choice("clear".into())),
            ];
            choices.extend(
                alternatives(shown.as_ref(), field)
                    .into_iter()
                    .map(|(value, label)| (label, FormValue::Choice(format!("value:{value}")))),
            );
            choices.push(("Other duration".into(), FormValue::Choice("change".into())));
            return Ok(View::create_choices("Duration", choices, 0));
        }
        let mut choices = vec![(
            "Keep current value".into(),
            FormValue::Choice("keep".into()),
        )];
        let values = alternatives(shown.as_ref(), field);
        choices.extend(
            values
                .iter()
                .map(|(value, label)| (label.clone(), FormValue::Choice(format!("value:{value}")))),
        );
        if values.is_empty() || field == "duration" {
            choices.push((
                "Enter a new value".into(),
                FormValue::Choice("change".into()),
            ));
        }
        if form_pump::can_clear(shown.as_ref(), field) || field == "duration" {
            choices.push(("Clear value".into(), FormValue::Choice("clear".into())));
        }
        Ok(View::create_choices(
            form::label_for(shown.as_ref(), field),
            choices,
            0,
        ))
    }
    pub fn input(&self) -> Result<View> {
        let field = self.field.ok_or_else(|| anyhow!("choose a field"))?;
        let shown = self.shown();
        if matches!(field, "amount" | "left" | "right")
            && let Some(Draft::Pump(pump)) = &shown
        {
            let amount = match field {
                "left" => pump.left_amount,
                "right" => pump.right_amount,
                _ => pump.total_amount(),
            };
            return Ok(View::create_volume(
                field,
                VolumeState::create_volume(pump.units, amount, 0.0),
            ));
        }
        Ok(View::create_text(
            "New value?",
            &raw_value(shown.as_ref(), field),
            false,
        ))
    }
}
fn alternatives(draft: Option<&Draft>, field: &str) -> Vec<(String, String)> {
    if matches!(draft, Some(Draft::Pump(_))) {
        if field == "mode" {
            return vec![
                ("total".into(), "Total".into()),
                ("leftright".into(), "Left and right".into()),
            ];
        }
        if field == "duration" {
            return [0, 5, 10, 15, 20, 30]
                .into_iter()
                .map(|minutes| (minutes.to_string(), format!("{minutes} minutes")))
                .collect();
        }
    }
    fields::choices(field)
}
fn raw_value(draft: Option<&Draft>, field: &str) -> String {
    match draft {
        Some(Draft::Nursing(nursing)) if field == "left" => nursing.left_minutes.to_string(),
        Some(Draft::Nursing(nursing)) if field == "right" => nursing.right_minutes.to_string(),
        Some(Draft::Pump(pump)) if field == "duration" => pump
            .duration_minutes
            .map_or_else(String::new, |value| value.to_string()),
        Some(draft) if field == "notes" => match draft {
            Draft::Diaper(value) => value.notes.clone(),
            Draft::Bottle(value) => value.notes.clone(),
            Draft::Nursing(value) => value.notes.clone(),
            Draft::Solids(value) => value.notes.clone(),
            Draft::Sleep(value) => value.notes.clone(),
            Draft::Pump(value) => value.notes.clone(),
        }
        .unwrap_or_default(),
        Some(draft) => fields::current(draft, field),
        None => String::new(),
    }
}
