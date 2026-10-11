//! Recording a completed pump from validated form answers.
use super::{
    form::{PumpOperation, PumpSubmission},
    prompts::Volumes,
};
use crate::{
    render::{format, output},
    session::Context,
};
use anyhow::Result;
use huckleberry_api::{Huckleberry, models::pump::PumpAmounts, ops::pump::PumpEntry};

pub(super) async fn execute(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    answer: PumpSubmission,
) -> Result<()> {
    let at = answer.at.expect("validated event time");
    let volumes = Volumes {
        amounts: answer.amounts.expect("validated amounts"),
        units: answer.units,
    };
    if answer.operation == PumpOperation::Log {
        let entry = PumpEntry {
            amounts: volumes.amounts,
            duration: answer.duration,
            units: volumes.units.to_api(),
            notes: answer.notes.as_deref(),
        };
        client.log_pump_at(cid, entry, at).await?;
        recorded(
            context,
            at,
            answer.duration,
            &volumes,
            answer.notes.as_deref(),
        )
    } else {
        stop(context, client, cid, at, &volumes, answer.notes.as_deref()).await
    }
}

async fn stop(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    at: f64,
    volumes: &Volumes,
    notes: Option<&str>,
) -> Result<()> {
    let completed = client
        .complete_pump_at(
            cid,
            volumes.amounts,
            Some(volumes.units.to_api()),
            notes,
            at,
        )
        .await?;
    if let Some(completed) = completed {
        recorded(
            context,
            completed.start,
            Some(completed.duration),
            volumes,
            notes,
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
