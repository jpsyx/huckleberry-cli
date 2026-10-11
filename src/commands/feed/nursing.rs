//! Nursing collection and execution share one typed transition across transports.
use super::nursing_form::{NursingForm, NursingInput, NursingOperation, NursingSubmission};
use crate::{
    cli::{NursingAction, Side},
    prompt::{self, flow::runner},
    render::{format, output},
    session::Context,
};
use anyhow::{Result, bail};
use huckleberry_api::{Huckleberry, TimerChange, client::now_seconds, models::feed::FeedSide};

/// Runs a nursing action or opens its read-only status view.
pub(super) async fn nursing(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    action: &NursingAction,
) -> Result<()> {
    if matches!(action, NursingAction::Status) {
        return nursing_status(context, client, cid).await;
    }
    let model = create_model(context, client, cid, action).await?;
    let answer = runner::read(model, now_seconds(), context.theme)?;
    execute(context, client, cid, answer).await
}

pub(super) async fn create_model(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    action: &NursingAction,
) -> Result<NursingForm> {
    let (operation, given_at, side) = match action {
        NursingAction::Start { start, side } => (NursingOperation::Start, start.as_deref(), *side),
        NursingAction::Pause { at } => (NursingOperation::Pause, at.as_deref(), None),
        NursingAction::Resume { at, side } => (NursingOperation::Resume, at.as_deref(), *side),
        NursingAction::Switch { at } => (NursingOperation::Switch, at.as_deref(), None),
        NursingAction::Stop { at } => (NursingOperation::Stop, at.as_deref(), None),
        NursingAction::Cancel => (NursingOperation::Cancel, None, None),
        NursingAction::Status => bail!("nursing status has no form"),
    };
    let at = if operation == NursingOperation::Cancel {
        None
    } else {
        crate::prompt::time::resolve_at(
            context,
            given_at,
            prompt::available() || prompt::host::hosted(),
        )?
    };
    let suggested = if operation == NursingOperation::Start && side.is_none() {
        suggested_side(client, cid).await
    } else {
        Side::Left
    };
    Ok(NursingForm::create_nursing(
        context.calendar()?,
        NursingInput {
            operation,
            at,
            side,
        },
        suggested,
    ))
}

async fn suggested_side(client: &Huckleberry, cid: &str) -> Side {
    let last = client
        .feed_document(cid)
        .await
        .ok()
        .flatten()
        .and_then(|document| document.prefs)
        .and_then(|prefs| prefs.last_side)
        .map(|side| side.last_side);
    if last == Some(FeedSide::Left) {
        Side::Right
    } else {
        Side::Left
    }
}

pub(super) async fn execute(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    answer: NursingSubmission,
) -> Result<()> {
    let at = answer.at.unwrap_or_default(); // Cancel is the only operation without a time.
    match answer.operation {
        NursingOperation::Start => {
            let side = answer.side.expect("validated start side").to_api();
            client.start_nursing_at(cid, side.clone(), at).await?;
            context.report(&format!("Nursing started on the {side}."));
        }
        NursingOperation::Pause => report(
            context,
            client.pause_nursing_at(cid, at).await?,
            "Paused.",
            "paused",
        ),
        NursingOperation::Resume => report(
            context,
            client
                .resume_nursing_at(cid, answer.side.map(Side::to_api), at)
                .await?,
            "Resumed.",
            "running",
        ),
        NursingOperation::Switch => report(
            context,
            client.switch_nursing_side_at(cid, at).await?,
            "Switched sides.",
            "running",
        ),
        NursingOperation::Cancel => report(
            context,
            client.cancel_nursing(cid).await?,
            "Thrown away. Nothing was recorded.",
            "not running",
        ),
        NursingOperation::Stop => stop_nursing(context, client, cid, at).await?,
    }
    Ok(())
}

async fn stop_nursing(context: &Context, client: &Huckleberry, cid: &str, at: f64) -> Result<()> {
    match client.complete_nursing_at(cid, at).await? {
        Some(completed) => context.receipt(
            "🤱 Nursing recorded",
            &output::nursing_fields(completed.left_seconds, completed.right_seconds),
            &[
                format!("total_seconds\t{:.0}", completed.total_seconds()),
                format!("left_seconds\t{:.0}", completed.left_seconds),
                format!("right_seconds\t{:.0}", completed.right_seconds),
            ],
        ),
        None => context.warn("No nursing session was running."),
    }
    Ok(())
}

async fn nursing_status(context: &Context, client: &Huckleberry, cid: &str) -> Result<()> {
    let document = client.feed_document(cid).await?;
    let at = now_seconds();
    let calendar = context.calendar()?;

    let Some(timer) = document
        .as_ref()
        .and_then(huckleberry_api::models::FeedDocument::running_timer)
    else {
        context.present(
            "🤱 Nursing status",
            &[("Status", "No nursing in progress".into())],
            &["running\tfalse".into()],
        );
        return Ok(());
    };

    let (left, right) = timer.totals(at);
    let mut machine = vec![
        "running\ttrue".into(),
        format!("paused\t{}", timer.paused),
        format!("side\t{}", timer.current_side()),
        format!("left_seconds\t{left:.0}"),
        format!("right_seconds\t{right:.0}"),
    ];
    let mut fields = vec![
        (
            "Status",
            if timer.paused { "Paused" } else { "Nursing" }.into(),
        ),
        ("Current side", output::words(timer.current_side().as_str())),
    ];
    if let Some(started) = timer.feed_start_time {
        machine.push(format!(
            "started_clock\t{}",
            format::clock(started.as_f64(), &calendar)
        ));
        fields.push(("Started", format::date_time(started.as_f64(), &calendar)));
    }
    fields.extend(output::nursing_fields(left, right));
    context.present("🤱 Nursing status", &fields, &machine);
    Ok(())
}

/// Turns a timer change into the line a person reads.
fn report(context: &Context, change: TimerChange, done: &str, already: &str) {
    match change {
        TimerChange::Applied => context.report(done),
        TimerChange::NotRunning => context.warn("No nursing session is running."),
        TimerChange::Unchanged => context.warn(&format!("The session is already {already}.")),
    }
}
