//! Resolves explicit flags and suggestions before a retained pump form opens.
use super::{
    form::{PumpDefaults, PumpForm, PumpInput, PumpOperation},
    prompts::{self, Part},
};
use crate::{
    cli::{PumpAction, PumpValues, Units},
    commands::prepared::{CommandProgress, PreparedCommand},
    prompt::{self, flow::runner},
    session::Context,
};
use anyhow::{Result, bail};
use huckleberry_api::Huckleberry;

/// Prepares an owned pump draft against an already selected account and child.
pub async fn prepare_with_client(
    context: Context,
    client: Huckleberry,
    cid: String,
    action: PumpAction,
    now: f64,
) -> Result<PreparedCommand> {
    let model = create_model(&context, &client, &cid, &action).await?;
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
    action: &PumpAction,
) -> Result<super::form::PumpSubmission> {
    runner::read(
        create_model(context, client, cid, action).await?,
        huckleberry_api::client::now_seconds(),
        context.theme,
    )
}

async fn create_model(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    action: &PumpAction,
) -> Result<PumpForm> {
    let interactive = prompt::available() || prompt::host::hosted();
    let input = create_input(context, action, interactive)?;
    let has_amounts = input.values.amount.is_some()
        || (input.values.left.is_some() && input.values.right.is_some());
    let last = if matches!(input.operation, PumpOperation::Log | PumpOperation::Stop)
        && !has_amounts
        && interactive
    {
        client.latest_pump(cid).await.ok().flatten()
    } else {
        None
    };
    let units = input
        .values
        .units
        .unwrap_or_else(|| Units::from_setting(&context.config.units));
    let defaults = PumpDefaults {
        units,
        total: prompts::offered(last.as_ref(), Part::Total, units),
        left: prompts::offered(last.as_ref(), Part::Left, units),
        right: prompts::offered(last.as_ref(), Part::Right, units),
    };
    PumpForm::create_pump(context.calendar()?, input, defaults)
}

fn create_input(context: &Context, action: &PumpAction, interactive: bool) -> Result<PumpInput> {
    let (operation, given_at, values, duration) = match action {
        PumpAction::Log { options } => (
            PumpOperation::Log,
            options.at.as_deref(),
            options.values.clone(),
            options.duration,
        ),
        PumpAction::Stop { at, values } => {
            (PumpOperation::Stop, at.as_deref(), values.clone(), None)
        }
        PumpAction::Start { start } => (
            PumpOperation::Start,
            start.as_deref(),
            PumpValues::default(),
            None,
        ),
        PumpAction::Pause { at } => (
            PumpOperation::Pause,
            at.as_deref(),
            PumpValues::default(),
            None,
        ),
        PumpAction::Resume { at } => (
            PumpOperation::Resume,
            at.as_deref(),
            PumpValues::default(),
            None,
        ),
        PumpAction::Cancel => (PumpOperation::Cancel, None, PumpValues::default(), None),
        PumpAction::Status => bail!("pump status has no form"),
    };
    let at = if operation == PumpOperation::Cancel {
        None
    } else {
        prompt::time::resolve_at(context, given_at, interactive)?
    };
    Ok(PumpInput {
        operation,
        at,
        values,
        duration: duration.map(prompts::duration_seconds).transpose()?,
    })
}
