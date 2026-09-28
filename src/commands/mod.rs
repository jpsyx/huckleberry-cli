//! Subcommand dispatch, and the loading every screen shares.
//!
//! One thin module per command, each a shell around the pure functions in
//! `domain` and `render`. This file routes, opens the configuration once, and
//! owns the two things more than one command needs: working out which child to
//! act on, and getting a [`Dataset`] from either Huckleberry or a snapshot.

pub mod auth;
pub mod child;
pub mod dash;
pub mod diaper;
pub mod edit;
pub mod export;
pub mod feed;
pub mod foods;
pub mod growth;
pub mod info;
pub mod settings;
pub mod sleep;
pub mod views;

use anyhow::{Result, bail};
use huckleberry_api::Huckleberry;
use huckleberry_api::client::now_seconds;

use crate::cli::{Cli, Command};
use crate::domain::Calendar;
use crate::domain::types::Dataset;
use crate::prompt::{self, Choice, Question};
use crate::session::Context;
use crate::theme::Theme;

/// Runs the command the arguments selected.
pub async fn run(cli: &Cli, theme: Theme) -> Result<()> {
    let context = Context::open(cli, theme)?;
    match &cli.command {
        Command::Auth { action } => auth::run(&context, action).await,
        Command::Child { action } => child::run(&context, action).await,
        Command::Now { json } => views::now(&context, *json).await,
        // Boxed because the dashboard's future holds a terminal, a dataset and
        // a draw closure, and an unboxed one would make every other arm of
        // this match as large as the biggest.
        Command::Dash { days, refresh } => Box::pin(dash::run(&context, *days, *refresh)).await,
        Command::Summary { days, json } => views::summary(&context, *days, *json).await,
        Command::Trends { metric, days } => views::trends(&context, *metric, *days).await,
        Command::Stripes { days } => views::stripes(&context, *days).await,
        Command::Log { kind, days, limit } => views::log(&context, *kind, *days, *limit).await,
        Command::Edit {
            id,
            set,
            list,
            days,
            limit,
        } => {
            Box::pin(edit::run(
                &context,
                id.as_deref(),
                set,
                *list,
                *days,
                *limit,
            ))
            .await
        }
        Command::Sleep { action } => sleep::run(&context, action).await,
        Command::Feed { action } => feed::run(&context, action).await,
        Command::Diaper {
            mode,
            pee,
            poo,
            color,
            consistency,
            rash,
            notes,
        } => {
            diaper::nappy(
                &context,
                *mode,
                crate::cli::actions::NappyFlags {
                    pee: *pee,
                    poo: *poo,
                    color: *color,
                    consistency: *consistency,
                    rash: *rash,
                },
                notes.as_deref(),
            )
            .await
        }
        Command::Potty {
            mode,
            how,
            color,
            consistency,
            notes,
        } => {
            diaper::potty(
                &context,
                *mode,
                *how,
                *color,
                *consistency,
                notes.as_deref(),
            )
            .await
        }
        Command::Growth {
            weight,
            height,
            head,
            units,
        } => growth::run(&context, *weight, *height, *head, *units).await,
        Command::Foods { action } => foods::run(&context, action).await,
        Command::Export { days, out } => export::run(&context, *days, out.as_deref()).await,
        Command::Config { action } => settings::run(&context, action),
        Command::Info => info::run(&context),
    }
}

/// Which child to act on, asking when nobody has said and there is a terminal.
///
/// The question is worth asking well: it lists the children on the account by
/// name, and offers to remember the answer, so the next command does not ask
/// again. With no terminal it fails naming the flag, as every question here
/// does.
pub async fn which_child(context: &Context, client: &Huckleberry) -> Result<String> {
    if let Some(cid) =
        crate::session::resolve_child(context.child_override.as_deref(), context.config.child())
    {
        return Ok(cid);
    }

    context.detail("no child chosen: reading the account");
    let user = client.user().await?;
    if user.child_list.is_empty() {
        bail!("this account has no children on it");
    }
    if let [only] = user.child_list.as_slice() {
        context.detail(&format!("one child on the account: {}", only.cid));
        return Ok(only.cid.clone());
    }

    let labels: Vec<(String, String)> = user
        .child_list
        .iter()
        .map(|entry| {
            (
                entry.cid.clone(),
                entry.nickname.clone().unwrap_or_else(|| entry.cid.clone()),
            )
        })
        .collect();
    let choices: Vec<Choice<'_>> = labels
        .iter()
        .map(|(cid, name)| Choice {
            value: cid,
            hint: name,
        })
        .collect();
    let question = Question::new(
        "child to act on",
        "Which child?",
        "--child <CID> (or `child use`)",
    )
    .with_choices(&choices);
    let chosen = prompt::ask(&question, context.theme)?;

    if prompt::confirm("Remember this child?", true, context.theme)? {
        let mut config = context.config.clone();
        config.set("child", &chosen)?;
        crate::config::save(&context.config_path, &config)?;
        context.report(&format!("saved child = {chosen}"));
    }
    Ok(chosen)
}

/// Everything about the chosen child, from Huckleberry or from a snapshot.
///
/// `--offline` is not a fallback: it is a different source, chosen
/// deliberately, and nothing under it touches the network. That is what makes
/// the screens demonstrable without an account.
pub async fn load(context: &Context, days: Option<u32>) -> Result<(Dataset, Calendar)> {
    if let Some(path) = &context.offline {
        context.detail(&format!("reading {}", path.display()));
        let dataset = crate::dataset::read_snapshot(path)?;
        let calendar = Calendar::new(&dataset.timezone)?;
        return Ok((dataset, calendar));
    }

    let days = context.days(days);
    let client = context.client()?;
    let cid = which_child(context, &client).await?;
    context.narrate(&format!("Reading {days} days from Huckleberry..."));

    let nickname = nickname_for(&client, &cid).await;
    let dataset = crate::dataset::pull(
        &client,
        &cid,
        nickname.as_deref(),
        days,
        &context.config.timezone,
        now_seconds(),
    )
    .await?;
    persist_session(context, &client).await?;

    for note in &dataset.notes {
        context.detail(&format!(
            "{} was not read: {}",
            note.collection, note.problem
        ));
    }
    let calendar = Calendar::new(&dataset.timezone)?;
    Ok((dataset, calendar))
}

/// The account's own name for a child, when the account can be read.
///
/// A failure here is not worth reporting: the profile carries a name too, and
/// this is only the fallback for when it does not.
async fn nickname_for(client: &Huckleberry, cid: &str) -> Option<String> {
    client
        .user()
        .await
        .ok()?
        .child(cid)
        .and_then(|entry| entry.nickname.clone())
}

/// Writes back the session the client may have renewed, so the next command
/// does not have to sign in again.
pub async fn persist_session(context: &Context, client: &Huckleberry) -> Result<()> {
    let Some(session) = client.session().await else {
        return Ok(());
    };
    let mut resolved = context.credentials()?;
    if resolved.session.as_ref() == Some(&session) {
        return Ok(());
    }
    resolved.session = Some(session);
    context.save_credentials(&resolved)
}

/// A client and the child to act on, which every write command needs.
pub async fn client_and_child(context: &Context) -> Result<(Huckleberry, String)> {
    if context.offline.is_some() {
        bail!("`--offline` reads a snapshot, so there is nothing to write to");
    }
    let client = context.client()?;
    let cid = which_child(context, &client).await?;
    Ok((client, cid))
}
