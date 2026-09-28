//! Diapers and potty trips.
//!
//! One collection holds both: a potty trip is a diaper row with `isPotty` set
//! and a `howItHappened`. The app's three amount buttons are stored as the
//! numbers 0, 50 and 100, which [`quantity`] is the only place that knows.

use serde_json::{Value as Json, json};

use crate::client::{Huckleberry, now_seconds};
use crate::error::Result;
use crate::firestore::FieldUpdate;
use crate::ids;
use crate::models::common::Number;
use crate::models::diaper::{
    DiaperAmount, DiaperDocument, DiaperEntry, DiaperMode, DiaperQuantity, LastDiaper, LastPotty,
    PooColor, PooConsistency, PottyResult,
};
use crate::models::{to_fields, to_json};
use crate::paths;
use crate::rows::RowRef;

/// Everything optional about a diaper, in one argument.
///
/// A struct rather than eight parameters, so a caller cannot transpose the
/// colour and the consistency and have it compile.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DiaperDetails {
    /// How much wet.
    pub pee_amount: Option<DiaperAmount>,
    /// How much dirty.
    pub poo_amount: Option<DiaperAmount>,
    /// The colour.
    pub color: Option<PooColor>,
    /// The consistency.
    pub consistency: Option<PooConsistency>,
    /// Whether a rash was noted.
    pub rash: bool,
    /// Whatever the parent wants to write down.
    pub notes: Option<String>,
}

/// The `quantity` field for a pair of amounts, or `None` when neither was
/// given.
///
/// An amount the app has no button for contributes nothing rather than a
/// guessed number, which is what keeps a made-up value out of the record.
#[must_use]
pub fn quantity(pee: Option<&DiaperAmount>, poo: Option<&DiaperAmount>) -> Option<DiaperQuantity> {
    let recorded = DiaperQuantity {
        pee: pee.and_then(DiaperAmount::to_stored).map(Number::Float),
        poo: poo.and_then(DiaperAmount::to_stored).map(Number::Float),
    };
    (!recorded.is_empty()).then_some(recorded)
}

impl Huckleberry {
    /// Records a diaper change.
    ///
    /// # Errors
    ///
    /// As every write: a refused or unreachable Firestore.
    pub async fn log_diaper(
        &self,
        cid: &str,
        mode: DiaperMode,
        details: &DiaperDetails,
    ) -> Result<()> {
        self.log_diaper_at(cid, mode, details, now_seconds()).await
    }

    /// Performs this operation at a Unix timestamp in seconds.
    /// Synchronization timestamps still describe the current write.
    ///
    /// # Errors
    ///
    /// Invalid event times or a refused or unreachable Firestore.
    pub async fn log_diaper_at(
        &self,
        cid: &str,
        mode: DiaperMode,
        details: &DiaperDetails,
        at: f64,
    ) -> Result<()> {
        self.log_diaper_event(cid, mode, details, None, at).await
    }

    /// Records a potty trip, in the same tracker.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::log_diaper`].
    pub async fn log_potty(
        &self,
        cid: &str,
        mode: DiaperMode,
        how_it_happened: PottyResult,
        details: &DiaperDetails,
    ) -> Result<()> {
        self.log_potty_at(cid, mode, how_it_happened, details, now_seconds())
            .await
    }

    /// Performs this operation at a Unix timestamp in seconds.
    /// Synchronization timestamps still describe the current write.
    ///
    /// # Errors
    ///
    /// Invalid event times or a refused or unreachable Firestore.
    pub async fn log_potty_at(
        &self,
        cid: &str,
        mode: DiaperMode,
        how_it_happened: PottyResult,
        details: &DiaperDetails,
        at: f64,
    ) -> Result<()> {
        self.log_diaper_event(cid, mode, details, Some(how_it_happened), at)
            .await
    }

    /// Changes a diaper or a potty trip that is already on the record.
    ///
    /// What was in it and everything said about it, and nothing else: the row
    /// keeps the moment it happened, because a row's id leads with its own
    /// timestamp and moving one would leave the collection ordered by a time
    /// the row no longer claims. A detail left out of `details` is removed
    /// from the row rather than left behind.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::update_history_row`], which includes the row having
    /// been deleted since it was read.
    pub async fn update_diaper_entry(
        &self,
        cid: &str,
        at: &RowRef,
        mode: DiaperMode,
        details: &DiaperDetails,
        how_it_happened: Option<PottyResult>,
    ) -> Result<()> {
        let quantity = quantity(details.pee_amount.as_ref(), details.poo_amount.as_ref())
            .map(|recorded| to_json(&recorded))
            .transpose()?;
        let mut updates = vec![
            FieldUpdate::set("mode", json!(mode.as_str())),
            FieldUpdate::set_or_clear("quantity", quantity),
            FieldUpdate::set_or_clear(
                "color",
                details.color.as_ref().map(|shade| json!(shade.as_str())),
            ),
            FieldUpdate::set_or_clear(
                "consistency",
                details
                    .consistency
                    .as_ref()
                    .map(|texture| json!(texture.as_str())),
            ),
            // As on a new row: the app reads the field's absence as "no rash",
            // and a stored `false` is a different thing to it.
            FieldUpdate::set_or_clear("diaperRash", details.rash.then_some(Json::Bool(true))),
            FieldUpdate::set_or_clear("notes", details.notes.as_ref().map(|text| json!(text))),
            FieldUpdate::set("lastUpdated", json!(now_seconds())),
        ];
        if let Some(outcome) = how_it_happened {
            updates.push(FieldUpdate::set("howItHappened", json!(outcome.as_str())));
        }
        self.update_history_row(cid, at, &updates, "changing the diaper")
            .await
    }

    /// The diaper tracker's document, which carries the last of each.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::log_diaper`].
    pub async fn diaper_document(&self, cid: &str) -> Result<Option<DiaperDocument>> {
        self.document(
            &paths::tracker(paths::DIAPER, cid),
            "reading the diaper tracker",
        )
        .await
    }

    /// The one write both of the above are.
    async fn log_diaper_event(
        &self,
        cid: &str,
        mode: DiaperMode,
        details: &DiaperDetails,
        how_it_happened: Option<PottyResult>,
        at: f64,
    ) -> Result<()> {
        let is_potty = how_it_happened.is_some();
        let operation = if is_potty {
            "recording the potty trip"
        } else {
            "recording the diaper"
        };
        super::timing::validate_time(at, None)?;
        let replaces = self
            .replaces_summary(
                paths::DIAPER,
                cid,
                if is_potty { "lastPotty" } else { "lastDiaper" },
                at,
            )
            .await?;
        let now = now_seconds();
        let offset = self.zone().offset_minutes(at);

        let entry = DiaperEntry {
            mode: mode.clone(),
            start: Number::Float(at),
            last_updated: Some(Number::Float(now)),
            offset: Number::Float(offset),
            quantity: quantity(details.pee_amount.as_ref(), details.poo_amount.as_ref()),
            color: details.color.clone(),
            consistency: details.consistency.clone(),
            // Written only when true: the app treats the field's absence as
            // "no rash", and a stored `false` is a different thing to it.
            diaper_rash: details.rash.then_some(true),
            notes: details.notes.clone(),
            is_potty: is_potty.then_some(true),
            how_it_happened,
        };

        let token = self.token().await?;
        self.firestore()
            .set(
                &token,
                &paths::history_row(paths::DIAPER, cid, &ids::interval_id(at)),
                &to_fields(&entry)?,
                operation,
            )
            .await?;

        if !replaces {
            return Ok(());
        }

        let summary = if is_potty {
            to_json(&LastPotty {
                mode: Some(mode),
                start: Some(Number::Float(at)),
                offset: Some(Number::Float(offset)),
            })?
        } else {
            to_json(&LastDiaper {
                start: Some(Number::Float(at)),
                mode: Some(mode),
                offset: Some(Number::Float(offset)),
            })?
        };
        let field = if is_potty {
            "prefs.lastPotty"
        } else {
            "prefs.lastDiaper"
        };

        self.firestore()
            .update(
                &token,
                &paths::tracker(paths::DIAPER, cid),
                &[
                    FieldUpdate::set(field, summary),
                    FieldUpdate::set("prefs.timestamp", json!({ "seconds": now })),
                    FieldUpdate::set("prefs.local_timestamp", json!(now)),
                ],
                operation,
            )
            .await
    }
}

#[cfg(test)]
mod quantities {
    use super::*;

    #[test]
    fn nothing_given_writes_no_quantity_at_all() {
        assert_eq!(quantity(None, None), None);
    }

    #[test]
    fn one_amount_is_enough_to_write_the_field() {
        let recorded = quantity(Some(&DiaperAmount::Big), None).expect("a quantity");
        assert_eq!(recorded.pee, Some(Number::Float(100.0)));
        assert_eq!(recorded.poo, None);
    }

    #[test]
    fn the_three_buttons_store_the_three_numbers_the_app_writes() {
        let recorded =
            quantity(Some(&DiaperAmount::Little), Some(&DiaperAmount::Medium)).expect("a quantity");
        assert_eq!(recorded.pee, Some(Number::Float(0.0)));
        assert_eq!(recorded.poo, Some(Number::Float(50.0)));
    }

    #[test]
    fn an_amount_the_app_has_no_button_for_contributes_nothing() {
        let unknown = DiaperAmount::Unknown("enormous".to_owned());
        assert_eq!(quantity(Some(&unknown), None), None);
    }
}
