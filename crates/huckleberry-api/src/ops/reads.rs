//! Reading: the account, a child, and the history of any tracker.
//!
//! Every range read here is two queries, and the reason is structural. When a
//! subcollection gets long, Huckleberry packs older rows into a single
//! document with `multi: true` and the rows nested under `data`. Firestore
//! indexes fields, not map entries, so those nested rows cannot be filtered by
//! a `where` clause: the only way to get them is to fetch every batch and
//! filter in memory. The loose rows are filtered properly, which is what keeps
//! the first query cheap as history grows.

use serde::de::DeserializeOwned;
use serde_json::{Value as Json, json};

use crate::client::Huckleberry;
use crate::error::{Error, Result};
use crate::firestore::{Document, Op, Query};
use crate::models::child::ChildDocument;
use crate::models::common::MultiContainer;
use crate::models::diaper::DiaperEntry;
use crate::models::feed::FeedInterval;
use crate::models::health::{GrowthEntry, HealthDocument, HealthEntry};
use crate::models::milestone::Milestone;
use crate::models::pump::PumpInterval;
use crate::models::sleep::SleepInterval;
use crate::models::user::UserDocument;
use crate::paths;
use crate::rows::{Located, RowRef};

/// A window of history, in Unix seconds.
///
/// Half-open: a row exactly on `start` is inside and a row exactly on `end` is
/// not, so consecutive windows tile without counting a row twice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// The first second inside the window.
    pub start: i64,
    /// The first second after it.
    pub end: i64,
}

impl Window {
    /// A window between two instants.
    #[must_use]
    pub const fn new(start: i64, end: i64) -> Self {
        Self { start, end }
    }

    /// The `days` days up to `now`.
    #[must_use]
    pub const fn last_days(now: f64, days: i64) -> Self {
        let end = now as i64;
        Self {
            start: end - days * 86_400,
            end,
        }
    }

    /// Whether an instant falls inside.
    #[must_use]
    pub const fn contains(&self, at: f64) -> bool {
        at >= self.start as f64 && at < self.end as f64
    }
}

impl Huckleberry {
    /// The signed-in account, with the children on it.
    ///
    /// # Errors
    ///
    /// Whatever signing in would fail with, or [`Error::Decode`] when the
    /// account document is missing.
    pub async fn user(&self) -> Result<UserDocument> {
        let uid = self.user_uid().await?;
        self.document(&paths::user(&uid), "reading the account")
            .await?
            .ok_or_else(|| Error::decode("reading the account", "no account document"))
    }

    /// One child's profile, or `None` when there is no such child.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::user`].
    pub async fn child(&self, cid: &str) -> Result<Option<ChildDocument>> {
        self.document(&paths::child(cid), "reading the child profile")
            .await
    }

    /// The last growth measurement, read off the health document rather than
    /// out of its history.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::user`].
    pub async fn latest_growth(&self, cid: &str) -> Result<Option<GrowthEntry>> {
        let health: Option<HealthDocument> = self
            .document(
                &paths::tracker(paths::HEALTH, cid),
                "reading the latest growth",
            )
            .await?;
        Ok(health
            .and_then(|document| document.prefs)
            .and_then(|prefs| prefs.last_growth_entry))
    }

    /// Sleeps that began inside the window.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::user`].
    pub async fn sleep_intervals(
        &self,
        cid: &str,
        window: Window,
    ) -> Result<Vec<Located<SleepInterval>>> {
        self.history(paths::SLEEP, cid, window, "reading sleep history")
            .await
    }

    /// Feeds that began inside the window: nursing, bottles and solids alike.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::user`].
    pub async fn feed_intervals(
        &self,
        cid: &str,
        window: Window,
    ) -> Result<Vec<Located<FeedInterval>>> {
        self.history(paths::FEED, cid, window, "reading feed history")
            .await
    }

    /// Diapers and potty trips inside the window.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::user`].
    pub async fn diaper_intervals(
        &self,
        cid: &str,
        window: Window,
    ) -> Result<Vec<Located<DiaperEntry>>> {
        self.history(paths::DIAPER, cid, window, "reading diaper history")
            .await
    }

    /// Growth, medication and temperature entries inside the window.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::user`].
    pub async fn health_entries(
        &self,
        cid: &str,
        window: Window,
    ) -> Result<Vec<Located<HealthEntry>>> {
        self.history(paths::HEALTH, cid, window, "reading health history")
            .await
    }

    /// Pumping sessions inside the window.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::user`].
    pub async fn pump_intervals(
        &self,
        cid: &str,
        window: Window,
    ) -> Result<Vec<Located<PumpInterval>>> {
        self.history(paths::PUMP, cid, window, "reading pumping history")
            .await
    }

    /// Every milestone on record.
    ///
    /// Not windowed: a milestone is a memory rather than a measurement, and
    /// there are few enough of them that a family wants all of them.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::user`].
    pub async fn milestones(&self, cid: &str) -> Result<Vec<Milestone>> {
        Ok(self
            .collection_rows(paths::MILESTONES, cid, "reading milestones")
            .await?
            .into_iter()
            .filter_map(|row| serde_json::from_value(row).ok())
            .collect())
    }

    /// One tracker's own document, untyped.
    ///
    /// The escape hatch beside [`Huckleberry::collection_rows`]: a caller that
    /// wants a preference this crate does not model, or that wants to see what
    /// a summary actually holds, reads it here.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::user`].
    pub async fn tracker_document(&self, tracker: &str, cid: &str) -> Result<Option<Json>> {
        let token = self.token().await?;
        Ok(self
            .firestore()
            .get(&token, &paths::tracker(tracker, cid), "reading the tracker")
            .await?
            .map(Document::into_json))
    }

    /// Every row of a tracker's history, untyped and unwindowed.
    ///
    /// The escape hatch: a caller that wants a field this crate does not model
    /// reads it here. Batched documents are expanded, so one row in equals one
    /// row out, and the result is sorted by `start`.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::user`].
    pub async fn collection_rows(
        &self,
        tracker: &str,
        cid: &str,
        operation: &str,
    ) -> Result<Vec<Json>> {
        Ok(self
            .located_rows(tracker, cid, operation)
            .await?
            .into_iter()
            .map(|(_, row)| row)
            .collect())
    }

    /// Every row of a tracker's history, each with the place it came from.
    ///
    /// As [`Huckleberry::collection_rows`], and the answer to "which document
    /// is this row actually in", which is what an edit needs and a decoded row
    /// does not carry.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::user`].
    pub async fn located_rows(
        &self,
        tracker: &str,
        cid: &str,
        operation: &str,
    ) -> Result<Vec<(RowRef, Json)>> {
        let token = self.token().await?;
        let documents = self
            .firestore()
            .list(
                &token,
                &paths::tracker(tracker, cid),
                paths::history_collection(tracker),
                operation,
            )
            .await?;

        let mut rows = Vec::new();
        for document in documents {
            rows.extend(expand(tracker, document));
        }
        sort_by_start(&mut rows);
        Ok(rows)
    }

    /// One document, deserialized, or `None` when it does not exist.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::user`].
    pub(crate) async fn document<T: DeserializeOwned>(
        &self,
        path: &str,
        operation: &str,
    ) -> Result<Option<T>> {
        let token = self.token().await?;
        let found = self.firestore().get(&token, path, operation).await?;
        found
            .map(|document| {
                serde_json::from_value(document.into_json())
                    .map_err(|error| Error::decode(operation, error))
            })
            .transpose()
    }

    /// The loose rows and the batched rows of one tracker's history, merged,
    /// windowed, and sorted by when they started.
    ///
    /// Rows this crate cannot decode are skipped rather than failing the read:
    /// one unfamiliar row must not cost the caller a month of history. Use
    /// [`Huckleberry::collection_rows`] to see everything.
    async fn history<T: DeserializeOwned>(
        &self,
        tracker: &str,
        cid: &str,
        window: Window,
        operation: &str,
    ) -> Result<Vec<Located<T>>> {
        let token = self.token().await?;
        let parent = paths::tracker(tracker, cid);
        let collection = paths::history_collection(tracker);

        let loose = Query::on(collection)
            .filter("start", Op::GreaterThanOrEqual, json!(window.start))
            .filter("start", Op::LessThan, json!(window.end))
            .order_by("start");
        let batched = Query::on(collection).filter("multi", Op::Equal, json!(true));

        let mut rows: Vec<(RowRef, Json)> = self
            .firestore()
            .query(&token, &parent, &loose, operation)
            .await?
            .into_iter()
            .filter(|document| !is_batch_document(document))
            .map(|document| (RowRef::loose(tracker, &document.id), document.into_json()))
            .collect();

        for document in self
            .firestore()
            .query(&token, &parent, &batched, operation)
            .await?
        {
            rows.extend(
                expand(tracker, document)
                    .into_iter()
                    .filter(|(_, row)| start_of(row).is_some_and(|start| window.contains(start))),
            );
        }

        sort_by_start(&mut rows);
        Ok(decode_rows(rows))
    }
}

/// Whether a row is one of the batched containers.
fn is_batch(row: &Json) -> bool {
    row.get("multi") == Some(&Json::Bool(true))
}

/// The same question of a document that has not been unwrapped yet.
fn is_batch_document(document: &Document) -> bool {
    document.fields.get("multi") == Some(&Json::Bool(true))
}

/// One document into the rows it holds, each with the place it came from:
/// itself, or the entries inside the batch.
fn expand(tracker: &str, document: Document) -> Vec<(RowRef, Json)> {
    let id = document.id.clone();
    let contents = document.into_json();
    if !is_batch(&contents) {
        return vec![(RowRef::loose(tracker, &id), contents)];
    }
    serde_json::from_value::<MultiContainer<Json>>(contents)
        .map(|container| {
            container
                .data
                .into_iter()
                .map(|(key, row)| (RowRef::batched(tracker, &id, &key), row))
                .collect()
        })
        .unwrap_or_default()
}

/// A row's `start`, when it has a readable one.
fn start_of(row: &Json) -> Option<f64> {
    row.get("start").and_then(Json::as_f64)
}

/// Oldest first, which is the order every caller in this crate wants and the
/// order the two queries do not arrive in.
fn sort_by_start(rows: &mut [(RowRef, Json)]) {
    rows.sort_by(|left, right| {
        start_of(&left.1)
            .unwrap_or(f64::MIN)
            .partial_cmp(&start_of(&right.1).unwrap_or(f64::MIN))
            .unwrap_or(core::cmp::Ordering::Equal)
    });
}

/// Decodes what decodes, drops what does not, keeping each row's place.
fn decode_rows<T: DeserializeOwned>(rows: Vec<(RowRef, Json)>) -> Vec<Located<T>> {
    rows.into_iter()
        .filter_map(|(at, row)| {
            serde_json::from_value(row)
                .ok()
                .map(|decoded| Located::new(at, decoded))
        })
        .collect()
}

#[cfg(test)]
mod windows {
    use super::*;

    #[test]
    fn a_window_is_half_open_so_two_of_them_tile() {
        let window = Window::new(100, 200);
        assert!(window.contains(100.0));
        assert!(window.contains(199.9));
        assert!(!window.contains(200.0));
        assert!(!window.contains(99.9));
    }

    #[test]
    fn a_window_of_days_ends_now() {
        let window = Window::last_days(1_000_000.0, 7);
        assert_eq!(window.end, 1_000_000);
        assert_eq!(window.start, 1_000_000 - 7 * 86_400);
    }
}

#[cfg(test)]
mod row_handling {
    use super::*;
    use crate::firestore::Document;

    fn document(id: &str, fields: &Json) -> Document {
        Document {
            update_time: None,
            id: id.to_owned(),
            fields: fields.as_object().expect("an object").clone(),
        }
    }

    #[test]
    fn an_ordinary_document_is_one_row_that_knows_where_it_lives() {
        let rows = expand(
            "diaper",
            document("row-one", &json!({ "start": 1.0, "mode": "pee" })),
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, RowRef::loose("diaper", "row-one"));
    }

    #[test]
    fn a_batch_becomes_the_rows_inside_it_each_under_its_own_key() {
        let rows = expand(
            "diaper",
            document(
                "pack",
                &json!({
                    "multi": true,
                    "data": {
                        "a": { "start": 2.0, "mode": "pee" },
                        "b": { "start": 1.0, "mode": "poo" },
                    },
                }),
            ),
        );
        assert_eq!(rows.len(), 2);
        let keys: Vec<Option<&str>> = rows.iter().map(|(at, _)| at.batch_key.as_deref()).collect();
        assert!(
            keys.contains(&Some("a")) && keys.contains(&Some("b")),
            "{keys:?}"
        );
        assert!(rows.iter().all(|(at, _)| at.document_id == "pack"));
    }

    #[test]
    fn a_batch_that_does_not_parse_yields_nothing_rather_than_panicking() {
        assert!(
            expand(
                "diaper",
                document("pack", &json!({ "multi": true, "data": 7 }))
            )
            .is_empty()
        );
    }

    #[test]
    fn rows_come_out_oldest_first() {
        let mut rows = vec![
            (RowRef::loose("diaper", "c"), json!({ "start": 30 })),
            (RowRef::loose("diaper", "a"), json!({ "start": 10 })),
            (RowRef::loose("diaper", "b"), json!({ "start": 20 })),
        ];
        sort_by_start(&mut rows);
        let starts: Vec<f64> = rows.iter().filter_map(|(_, row)| start_of(row)).collect();
        assert_eq!(starts, vec![10.0, 20.0, 30.0]);
    }

    #[test]
    fn a_row_with_no_start_sorts_first_rather_than_being_lost() {
        let mut rows = vec![
            (RowRef::loose("diaper", "a"), json!({ "start": 10 })),
            (RowRef::loose("diaper", "b"), json!({ "note": "no start" })),
        ];
        sort_by_start(&mut rows);
        assert!(rows[0].1.get("start").is_none());
    }

    #[test]
    fn a_row_this_crate_cannot_read_is_skipped_and_the_rest_survive() {
        let rows = vec![
            (
                RowRef::loose("diaper", "a"),
                json!({ "mode": "pee", "start": 1.0, "offset": 0.0 }),
            ),
            (RowRef::loose("diaper", "b"), json!({ "mode": "pee" })),
            (
                RowRef::loose("diaper", "c"),
                json!({ "mode": "poo", "start": 2.0, "offset": 0.0 }),
            ),
        ];
        let decoded: Vec<Located<DiaperEntry>> = decode_rows(rows);
        assert_eq!(
            decoded.len(),
            2,
            "the row with no start went, the others stayed"
        );
        assert_eq!(decoded[1].at.document_id, "c");
    }
}
