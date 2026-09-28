//! Pulling everything about one child into one value.
//!
//! Two rules shape this module.
//!
//! **One refusal must not cost the rest.** Several Huckleberry trackers are
//! simply not readable on some accounts: the tracker was never switched on, or
//! the account has no permission. A pull that aborts on the first one leaves a
//! parent with nothing. So each collection is attempted separately and a
//! failure becomes a note on the dataset, which the screens then show. A
//! parent seeing no pumping sessions can tell "none logged" from "not allowed
//! to look".
//!
//! **The reads happen together.** Six collections over a home connection is
//! several seconds if they queue. They do not depend on each other, so they
//! are awaited concurrently.

use std::path::Path;

use anyhow::{Context, Result};
use huckleberry_api::{Huckleberry, Window};

use crate::domain::normalize;
use crate::domain::types::{CollectionNote, Dataset};

/// The version of the snapshot format `export` writes.
///
/// Read back by [`read_snapshot`], which refuses a version it does not
/// understand rather than misreading it.
pub const SNAPSHOT_VERSION: u32 = 1;

/// What a snapshot file holds: the dataset, and which shape it is in.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    /// The format version.
    pub version: u32,
    /// Everything that was pulled.
    #[serde(flatten)]
    pub dataset: Dataset,
}

/// Reads everything about one child over a window.
pub async fn pull(
    client: &Huckleberry,
    cid: &str,
    nickname: Option<&str>,
    days: u32,
    timezone: &str,
    now: f64,
) -> Result<Dataset> {
    let window = Window::new(now as i64 - i64::from(days) * 86_400, now as i64);
    let mut notes = Vec::new();

    // Six independent reads. Awaiting them together turns a home connection's
    // round trips from six in series into one in parallel.
    let (profile, growth, sleep, feed, diaper, pump, milestones, sleep_live, feed_live) = tokio::join!(
        client.child(cid),
        client.latest_growth(cid),
        client.sleep_intervals(cid, window),
        client.feed_intervals(cid, window),
        client.diaper_intervals(cid, window),
        client.pump_intervals(cid, window),
        client.milestones(cid),
        client.sleep_document(cid),
        client.feed_document(cid),
    );

    let profile = note_failure(&mut notes, "profile", profile).flatten();
    let growth = note_failure(&mut notes, "growth", growth).flatten();
    let sleep_rows = note_failure(&mut notes, "sleep", sleep).unwrap_or_default();
    let feed_rows = note_failure(&mut notes, "feed", feed).unwrap_or_default();
    let diaper_rows = note_failure(&mut notes, "diaper", diaper).unwrap_or_default();
    let pump_rows = note_failure(&mut notes, "pump", pump).unwrap_or_default();
    let milestone_rows = note_failure(&mut notes, "milestones", milestones).unwrap_or_default();
    let sleep_document = note_failure(&mut notes, "sleep timer", sleep_live).flatten();
    let feed_document = note_failure(&mut notes, "feed timer", feed_live).flatten();

    Ok(Dataset {
        fetched_at: now,
        timezone: timezone.to_owned(),
        days: i64::from(days),
        child: normalize::child(cid, nickname, profile.as_ref()),
        growth: normalize::growth(growth.as_ref()),
        sleep: normalize::sleeps(&sleep_rows),
        feeds: normalize::feeds(&feed_rows),
        diapers: normalize::diapers(&diaper_rows),
        pumps: normalize::pumps(&pump_rows),
        milestones: normalize::milestones(&milestone_rows),
        live: normalize::live(sleep_document.as_ref(), feed_document.as_ref()),
        notes,
    })
}

/// Records a failed read as a note and carries on.
fn note_failure<T>(
    notes: &mut Vec<CollectionNote>,
    collection: &str,
    outcome: huckleberry_api::Result<T>,
) -> Option<T> {
    match outcome {
        Ok(value) => Some(value),
        Err(failure) => {
            notes.push(CollectionNote {
                collection: collection.to_owned(),
                problem: describe(&failure),
            });
            None
        }
    }
}

/// What to write beside a collection that could not be read.
#[must_use]
pub fn describe(failure: &huckleberry_api::Error) -> String {
    if failure.is_permission_denied() {
        return "not readable on this account".to_owned();
    }
    format!("{failure}")
}

/// Writes a snapshot, creating the directory it belongs in.
pub fn write_snapshot(path: &Path, dataset: &Dataset) -> Result<()> {
    let text = render_snapshot(dataset)?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

/// A snapshot as the JSON that goes in the file.
pub fn render_snapshot(dataset: &Dataset) -> Result<String> {
    let snapshot = Snapshot {
        version: SNAPSHOT_VERSION,
        dataset: dataset.clone(),
    };
    serde_json::to_string_pretty(&snapshot).context("serializing the snapshot")
}

/// Reads a snapshot back.
pub fn read_snapshot(path: &Path) -> Result<Dataset> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse_snapshot(&text).with_context(|| format!("reading {}", path.display()))
}

/// Reads a snapshot's text, refusing a version this build does not know.
///
/// The version is checked before anything else is decoded. A snapshot from a
/// later version is likely to have a different shape, and "this build reads
/// version 1" is a message somebody can act on where "missing field
/// `sleep_active`" is not.
pub fn parse_snapshot(text: &str) -> Result<Dataset> {
    let payload: serde_json::Value = serde_json::from_str(text).context("parsing the snapshot")?;
    let version = payload
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .context("this file has no snapshot version, so it is not one of ours")?;
    anyhow::ensure!(
        version == u64::from(SNAPSHOT_VERSION),
        "this snapshot is version {version}, and this build reads version {SNAPSHOT_VERSION}"
    );
    let snapshot: Snapshot = serde_json::from_value(payload).context("parsing the snapshot")?;
    Ok(snapshot.dataset)
}

#[cfg(test)]
mod snapshots {
    use super::*;
    use crate::domain::fixtures;

    #[test]
    fn a_snapshot_reads_back_as_what_was_written() {
        let mut dataset = fixtures::dataset();
        dataset.feeds = vec![fixtures::bottle(fixtures::AFTERNOON, 90.0)];
        let text = render_snapshot(&dataset).expect("serializing");
        assert_eq!(parse_snapshot(&text).expect("parsing"), dataset);
    }

    #[test]
    fn a_snapshot_from_a_future_version_is_refused_rather_than_misread() {
        let text = r#"{"version":99,"fetched_at":0,"timezone":"UTC","days":7,
            "child":{"cid":"c","name":"B","birthdate":null,"night_start_hour":20,
            "morning_cutoff_hour":7},"growth":null,"sleep":[],"feeds":[],"diapers":[],
            "pumps":[],"milestones":[],"live":{},"notes":[]}"#;
        let error = parse_snapshot(text).expect_err("refused");
        assert!(format!("{error:#}").contains("version 99"), "{error:#}");
    }

    #[test]
    fn the_version_is_written_beside_the_data_rather_than_inside_it() {
        let text = render_snapshot(&fixtures::dataset()).expect("serializing");
        let value: serde_json::Value = serde_json::from_str(&text).expect("parsing");
        assert_eq!(value["version"], serde_json::json!(SNAPSHOT_VERSION));
        assert!(
            value.get("child").is_some(),
            "the dataset is flattened alongside it"
        );
    }

    #[test]
    fn a_file_that_is_not_a_snapshot_fails_with_something_a_person_can_act_on() {
        let error = parse_snapshot("not json at all").expect_err("refused");
        assert!(
            format!("{error:#}").contains("parsing the snapshot"),
            "{error:#}"
        );
    }

    #[test]
    fn json_that_is_not_a_snapshot_at_all_says_so() {
        let error = parse_snapshot(r#"{"hello":"world"}"#).expect_err("refused");
        assert!(
            format!("{error:#}").contains("no snapshot version"),
            "{error:#}"
        );
    }
}

#[cfg(test)]
mod notes {
    use super::*;

    #[test]
    fn a_denial_is_reported_as_one_rather_than_as_a_status_code() {
        let denied = huckleberry_api::Error::Api {
            operation: "reading pumping history".to_owned(),
            status: 403,
            message: "Missing or insufficient permissions.".to_owned(),
        };
        assert_eq!(describe(&denied), "not readable on this account");
    }

    #[test]
    fn anything_else_keeps_its_own_message() {
        let broken = huckleberry_api::Error::Api {
            operation: "reading sleep history".to_owned(),
            status: 503,
            message: "backend unavailable".to_owned(),
        };
        assert!(describe(&broken).contains("backend unavailable"));
    }

    #[test]
    fn a_failed_read_becomes_a_note_and_the_pull_carries_on() {
        let mut collected = Vec::new();
        let outcome: huckleberry_api::Result<Vec<u8>> = Err(huckleberry_api::Error::Api {
            operation: "reading pumping history".to_owned(),
            status: 403,
            message: String::new(),
        });
        assert_eq!(note_failure(&mut collected, "pump", outcome), None);
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].collection, "pump");
    }

    #[test]
    fn a_successful_read_leaves_no_note() {
        let mut collected = Vec::new();
        let outcome: huckleberry_api::Result<u8> = Ok(7);
        assert_eq!(note_failure(&mut collected, "pump", outcome), Some(7));
        assert!(collected.is_empty());
    }
}
