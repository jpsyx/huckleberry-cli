//! The live sleep is a timer edit, never an edit of a history row.

use anyhow::{Result, bail};
use huckleberry_api::{Huckleberry, TimerChange};

use crate::domain::log::{Entry, Kind};
use crate::domain::types::LiveState;
use crate::render::format;
use crate::session::Context;

/// A stable command-line selector for the currently active sleep.
pub const TOKEN: &str = "sleep/current";

/// Offers an active timer above history, including when it is paused.
pub fn entry(state: &LiveState, now: f64) -> Option<Entry> {
    state.sleep_active.then(|| Entry {
        id: TOKEN.into(),
        at: None,
        kind: Kind::Sleep,
        start: state.sleep_start.unwrap_or(now),
        title: "Ongoing sleep".into(),
        description: if state.sleep_paused {
            "timer paused · edit start time"
        } else {
            "still sleeping · edit start time"
        }
        .into(),
        notes: state
            .sleep_start
            .is_none()
            .then(|| "start time is not recorded".into()),
    })
}

/// Changes the selected session after asking for its corrected start.
pub async fn run(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    changes: &[String],
) -> Result<()> {
    let given = start_value(changes)?;
    let timer = client
        .sleep_document(cid)
        .await?
        .and_then(|document| document.timer)
        .filter(|timer| timer.active)
        .ok_or_else(|| anyhow::anyhow!("no ongoing sleep to edit"))?;
    let Some(started) = crate::prompt::time::read_edit_start(context, given, timer.started_at())?
    else {
        super::super::persist_session(context, client).await?;
        context.report("The ongoing sleep is unchanged.");
        return Ok(());
    };
    let outcome = client.update_sleep_start(cid, &timer.uuid, started).await?;
    super::super::persist_session(context, client).await?;
    if outcome == TimerChange::Unchanged {
        context.report("The ongoing sleep is unchanged.");
        return Ok(());
    }
    receipt(context, timer.paused, started)
}

/// Reports the corrected start without implying that a history entry was saved.
fn receipt(context: &Context, paused: bool, started: f64) -> Result<()> {
    let calendar = context.calendar()?;
    context.receipt(
        "😴 Ongoing sleep updated",
        &[
            ("Started", format::date_time(started, &calendar)),
            (
                "Status",
                if paused {
                    "Still paused"
                } else {
                    "Still sleeping"
                }
                .into(),
            ),
        ],
        &[format!("entry\t{TOKEN}"), format!("start\t{started}")],
    );
    Ok(())
}

/// Only a new start belongs to a live timer; duration would finish a sleep.
fn start_value(changes: &[String]) -> Result<Option<&str>> {
    match changes {
        [] => Ok(None),
        [change] => {
            let Some((key, value)) = change.split_once('=') else {
                bail!("use --set start=<TIME> for an ongoing sleep");
            };
            if key != "start" || value.trim().is_empty() {
                bail!("an ongoing sleep accepts only --set start=<TIME>");
            }
            Ok(Some(value))
        }
        _ => bail!("supply exactly one --set start=<TIME> for an ongoing sleep"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_live_edit_only_accepts_one_nonempty_start() {
        assert_eq!(
            start_value(&["start=32 mins ago".into()]).unwrap(),
            Some("32 mins ago")
        );
        for changes in [
            vec!["duration=10".into()],
            vec!["start=".into()],
            vec!["start=now".into(), "start=9pm".into()],
        ] {
            assert!(start_value(&changes).is_err());
        }
    }
}
