//! Resolve sleep flags and live context before collecting a retained draft.
use crate::{
    cli::SleepAction,
    commands::{
        prepared::{CommandProgress, PreparedCommand},
        sleep_form::{SleepForm, SleepInput, SleepOperation, validate_interval},
    },
    domain::clock,
    prompt::{self, flow::runner},
    session::Context,
};
use anyhow::{Result, bail};
use huckleberry_api::Huckleberry;

/// Prepares sleep against the selected account and child without mutating either.
pub async fn prepare_with_client(
    context: Context,
    client: Huckleberry,
    cid: String,
    action: SleepAction,
    now: f64,
) -> Result<PreparedCommand> {
    let model = create_model(&context, &client, &cid, &action, now, true).await?;
    PreparedCommand::create_prepared(model, now, move |answer| async move {
        let outcome = super::execute(&context, &client, &cid, answer).await;
        let persisted = crate::commands::persist_session(&context, &client).await;
        outcome?;
        persisted?;
        Ok(CommandProgress::Finished { home: true })
    })
}
pub(super) async fn collect(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    action: &SleepAction,
) -> Result<crate::commands::sleep_form::SleepSubmission> {
    let now = huckleberry_api::client::now_seconds();
    runner::read(
        create_model(
            context,
            client,
            cid,
            action,
            now,
            prompt::available() || prompt::host::hosted(),
        )
        .await?,
        now,
        context.theme,
    )
}
async fn create_model(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    action: &SleepAction,
    now: f64,
    interactive: bool,
) -> Result<SleepForm> {
    let input = create_input(context, action, now, interactive)?;
    let running = if matches!(
        input.operation,
        SleepOperation::Manual | SleepOperation::Stop
    ) {
        client
            .sleep_document(cid)
            .await?
            .and_then(|document| document.timer)
    } else {
        None
    };
    SleepForm::create_sleep(context.calendar()?, input, running, now)
}
fn create_input(
    context: &Context,
    action: &SleepAction,
    now: f64,
    interactive: bool,
) -> Result<SleepInput> {
    let (operation, start, end, overlap) = match action {
        SleepAction::Start { start } => (SleepOperation::Start, start, None, None),
        SleepAction::Manual {
            start,
            end,
            overlap,
        } => (SleepOperation::Manual, start, end.as_deref(), *overlap),
        SleepAction::Pause { at } => (SleepOperation::Pause, at, None, None),
        SleepAction::Resume { at } => (SleepOperation::Resume, at, None, None),
        SleepAction::Stop { at } => (SleepOperation::Stop, at, None, None),
        SleepAction::Cancel => {
            return Ok(SleepInput {
                operation: SleepOperation::Cancel,
                start: None,
                end: None,
                given_end: None,
                overlap: None,
            });
        }
        SleepAction::Status => bail!("sleep status has no form"),
    };
    let calendar = context.calendar()?;
    let start = start
        .as_deref()
        .map(|text| prompt::time::parse_explicit(&calendar, text, now, clock::most_recent))
        .transpose()?;
    if operation == SleepOperation::Manual && !interactive {
        if start.is_none() {
            bail!("provide --start <TIME>");
        }
        if end.is_none() {
            bail!("provide --end <TIME>");
        }
    }
    let start = start.or_else(|| (!interactive).then_some(now));
    let resolved_end = match (start, end) {
        (Some(start), Some(text)) => Some(prompt::time::parse_explicit(
            &calendar,
            text,
            now,
            |time, _, calendar| clock::first_after(time, start, calendar),
        )?),
        _ => None,
    };
    if let (Some(start), Some(end)) = (start, resolved_end) {
        validate_interval(&calendar, start, end, now)?;
    }
    Ok(SleepInput {
        operation,
        start,
        end: resolved_end,
        given_end: end.filter(|_| resolved_end.is_none()).map(str::to_owned),
        overlap,
    })
}
