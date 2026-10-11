//! Asking for what is missing, and signing in when nobody has.
//!
//! The thin impure half of [`super`]: it reads credentials, opens a client,
//! asks questions and writes the configuration file. Every decision it acts on
//! is a pure function next door.

use anyhow::Result;

use crate::cli::AuthAction;
use crate::config::Config;
use crate::domain::clock::TimeOfDay;
use crate::domain::time::split_hour;
use crate::prompt;
use crate::session::Context;

use super::{Needs, missing, refuse};

/// Puts whatever is missing in place, and hands back a context to dispatch on.
///
/// The context is reopened after anything is written, so a command never acts
/// on the settings as they were before setup ran.
pub async fn ensure(context: &Context, needs: Needs) -> Result<Context> {
    let mut current = context.reload()?;
    if needs.credentials && !current.credentials()?.is_usable() {
        sign_in(&current).await?;
        current = current.reload()?;
    }
    if needs.settings && !missing(&current.config).is_empty() {
        if !(prompt::available() || crate::prompt::host::hosted()) {
            refuse(&missing(&current.config))?;
        }
        let model = super::form::SetupForm::create_setup(
            current.config.clone(),
            age_in_days(&current).await,
        );
        let config = crate::prompt::flow::runner::read(
            model,
            huckleberry_api::client::now_seconds(),
            current.theme,
        )?;
        save(&current, &config)?;
        current = current.reload()?;
    }
    Ok(current)
}

/// Signs in, or says what would have.
async fn sign_in(context: &Context) -> Result<()> {
    if !(prompt::available() || crate::prompt::host::hosted()) {
        anyhow::bail!(crate::session::not_signed_in_message());
    }
    context.narrate("Nobody is signed in yet, so let us do that first.");
    crate::commands::auth::run(
        context,
        &AuthAction::Login {
            email: None,
            password: None,
            timezone: None,
        },
    )
    .await
}

/// `06:00` as somebody reads it.
pub(crate) fn label_for(stored: &str) -> String {
    match crate::domain::clock::parse(stored) {
        Some(crate::domain::clock::Typed::Certain(time)) => time.label(),
        _ => stored.to_owned(),
    }
}

/// Saves a completed setup once, then reports all answers together.
pub(crate) fn save(context: &Context, config: &Config) -> Result<()> {
    let outstanding = missing(&context.config);
    if !missing(config).is_empty() {
        refuse(&missing(config))?;
    }
    crate::config::save(&context.config_path, config)?;
    for key in outstanding {
        context.report(&confirmation(key, &config.get(key).unwrap_or_default()));
    }
    Ok(())
}

/// What one answer is read back as, in the words the question asked in.
///
/// Times read back as somebody says them rather than as they are stored, so
/// `06:00` is confirmed as `6:00 am` and a mis-picked pm is visible at once.
#[must_use]
pub fn confirmation(key: &str, stored: &str) -> String {
    match key {
        "day_mode" => crate::domain::today::DayMode::from_key(stored).map_or_else(
            || format!("A day is counted {stored}."),
            |mode| format!("A day is counted {stored}: {}.", mode.describe()),
        ),
        "day_start" => format!("The day starts at {}.", label_for(stored)),
        _ => format!(
            "The day ends at {}, and night runs from there.",
            label_for(stored)
        ),
    }
}

/// The baby's age, when it can be had without making anybody wait.
///
/// Best effort on purpose: this only chooses which answer is offered first,
/// and a question that cannot be asked because the profile would not load is a
/// worse outcome than a default somebody changes with one keystroke.
pub(crate) async fn age_in_days(context: &Context) -> Option<i64> {
    let calendar = context.calendar().ok()?;
    let now = huckleberry_api::client::now_seconds();
    if let Some(path) = &context.offline {
        let dataset = crate::dataset::read_snapshot(path).ok()?;
        return calendar.age_in_days(dataset.child.birthdate.as_deref()?, now);
    }
    let client = context.client().ok()?;
    let cid = only_child(context, &client).await?;
    let profile = client.child(&cid).await.ok()??;
    let child = crate::domain::normalize::child(&cid, None, Some(&profile));
    calendar.age_in_days(child.birthdate.as_deref()?, now)
}

/// The child to read an age off, without asking which.
async fn only_child(context: &Context, client: &huckleberry_api::Huckleberry) -> Option<String> {
    if let Some(cid) =
        crate::session::resolve_child(context.child_override.as_deref(), context.config.child())
    {
        return Some(cid);
    }
    match client.user().await.ok()?.child_list.as_slice() {
        [only] => Some(only.cid.clone()),
        _ => None,
    }
}

/// An hour fraction as a time somebody reads, for the lines that report one.
#[must_use]
pub fn hour_label(hour_fraction: f64) -> String {
    let (hours, minutes) = split_hour(hour_fraction);
    TimeOfDay::new(hours, minutes).map_or_else(|| format!("{hour_fraction:.2}"), TimeOfDay::label)
}

/// The settings setup would still ask for, for `config show`.
#[must_use]
pub fn outstanding(config: &Config) -> Vec<&'static str> {
    missing(config)
}
