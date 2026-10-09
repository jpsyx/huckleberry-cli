//! Recording a completed pump, manually or from the live timer.

use anyhow::Result;
use huckleberry_api::Huckleberry;
use huckleberry_api::models::pump::PumpAmounts;
use huckleberry_api::ops::pump::PumpEntry;

use super::prompts::{self, Volumes};
use crate::cli::{PumpLogOptions, PumpValues};
use crate::prompt;
use crate::render::{format, output};
use crate::session::Context;

/// Records a completed session after asking when first.
pub(super) async fn log(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    options: &PumpLogOptions,
) -> Result<()> {
    let at = prompt::time::read_at(context, options.at.as_deref())?;
    let volumes = prompts::volumes(context, client, cid, &options.values).await?;
    let duration = prompts::duration(context, options.duration)?;
    let notes = prompts::notes(context, options.values.notes.as_deref())?;
    let entry = PumpEntry {
        amounts: volumes.amounts,
        duration,
        units: volumes.units.to_api(),
        notes: notes.as_deref(),
    };
    client.log_pump_at(cid, entry, at).await?;
    recorded(context, at, duration, &volumes, notes.as_deref())
}

/// Finishes the timer with the amounts expressed.
pub(super) async fn stop(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    at: Option<&str>,
    values: &PumpValues,
) -> Result<()> {
    let at = prompt::time::read_end(context, at)?;
    let volumes = prompts::volumes(context, client, cid, values).await?;
    let notes = prompts::notes(context, values.notes.as_deref())?;
    let completed = client
        .complete_pump_at(
            cid,
            volumes.amounts,
            Some(volumes.units.to_api()),
            notes.as_deref(),
            at,
        )
        .await?;
    if let Some(completed) = completed {
        recorded(
            context,
            completed.start,
            Some(completed.duration),
            &volumes,
            notes.as_deref(),
        )?;
    } else {
        context.warn("No pumping session was running.");
    }
    Ok(())
}

/// One receipt for manual sessions and completed timers.
fn recorded(
    context: &Context,
    start: f64,
    duration: Option<f64>,
    volumes: &Volumes,
    notes: Option<&str>,
) -> Result<()> {
    let mut fields = vec![("When", format::date_time(start, &context.calendar()?))];
    let mut machine = vec![
        format!("start\t{start}"),
        format!("units\t{}", volumes.units.as_str()),
    ];
    for (label, key, amount) in quantities(volumes.amounts) {
        fields.push((
            label,
            format!(
                "{} {}",
                format::amount_in(amount, volumes.units),
                volumes.units.as_str()
            ),
        ));
        machine.push(format!("{key}\t{amount}"));
    }
    if let Some(duration) = duration {
        fields.push(("Duration", output::duration(duration)));
        machine.push(format!("duration_seconds\t{duration}"));
    }
    fields.push(("Notes", notes.unwrap_or("None").into()));
    context.receipt("🧴 Pumping recorded", &fields, &machine);
    Ok(())
}

/// Human and machine labels for the mode that was actually recorded.
fn quantities(amounts: PumpAmounts) -> Vec<(&'static str, &'static str, f64)> {
    match amounts {
        PumpAmounts::Total(amount) => vec![("Total", "amount", amount)],
        PumpAmounts::LeftRight { left, right } => vec![
            ("Left", "left", left),
            ("Right", "right", right),
            ("Total", "amount", left + right),
        ],
    }
}
