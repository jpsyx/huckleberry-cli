//! Nappies and potty trips.
//!
//! One collection holds both: a potty trip is a nappy row with `isPotty` set
//! and a `howItHappened`. The app's three amount buttons are stored as the
//! numbers 0, 50 and 100, which [`quantity`] is the only place that knows.

use serde_json::json;

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

/// Everything optional about a nappy, in one argument.
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
    /// Records a nappy change.
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
        self.log_diaper_event(cid, mode, details, None).await
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
        self.log_diaper_event(cid, mode, details, Some(how_it_happened))
            .await
    }

    /// The nappy tracker's document, which carries the last of each.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::log_diaper`].
    pub async fn diaper_document(&self, cid: &str) -> Result<Option<DiaperDocument>> {
        self.document(
            &paths::tracker(paths::DIAPER, cid),
            "reading the nappy tracker",
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
    ) -> Result<()> {
        let is_potty = how_it_happened.is_some();
        let operation = if is_potty {
            "recording the potty trip"
        } else {
            "recording the nappy"
        };
        let now = now_seconds();
        let offset = self.zone().offset_minutes(now);

        let entry = DiaperEntry {
            mode: mode.clone(),
            start: Number::Float(now),
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
                &paths::history_row(paths::DIAPER, cid, &ids::interval_id(now)),
                &to_fields(&entry)?,
                operation,
            )
            .await?;

        let summary = if is_potty {
            to_json(&LastPotty {
                mode: Some(mode),
                start: Some(Number::Float(now)),
                offset: Some(Number::Float(offset)),
            })?
        } else {
            to_json(&LastDiaper {
                start: Some(Number::Float(now)),
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
