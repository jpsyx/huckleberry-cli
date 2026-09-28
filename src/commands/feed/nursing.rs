//! Nursing timer actions.
use super::{
    Choice, Context, FeedSide, Huckleberry, NursingAction, Question, Result, Side, TimerChange,
    format, now_seconds, output, prompt,
};
/// Runs the nursing timer.
pub(super) async fn nursing(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    action: &NursingAction,
) -> Result<()> {
    match action {
        NursingAction::Start { side, start } => {
            let at = prompt::time::read_start(context, start.as_deref())?;
            let side = match side {
                Some(given) => given.to_api(),
                None => suggested_side(context, client, cid).await?,
            };
            client.start_nursing_at(cid, side.clone(), at).await?;
            context.report(&format!("Nursing started on the {side}."));
            Ok(())
        }
        NursingAction::Pause { at } => {
            let at = prompt::time::read_at(context, at.as_deref())?;
            report(
                context,
                client.pause_nursing_at(cid, at).await?,
                "Paused.",
                "paused",
            );
            Ok(())
        }
        NursingAction::Resume { side, at } => {
            let at = prompt::time::read_at(context, at.as_deref())?;
            report(
                context,
                client
                    .resume_nursing_at(cid, side.map(Side::to_api), at)
                    .await?,
                "Resumed.",
                "running",
            );
            Ok(())
        }
        NursingAction::Switch { at } => {
            let at = prompt::time::read_at(context, at.as_deref())?;
            report(
                context,
                client.switch_nursing_side_at(cid, at).await?,
                "Switched sides.",
                "running",
            );
            Ok(())
        }
        NursingAction::Cancel => {
            report(
                context,
                client.cancel_nursing(cid).await?,
                "Thrown away. Nothing was recorded.",
                "not running",
            );
            Ok(())
        }
        NursingAction::Stop { at } => stop_nursing(context, client, cid, at.as_deref()).await,
        NursingAction::Status => nursing_status(context, client, cid).await,
    }
}

/// Which side to offer: the one opposite the last feed, which is what the app
/// suggests and what a parent half asleep wants pre-filled.
async fn suggested_side(context: &Context, client: &Huckleberry, cid: &str) -> Result<FeedSide> {
    const CHOICES: [Choice<'static>; 2] = [
        Choice {
            value: "left",
            hint: "The left side",
        },
        Choice {
            value: "right",
            hint: "The right side",
        },
    ];

    let last = client
        .feed_document(cid)
        .await
        .ok()
        .flatten()
        .and_then(|document| document.prefs)
        .and_then(|prefs| prefs.last_side)
        .map(|side| side.last_side);

    // The app suggests the side opposite the last feed, and so does this.
    let suggested = if last == Some(FeedSide::Left) {
        "right"
    } else {
        "left"
    };
    let question = Question::new("side", "Which side?", "--side <SIDE>")
        .with_choices(&CHOICES)
        .with_default(suggested);
    let answer = prompt::ask(&question, context.theme)?;
    Ok(if answer == "right" {
        FeedSide::Right
    } else {
        FeedSide::Left
    })
}

async fn stop_nursing(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    at: Option<&str>,
) -> Result<()> {
    let at = prompt::time::read_at(context, at)?;
    match client.complete_nursing_at(cid, at).await? {
        Some(completed) => {
            context.receipt(
                "🤱 Nursing recorded",
                &output::nursing_fields(completed.left_seconds, completed.right_seconds),
                &[
                    format!("total_seconds\t{:.0}", completed.total_seconds()),
                    format!("left_seconds\t{:.0}", completed.left_seconds),
                    format!("right_seconds\t{:.0}", completed.right_seconds),
                ],
            );
        }
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
