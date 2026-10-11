//! Small factories and parsers shared by typed command forms.
use super::{FormValue, View, ViewId, time::TimeState};
use crate::{
    domain::Calendar,
    prompt::dial::time::{Mode, Opening},
};
use anyhow::{Result, anyhow};
use clap::ValueEnum;

/// Identifies a non-repeated field independently of its displayed label.
#[must_use]
pub const fn create_id(field: &'static str) -> ViewId {
    ViewId { field, owner: None }
}

/// Builds a choice view with a stable semantic value for each row.
#[must_use]
pub fn create_choices(label: &str, values: &[(&str, &str)], default: usize) -> View {
    View::create_choices(
        label,
        values
            .iter()
            .map(|(value, label)| ((*label).into(), FormValue::Choice((*value).into())))
            .collect(),
        default,
    )
}

/// Opens an ordinary event-time dial with the exact prior answer, when present.
#[must_use]
pub fn create_time(label: &str, calendar: &Calendar, at: Option<f64>, now: f64) -> View {
    View::create_time(
        label,
        TimeState::create_time(
            calendar.clone(),
            Opening {
                at: at.unwrap_or(now),
                mode: Mode::Relative,
                not_before: None,
            },
            now,
        ),
    )
}

/// Reads a semantic choice through the same enum parser as command flags.
pub fn parse_choice<Value: ValueEnum>(value: FormValue) -> Result<Value> {
    let FormValue::Choice(value) = value else {
        return Err(anyhow!("choose a value"));
    };
    Value::from_str(&value, true).map_err(anyhow::Error::msg)
}

/// Reads a resolved instant, without interpreting relative text again.
pub fn instant(value: &FormValue) -> Result<f64> {
    match value {
        FormValue::Instant(at) => Ok(*at),
        _ => Err(anyhow!("choose a time")),
    }
}

/// Reads a text answer, preserving an explicit empty buffer.
pub fn text(value: FormValue) -> Result<String> {
    match value {
        FormValue::Text(value) => Ok(value),
        _ => Err(anyhow!("enter a value")),
    }
}

/// An empty optional text answer means omission, not the old default.
pub fn optional_text(value: FormValue) -> Result<Option<String>> {
    let text = text(value)?;
    Ok((!text.trim().is_empty()).then(|| text.trim().to_owned()))
}

/// Reads a semantic menu value without interpreting its label.
pub fn choice(value: FormValue) -> Result<String> {
    match value {
        FormValue::Choice(value) => Ok(value),
        _ => Err(anyhow!("choose a value")),
    }
}
