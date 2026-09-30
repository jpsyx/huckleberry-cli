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
use crate::domain::today::DayMode;
use crate::prompt::{self, Choice, Question};
use crate::session::Context;

use super::{Needs, default_mode, missing, refuse};

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
    if needs.settings {
        loop {
            let outstanding = missing(&current.config);
            let Some(key) = outstanding.first().copied() else {
                break;
            };
            if !prompt::available() {
                refuse(&outstanding)?;
            }
            let answer = ask(&current, key).await?;
            save(&current, key, &answer)?;
            current = current.reload()?;
        }
    }
    Ok(current)
}

/// Signs in, or says what would have.
async fn sign_in(context: &Context) -> Result<()> {
    if !prompt::available() {
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

/// Asks for one missing setting.
async fn ask(context: &Context, key: &str) -> Result<String> {
    if key == "day_mode" {
        return ask_day_mode(context).await;
    }
    ask_hour(context, key)
}

/// Which way this family counts a day.
///
/// The default is offered from the baby's age rather than chosen for them: a
/// newborn's day has no shape, and by twelve weeks it has one. Either answer
/// is one keystroke, and either can be changed later with `config set`.
async fn ask_day_mode(context: &Context) -> Result<String> {
    let age = age_in_days(context).await;
    let suggested = default_mode(age);
    if let Some(days) = age {
        context.detail(&format!("the profile says the baby is {days} days old"));
    }
    let choices = [
        Choice {
            value: DayMode::Continuous.key(),
            hint: DayMode::Continuous.describe(),
        },
        Choice {
            value: DayMode::Discrete.key(),
            hint: DayMode::Discrete.describe(),
        },
    ];
    prompt::ask(
        &Question::new(
            "how a day is counted",
            "How should \"today\" be counted? It decides what `fed today` covers.",
            "config set day_mode",
        )
        .with_choices(&choices)
        .with_default(suggested.key()),
        context.theme,
    )
}

/// What time a day, or a night, begins at.
fn ask_hour(context: &Context, key: &str) -> Result<String> {
    let (hours, fallback) = super::choices_for(key);
    let labels: Vec<(String, String)> = hours
        .iter()
        .map(|stored| ((*stored).to_owned(), label_for(stored)))
        .collect();
    let choices: Vec<Choice<'_>> = labels
        .iter()
        .map(|(value, hint)| Choice { value, hint })
        .collect();
    // A fixed-choice prompt draws its label and its choices and nothing else,
    // so anything the answer depends on has to be in the label itself.
    let label = if key == "day_start" {
        "What time does a day start? Anything earlier counts as the day before."
    } else {
        "What time does a day end? Night runs from here to the next day's start."
    };
    prompt::ask(
        &Question::new(key, label, &format!("config set {key}"))
            .with_choices(&choices)
            .with_default(fallback),
        context.theme,
    )
}

/// `06:00` as somebody reads it.
fn label_for(stored: &str) -> String {
    match crate::domain::clock::parse(stored) {
        Some(crate::domain::clock::Typed::Certain(time)) => time.label(),
        _ => stored.to_owned(),
    }
}

/// Writes one answer, and says back what it now means.
fn save(context: &Context, key: &str, value: &str) -> Result<()> {
    let mut config = context.config.clone();
    config.set(key, value)?;
    crate::config::save(&context.config_path, &config)?;
    let stored = config.get(key).unwrap_or_else(|| value.to_owned());
    context.report(&confirmation(key, &stored));
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
async fn age_in_days(context: &Context) -> Option<i64> {
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
