//! Taking a row of history away, and tidying up after it.
//!
//! A delete is two writes, and the second one is the part that is easy to
//! forget. Every tracker keeps a copy of its most recent entry on its own
//! document (`prefs.lastDiaper`, `prefs.lastBottle`, `prefs.lastGrowthEntry`
//! and the rest) so the app can draw a home screen without reading history.
//! Remove the row those copies describe and the app goes on showing an entry
//! that is no longer there.
//!
//! So a delete removes the row and then, for any summary that pointed at it,
//! writes the summary of whatever is now the most recent row of that kind, or
//! takes the summary away when there is nothing left. Which rows can fill
//! which summary, and which of their fields it copies, is the table below:
//! declaring it is what keeps six trackers' worth of this from being six
//! hand-written repairs.

use serde_json::{Map, Value as Json};

use crate::client::Huckleberry;
use crate::error::Result;
use crate::firestore::FieldUpdate;
use crate::paths;
use crate::rows::RowRef;

/// How close two `start` values have to be to be the same moment.
///
/// A summary is a copy of the row, so the numbers are the same number; this
/// is only insurance against a float that has been through a string.
const SAME_MOMENT: f64 = 0.001;

/// Which rows of a tracker can fill one of its summaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fills {
    /// Any row of the tracker.
    Any,
    /// A row whose `mode` is this word.
    Mode(&'static str),
    /// A diaper, or a potty trip.
    Potty(bool),
}

impl Fills {
    /// Whether a row can fill this summary.
    #[must_use]
    pub fn matches(self, row: &Json) -> bool {
        match self {
            Self::Any => true,
            Self::Mode(wanted) => row.get("mode").and_then(Json::as_str) == Some(wanted),
            Self::Potty(wanted) => (row.get("isPotty") == Some(&Json::Bool(true))) == wanted,
        }
    }
}

/// One of a tracker document's copies of its most recent entry.
#[derive(Debug, Clone, Copy)]
pub struct Summary {
    /// The field it lives at.
    pub field: &'static str,
    /// Which rows can fill it.
    pub fills: Fills,
    /// The summary's keys, and the row keys they are copied from. Empty means
    /// the row itself, which is what health's summaries are.
    pub keys: &'static [(&'static str, &'static str)],
}

/// The only field a whole-row summary leaves behind.
///
/// Everything else is copied, including `_id` and `lastUpdated`: health's
/// summaries *are* the entry, and [`crate::models::health::GrowthEntry`]
/// requires `lastUpdated`, so dropping it writes a summary nothing can read
/// back.
const PRIVATE: [&str; 1] = ["multi"];

/// Every summary a tracker keeps, or nothing for a tracker that keeps none.
#[must_use]
pub fn summaries_for(tracker: &str) -> &'static [Summary] {
    const DIAPER: &[Summary] = &[
        Summary {
            field: "prefs.lastDiaper",
            fills: Fills::Potty(false),
            keys: &[("start", "start"), ("mode", "mode"), ("offset", "offset")],
        },
        Summary {
            field: "prefs.lastPotty",
            fills: Fills::Potty(true),
            keys: &[("mode", "mode"), ("start", "start"), ("offset", "offset")],
        },
    ];
    const FEED: &[Summary] = &[
        Summary {
            field: "prefs.lastBottle",
            fills: Fills::Mode("bottle"),
            keys: &[
                ("mode", "mode"),
                ("start", "start"),
                ("bottleType", "bottleType"),
                ("bottleAmount", "amount"),
                ("bottleUnits", "units"),
                ("offset", "offset"),
            ],
        },
        Summary {
            field: "prefs.lastNursing",
            fills: Fills::Mode("breast"),
            keys: &[
                ("mode", "mode"),
                ("start", "start"),
                ("leftDuration", "leftDuration"),
                ("rightDuration", "rightDuration"),
                ("offset", "offset"),
            ],
        },
        Summary {
            field: "prefs.lastSolid",
            fills: Fills::Mode("solids"),
            keys: &[
                ("mode", "mode"),
                ("start", "start"),
                ("foods", "foods"),
                ("reactions", "reactions"),
                ("notes", "notes"),
                ("offset", "offset"),
            ],
        },
    ];
    const SLEEP: &[Summary] = &[Summary {
        field: "prefs.lastSleep",
        fills: Fills::Any,
        keys: &[
            ("start", "start"),
            ("duration", "duration"),
            ("offset", "offset"),
        ],
    }];
    // Health's summaries are the entry itself, which is why they have no key
    // map: `lastGrowthEntry` deserializes as a `GrowthEntry`.
    const HEALTH: &[Summary] = &[
        Summary {
            field: "prefs.lastGrowthEntry",
            fills: Fills::Mode("growth"),
            keys: &[],
        },
        Summary {
            field: "prefs.lastMedication",
            fills: Fills::Mode("medication"),
            keys: &[],
        },
        Summary {
            field: "prefs.lastTemperature",
            fills: Fills::Mode("temperature"),
            keys: &[],
        },
    ];

    match tracker {
        paths::DIAPER => DIAPER,
        paths::FEED => FEED,
        paths::SLEEP => SLEEP,
        paths::HEALTH => HEALTH,
        _ => &[],
    }
}

/// What a document holds at a dotted path, if anything.
fn at_path<'a>(document: &'a Json, field: &str) -> Option<&'a Json> {
    let mut here = document;
    for segment in field.split('.') {
        here = here.get(segment)?;
    }
    Some(here)
}

/// Whether a summary describes the row that started at this moment.
#[must_use]
pub fn points_at(document: &Json, field: &str, started_at: f64) -> bool {
    at_path(document, field)
        .and_then(|summary| summary.get("start"))
        .and_then(Json::as_f64)
        .is_some_and(|start| (start - started_at).abs() < SAME_MOMENT)
}

/// Whether a summary needs writing again after a row was removed.
///
/// Either it described that row, or it is not there at all while history says
/// it should be. The second case is what makes a delete self-healing rather
/// than only tidy: a summary written wrong or lost earlier is put right the
/// next time somebody removes an entry from that tracker.
#[must_use]
pub fn needs_rewriting(document: &Json, field: &str, started_at: f64, has_rows: bool) -> bool {
    points_at(document, field, started_at)
        || (has_rows && at_path(document, field).is_none_or(Json::is_null))
}

/// The newest row that can fill a summary, if any is left.
#[must_use]
pub fn newest_filling(rows: &[(RowRef, Json)], fills: Fills) -> Option<&Json> {
    rows.iter()
        .map(|(_, row)| row)
        .filter(|row| fills.matches(row))
        .max_by(|left, right| {
            start_of(left)
                .partial_cmp(&start_of(right))
                .unwrap_or(core::cmp::Ordering::Equal)
        })
}

/// A row's `start`, or the beginning of time when it has none.
fn start_of(row: &Json) -> f64 {
    row.get("start").and_then(Json::as_f64).unwrap_or(f64::MIN)
}

/// One row as one summary of it.
///
/// A key the row does not carry is left out rather than written as null: the
/// summary is what the app reads, and a null where it expects a number reads
/// worse than an absence.
#[must_use]
pub fn summary_of(row: &Json, keys: &[(&str, &str)]) -> Json {
    if keys.is_empty() {
        let kept: Map<String, Json> = row
            .as_object()
            .map(|fields| {
                fields
                    .iter()
                    .filter(|(key, _)| !PRIVATE.contains(&key.as_str()))
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect()
            })
            .unwrap_or_default();
        return Json::Object(kept);
    }
    let mut summary = Map::new();
    for (into, from) in keys {
        if let Some(value) = row.get(*from) {
            summary.insert((*into).to_owned(), value.clone());
        }
    }
    Json::Object(summary)
}

impl Huckleberry {
    /// Removes one row of history, and puts the tracker's summaries back.
    ///
    /// `started_at` is the moment the row claimed, which is how a summary is
    /// recognised as a copy of it. Any summary that was describing this row is
    /// rewritten from whatever is now the most recent row of that kind, or
    /// taken away when there is none left, so nothing goes on pointing at an
    /// entry that is no longer there.
    ///
    /// Deleting a row that is already gone is not a failure.
    ///
    /// # Errors
    ///
    /// [`crate::Error::Api`] when Firestore refuses the write, and
    /// [`crate::Error::Network`] when it cannot be reached.
    pub async fn delete_history_row(&self, cid: &str, at: &RowRef, started_at: f64) -> Result<()> {
        let operation = "removing the entry";
        let token = self.token().await?;
        match &at.batch_key {
            // A row of its own is a document of its own.
            None => {
                self.firestore()
                    .remove(&token, &at.document_path(cid), operation)
                    .await?;
            }
            // A row inside a batch is one key of one document, and the
            // document keeps its other rows.
            Some(key) => {
                let mut path = vec!["data".to_owned()];
                path.push(key.clone());
                self.firestore()
                    .update(
                        &token,
                        &at.document_path(cid),
                        &[FieldUpdate { path, value: None }],
                        operation,
                    )
                    .await?;
            }
        }
        self.repair_summaries(cid, &at.tracker, started_at).await
    }

    /// Rewrites any of a tracker's summaries that described a row that is no
    /// longer there.
    ///
    /// Called by [`Huckleberry::delete_history_row`], and public because a
    /// summary can be left pointing at nothing by anything that removes a row,
    /// including the app itself.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::delete_history_row`].
    pub async fn repair_summaries(&self, cid: &str, tracker: &str, started_at: f64) -> Result<()> {
        let operation = "putting the tracker's last entry back";
        let summaries = summaries_for(tracker);
        if summaries.is_empty() {
            return Ok(());
        }

        let token = self.token().await?;
        let path = paths::tracker(tracker, cid);
        let Some(document) = self.firestore().get(&token, &path, operation).await? else {
            return Ok(());
        };
        let document = document.into_json();
        // One listing, because deciding whether a missing summary should be
        // there at all is a question about history.
        let rows = self.located_rows(tracker, cid, operation).await?;

        let updates: Vec<FieldUpdate> = summaries
            .iter()
            .filter_map(|summary| {
                let newest = newest_filling(&rows, summary.fills);
                if !needs_rewriting(&document, summary.field, started_at, newest.is_some()) {
                    return None;
                }
                Some(newest.map_or_else(
                    || FieldUpdate::delete(summary.field),
                    |row| FieldUpdate::set(summary.field, summary_of(row, summary.keys)),
                ))
            })
            .collect();
        if updates.is_empty() {
            return Ok(());
        }
        self.firestore()
            .update(&token, &path, &updates, operation)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn diaper(start: f64, potty: bool) -> Json {
        let mut row = json!({ "mode": "pee", "start": start, "offset": -240.0 });
        if potty {
            row["isPotty"] = json!(true);
        }
        row
    }

    fn rows(rows: Vec<Json>) -> Vec<(RowRef, Json)> {
        rows.into_iter()
            .enumerate()
            .map(|(position, row)| (RowRef::loose("diaper", &format!("row-{position}")), row))
            .collect()
    }

    #[test]
    fn a_tracker_with_no_summaries_needs_no_repair() {
        assert!(summaries_for("pump").is_empty());
        assert!(summaries_for("milestones").is_empty());
        assert_eq!(summaries_for(paths::SLEEP).len(), 1);
    }

    #[test]
    fn a_summary_that_is_not_there_is_written_when_history_has_one_to_write() {
        let empty = json!({ "prefs": {} });
        assert!(
            needs_rewriting(&empty, "prefs.lastDiaper", 1_000.0, true),
            "a summary history can fill and the document has not"
        );
        assert!(
            !needs_rewriting(&empty, "prefs.lastDiaper", 1_000.0, false),
            "and nothing is written when there is no row to describe"
        );
    }

    #[test]
    fn a_summary_describing_something_else_is_left_where_it_is() {
        let other = json!({ "prefs": { "lastDiaper": { "start": 5_000.0 } } });
        assert!(!needs_rewriting(&other, "prefs.lastDiaper", 1_000.0, true));
    }

    #[test]
    fn a_summary_is_recognised_as_a_copy_of_the_row_it_describes() {
        let document = json!({ "prefs": { "lastDiaper": { "start": 1_000.5, "mode": "pee" } } });
        assert!(points_at(&document, "prefs.lastDiaper", 1_000.5));
        assert!(!points_at(&document, "prefs.lastDiaper", 999.0));
        assert!(
            !points_at(&document, "prefs.lastPotty", 1_000.5),
            "a summary that is not there describes nothing"
        );
    }

    #[test]
    fn a_diaper_and_a_potty_trip_fill_different_summaries() {
        let history = rows(vec![diaper(100.0, false), diaper(200.0, true)]);
        let last_diaper = newest_filling(&history, Fills::Potty(false)).expect("a diaper");
        assert_eq!(last_diaper["start"], json!(100.0));
        let last_potty = newest_filling(&history, Fills::Potty(true)).expect("a potty trip");
        assert_eq!(last_potty["start"], json!(200.0));
    }

    #[test]
    fn the_newest_row_wins_whatever_order_they_arrive_in() {
        let history = rows(vec![
            diaper(300.0, false),
            diaper(100.0, false),
            diaper(200.0, false),
        ]);
        let newest = newest_filling(&history, Fills::Any).expect("a row");
        assert_eq!(newest["start"], json!(300.0));
    }

    #[test]
    fn nothing_of_that_kind_left_is_no_summary_rather_than_a_wrong_one() {
        let history = rows(vec![diaper(100.0, false)]);
        assert!(newest_filling(&history, Fills::Potty(true)).is_none());
        assert!(newest_filling(&[], Fills::Any).is_none());
    }

    #[test]
    fn a_summary_takes_the_keys_it_is_made_of_and_renames_what_the_app_renames() {
        let bottle = json!({
            "mode": "bottle", "start": 100.0, "bottleType": "Formula",
            "amount": 90.0, "units": "ml", "offset": -240.0, "notes": "all of it",
        });
        let summary = summary_of(&bottle, summaries_for(paths::FEED)[0].keys);
        assert_eq!(summary["bottleAmount"], json!(90.0), "`amount` on the row");
        assert_eq!(summary["bottleUnits"], json!("ml"), "`units` on the row");
        assert_eq!(summary["bottleType"], json!("Formula"));
        assert!(
            summary.get("notes").is_none(),
            "a summary is the keys it is made of and no more: {summary}"
        );
    }

    #[test]
    fn a_key_the_row_does_not_have_is_left_out_rather_than_written_as_null() {
        let sparse = json!({ "start": 100.0 });
        let summary = summary_of(&sparse, summaries_for(paths::SLEEP)[0].keys);
        assert_eq!(summary["start"], json!(100.0));
        assert!(summary.get("duration").is_none(), "{summary}");
    }

    #[test]
    fn a_health_summary_is_the_entry_itself() {
        // The trap: `GrowthEntry` requires `lastUpdated`, so a summary written
        // without it is one the app and this crate both fail to read back, and
        // the growth card goes blank rather than showing the entry before.
        let growth = json!({
            "mode": "growth", "start": 100.0, "weight": 3.6, "weightUnits": "kg",
            "_id": "abc", "lastUpdated": 100.0,
        });
        let summary = summary_of(&growth, &[]);
        assert_eq!(summary["weight"], json!(3.6));
        assert_eq!(summary["lastUpdated"], json!(100.0));
        assert_eq!(summary["_id"], json!("abc"));
    }

    #[test]
    fn a_whole_row_summary_leaves_the_batch_marker_behind() {
        let row = json!({ "mode": "growth", "start": 100.0, "multi": true });
        assert!(summary_of(&row, &[]).get("multi").is_none());
    }

    #[test]
    fn every_whole_row_summary_a_tracker_keeps_can_be_read_back_as_its_model() {
        // What the bug above cost: the repair wrote a summary, and the model
        // it is read back through could not decode it.
        let growth = json!({
            "mode": "growth", "start": 1_789_471_843.0, "offset": 240.0,
            "lastUpdated": 1_789_681_221.499, "weight": 7.25, "weightUnits": "lbs.oz",
        });
        let summary = summary_of(&growth, &[]);
        let decoded: std::result::Result<crate::models::health::GrowthEntry, _> =
            serde_json::from_value(summary);
        assert!(decoded.is_ok(), "{decoded:?}");
    }
}
