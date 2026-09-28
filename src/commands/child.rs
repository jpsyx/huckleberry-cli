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
    let chosen = context.config.child();
    for entry in &user.child_list {
        let name = entry.nickname.as_deref().unwrap_or("(no nickname)");
        println!("{}\t{name}", entry.cid);
        if chosen == Some(entry.cid.as_str()) {
            context.detail(&format!("  {} is the chosen child", entry.cid));
        }
    }
    Ok(())
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
    context.report(&format!("Now tracking {chosen}."));
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

    println!("cid\t{}", child.cid);
    println!("name\t{}", child.name);
    println!(
        "birthdate\t{}",
        child.birthdate.as_deref().unwrap_or("(unknown)")
    );
    if let Some(age) = child
        .birthdate
        .as_deref()
        .and_then(|birthdate| calendar.age_in_days(birthdate, now))
    {
        println!("age_days\t{age}");
    }
    println!("night_start\t{}", hour_label(child.night_start_hour));
    println!("morning_cutoff\t{}", hour_label(child.morning_cutoff_hour));
    println!("timezone\t{}", calendar.name());

    if let Some(growth) = client.latest_growth(&cid).await? {
        if let Some(kilograms) = growth.weight_kilograms() {
            println!("weight_kg\t{kilograms:.3}");
        }
        // The measurement's own time, so a weight from last month is not read
        // as this morning's.
        println!(
            "weight_measured\t{}",
            format::clock(growth.start.as_f64(), &calendar)
        );
    }
    Ok(())
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
