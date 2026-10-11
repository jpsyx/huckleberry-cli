//! Preparation resolves prerequisites and binds each form to its existing operation.
use super::PreparedCommand;
use crate::{
    cli::{Command, actions::DiaperFlags},
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
    let (input, at) = match command {
        Command::Diaper {
            at,
            mode,
            pee,
            poo,
            color,
            consistency,
            rash,
            notes,
        } => (
            DiaperInput {
                mode,
                flags: DiaperFlags {
                    pee,
                    poo,
                    color,
                    consistency,
                    rash,
                },
                notes,
                ..DiaperInput::default()
            },
            at,
        ),
        Command::Potty {
            at,
            mode,
            how,
            color,
            consistency,
            notes,
        } => (
            DiaperInput {
                mode,
                outcome: how,
                potty: true,
                flags: DiaperFlags {
                    pee: None,
                    poo: None,
                    color,
                    consistency,
                    rash: false,
                },
                notes,
                at: None,
            },
            at,
        ),
        _ => return Ok(None),
    };
    let (client, cid) = crate::commands::client_and_child(&context).await?;
    let input = DiaperInput {
        at: diaper::resolve_at(&context, at.as_deref(), true)?,
        ..input
    };
    diaper::prepare_with_client(context, client, cid, input, now).map(Some)
}
