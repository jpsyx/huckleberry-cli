//! `child`: who the tool is tracking.

use anyhow::{Result, bail};

use crate::cli::ChildAction;
use crate::domain::normalize;
use crate::prompt::{self, Choice, Question};
use crate::render::format;
use crate::session::Context;

/// Runs the chosen action.
pub async fn run(context: &Context, action: &ChildAction) -> Result<()> {
    match action {
        ChildAction::List => list(context).await,
        ChildAction::Use { cid } => choose(context, cid.clone()).await,
        ChildAction::Show => show(context).await,
    }
}

/// Lists the children on the account, one `cid\tname` line each.
async fn list(context: &Context) -> Result<()> {
    let client = context.client()?;
    let user = client.user().await?;
    super::persist_session(context, &client).await?;

    if user.child_list.is_empty() {
        context.warn("This account has no children on it.");
        return Ok(());
    }
    show_list(context, &user.child_list);
    Ok(())
}

/// Lists names first, while preserving child IDs for scripts and selection.
pub(super) fn show_list(
    context: &Context,
    children: &[huckleberry_api::models::user::UserChildRef],
) {
    let chosen = context
        .child_override
        .as_deref()
        .or_else(|| context.config.child());
    let machine: Vec<String> = children
        .iter()
        .map(|child| {
            format!(
                "{}\t{}",
                child.cid,
                child.nickname.as_deref().unwrap_or("(no nickname)")
            )
        })
        .collect();
    let rows: Vec<Vec<String>> = children
        .iter()
        .map(|child| {
            vec![
                child
                    .nickname
                    .clone()
                    .unwrap_or_else(|| "No nickname".into()),
                if chosen == Some(child.cid.as_str()) {
                    "Selected"
                } else {
                    ""
                }
                .into(),
                child.cid.clone(),
            ]
        })
        .collect();
    context.table(
        "👶 Children",
        &["Name", "Tracking", "Child ID"],
        &rows,
        &machine,
    );
}

/// Chooses the child every command acts on.
async fn choose(context: &Context, cid: Option<String>) -> Result<()> {
    let client = context.client()?;
    let user = client.user().await?;
    super::persist_session(context, &client).await?;

    if user.child_list.is_empty() {
        bail!("this account has no children on it");
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

    let chosen = if let Some(named) = cid {
        if !labels.iter().any(|(cid, _)| *cid == named) {
            bail!(unknown_child_message(&named, &labels));
        }
        named
    } else {
        {
            let choices: Vec<Choice<'_>> = labels
                .iter()
                .map(|(cid, name)| Choice {
                    value: cid,
                    hint: name,
                })
                .collect();
            let question =
                Question::new("child to use", "Which child?", "<CID>").with_choices(&choices);
            prompt::ask(&question, context.theme)?
        }
    };

    let mut config = context.config.clone();
    config.set("child", &chosen)?;
    crate::config::save(&context.config_path, &config)?;
    let name = labels
        .iter()
        .find(|(cid, _)| *cid == chosen)
        .map_or(chosen.as_str(), |(_, name)| name.as_str());
    context.receipt(
        "👶 Child selected",
        &[("Name", name.into()), ("Child ID", chosen)],
        &[],
    );
    Ok(())
}

/// Shows one child's profile.
async fn show(context: &Context) -> Result<()> {
    let (client, cid) = super::client_and_child(context).await?;
    let profile = client.child(&cid).await?;
    super::persist_session(context, &client).await?;

    let child = normalize::child(&cid, None, profile.as_ref());
    let calendar = context.calendar()?;
    let now = huckleberry_api::client::now_seconds();

    let mut machine = profile_machine(&child, &calendar, now);
    let mut fields = crate::render::output::profile_fields(&child, &calendar, now);
    if let Some(growth) = client.latest_growth(&cid).await? {
        if let Some(kilograms) = growth.weight_kilograms() {
            machine.push(format!("weight_kg\t{kilograms:.3}"));
            fields.extend(crate::render::output::weight_fields(
                kilograms,
                growth.start.as_f64(),
                context.config.measurements == "imperial",
                &calendar,
            ));
        }
        machine.push(format!(
            "weight_measured\t{}",
            format::clock(growth.start.as_f64(), &calendar)
        ));
    }
    context.present("👶 Child profile", &fields, &machine);
    Ok(())
}

/// The stable pipe representation of the profile.
fn profile_machine(
    child: &crate::domain::types::Child,
    calendar: &crate::domain::Calendar,
    now: f64,
) -> Vec<String> {
    let mut lines = vec![
        format!("cid\t{}", child.cid),
        format!("name\t{}", child.name),
        format!(
            "birthdate\t{}",
            child.birthdate.as_deref().unwrap_or("(unknown)")
        ),
    ];
    if let Some(age) = child
        .birthdate
        .as_deref()
        .and_then(|birthdate| calendar.age_in_days(birthdate, now))
    {
        lines.push(format!("age_days\t{age}"));
    }
    lines.extend([
        format!("night_start\t{}", hour_label(child.night_start_hour)),
        format!("morning_cutoff\t{}", hour_label(child.morning_cutoff_hour)),
        format!("timezone\t{}", calendar.name()),
    ]);
    lines
}

/// An hour-with-a-fraction as a clock time: `20.0` is `20:00`.
#[must_use]
pub fn hour_label(hour_fraction: f64) -> String {
    let (hour, minute) = crate::domain::time::split_hour(hour_fraction);
    format!("{hour:02}:{minute:02}")
}

/// The refusal when a named child is not on the account.
#[must_use]
pub fn unknown_child_message(named: &str, children: &[(String, String)]) -> String {
    let known: Vec<String> = children
        .iter()
        .map(|(cid, name)| format!("{cid} ({name})"))
        .collect();
    format!(
        "no child `{named}` on this account: try {}",
        known.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_hour_with_a_fraction_reads_as_a_clock_time() {
        assert_eq!(hour_label(20.0), "20:00");
        assert_eq!(hour_label(6.75), "06:45");
    }

    #[test]
    fn a_child_that_is_not_on_the_account_is_refused_with_the_ones_that_are() {
        let children = vec![
            ("abc".to_owned(), "Bear".to_owned()),
            ("def".to_owned(), "Fox".to_owned()),
        ];
        let message = unknown_child_message("xyz", &children);
        assert!(message.contains("abc (Bear)"), "{message}");
        assert!(message.contains("def (Fox)"), "{message}");
    }
}
