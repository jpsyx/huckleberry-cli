//! Pump view factories share controls with every other recording form.
use super::{PumpForm, PumpOperation};
use crate::prompt::flow::{View, fields, volume::VolumeState};
use anyhow::{Result, bail};

pub(super) fn create_view(form: &PumpForm, field: &str, now: f64) -> Result<View> {
    Ok(match field {
        "at" => fields::create_time(
            match form.given.operation {
                PumpOperation::Start => "When did it start?",
                PumpOperation::Stop => "When did it end?",
                _ => "When?",
            },
            &form.calendar,
            form.at,
            now,
        ),
        "mode" => fields::create_choices(
            "Record which amounts?",
            &[("total", "Total"), ("leftright", "Left and right")],
            0,
        ),
        "total" | "left" | "right" => {
            let (label, amount) = match field {
                "total" => ("Total amount?", form.defaults.total),
                "left" => ("Left amount?", form.defaults.left),
                _ => ("Right amount?", form.defaults.right),
            };
            View::create_volume(
                label,
                VolumeState::create_volume(
                    form.given.values.units.unwrap_or(form.defaults.units),
                    amount,
                    0.0,
                ),
            )
        }
        "duration" => fields::create_choices(
            "How long?",
            &[
                ("skip", "Skip"),
                ("0", "0 minutes"),
                ("5", "5 minutes"),
                ("10", "10 minutes"),
                ("15", "15 minutes"),
                ("20", "20 minutes"),
                ("30", "30 minutes"),
                ("custom", "Enter minutes"),
            ],
            0,
        ),
        "custom_duration" => View::create_text("How many minutes?", "", false),
        "notes" => View::create_text("Anything to note?", "", false),
        _ => bail!("unknown pump field"),
    })
}
