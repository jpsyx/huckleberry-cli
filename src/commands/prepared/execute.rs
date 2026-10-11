//! Preparation resolves prerequisites and binds each form to its existing operation.
use super::PreparedCommand;
use crate::{
    cli::{Command, FeedAction, NursingAction, PumpAction, SleepAction, actions::DiaperFlags},
    commands::{diaper, diaper_form::DiaperInput},
    session::Context,
};
use anyhow::Result;

/// Prepares a retained command, or leaves a read-only operation to its existing viewer.
pub async fn prepare(
    context: Context,
    command: Command,
    now: f64,
) -> Result<Option<PreparedCommand>> {
    match command {
        Command::Edit { options } if !options.list => {
            let (client, cid) = crate::commands::client_and_child(&context).await?;
            Box::pin(crate::commands::edit::prepare::prepare_with_client(
                context, client, cid, options, now,
            ))
            .await
            .map(Some)
        }
        Command::Delete { options } if !options.list => {
            let (client, cid) = crate::commands::client_and_child(&context).await?;
            Box::pin(crate::commands::delete::prepare_with_client(
                context, client, cid, options, now,
            ))
            .await
            .map(Some)
        }
        Command::Growth {
            weight,
            height,
            head,
            units,
            at,
        } => {
            let (client, cid) = crate::commands::client_and_child(&context).await?;
            let input = crate::commands::growth::create_input(
                &context,
                weight,
                height,
                head,
                units,
                at.as_deref(),
            )?;
            crate::commands::growth::prepare_with_client(context, client, cid, input, now).map(Some)
        }
        Command::Sleep { action } => prepare_sleep(context, action, now).await,
        Command::Pump { action } => prepare_pump(context, action, now).await,
        Command::Feed { action } => prepare_feed(context, action, now).await,
        command => prepare_diaper(context, command, now).await,
    }
}

async fn prepare_pump(
    context: Context,
    action: PumpAction,
    now: f64,
) -> Result<Option<PreparedCommand>> {
    if matches!(action, PumpAction::Status) {
        return Ok(None);
    }
    let (client, cid) = crate::commands::client_and_child(&context).await?;
    crate::commands::pump::prepare::prepare_with_client(context, client, cid, action, now)
        .await
        .map(Some)
}

async fn prepare_feed(
    context: Context,
    action: FeedAction,
    now: f64,
) -> Result<Option<PreparedCommand>> {
    if matches!(
        action,
        FeedAction::Nursing {
            action: NursingAction::Status
        }
    ) {
        return Ok(None);
    }
    let (client, cid) = crate::commands::client_and_child(&context).await?;
    crate::commands::feed::prepare::prepare_with_client(context, client, cid, action, now)
        .await
        .map(Some)
}

async fn prepare_diaper(
    context: Context,
    command: Command,
    now: f64,
) -> Result<Option<PreparedCommand>> {
    let Some((input, at)) = diaper_input(command) else {
        return Ok(None);
    };
    let (client, cid) = crate::commands::client_and_child(&context).await?;
    let input = DiaperInput {
        at: crate::prompt::time::resolve_at(&context, at.as_deref(), true)?,
        ..input
    };
    diaper::prepare_with_client(context, client, cid, input, now).map(Some)
}

fn diaper_input(command: Command) -> Option<(DiaperInput, Option<String>)> {
    let mut input = DiaperInput::default();
    let at = match command {
        Command::Diaper {
            at,
            mode,
            pee,
            poo,
            color,
            consistency,
            rash,
            notes,
        } => {
            input.mode = mode;
            input.flags = DiaperFlags {
                pee,
                poo,
                color,
                consistency,
                rash,
            };
            input.notes = notes;
            at
        }
        Command::Potty {
            at,
            mode,
            how,
            color,
            consistency,
            notes,
        } => {
            input.potty = true;
            input.mode = mode;
            input.outcome = how;
            input.flags.color = color;
            input.flags.consistency = consistency;
            input.notes = notes;
            at
        }
        _ => return None,
    };
    Some((input, at))
}

async fn prepare_sleep(
    context: Context,
    action: SleepAction,
    now: f64,
) -> Result<Option<PreparedCommand>> {
    if matches!(action, SleepAction::Status) {
        return Ok(None);
    }
    let (client, cid) = crate::commands::client_and_child(&context).await?;
    crate::commands::sleep::prepare::prepare_with_client(context, client, cid, action, now)
        .await
        .map(Some)
}
