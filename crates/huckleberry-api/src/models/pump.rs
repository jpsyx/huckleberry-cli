//! `pump/{cid}`: expressed milk.
//!
//! The document, timer, amounts and history shapes are ported from Woyken's
//! `py-huckleberry-api`, including equal side amounts for total-mode entries.

mod document;
mod timer;

pub use document::{LastPump, PumpDocument, PumpPrefs};
pub use timer::PumpTimer;

use serde::{Deserialize, Serialize};

use super::common::Number;
use super::feed::VolumeUnits;

/// The two ways to record expressed milk, excluding ambiguous mixed inputs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PumpAmounts {
    /// A combined volume, stored evenly between the two side fields.
    Total(f64),
    /// A separately measured volume for each side, including zero.
    LeftRight {
        /// The volume from the left side.
        left: f64,
        /// The volume from the right side.
        right: f64,
    },
}

string_enum! {
    /// Whether the two sides were recorded apart or together.
    PumpEntryMode {
        /// Left and right separately.
        LeftRight => "leftright",
        /// One combined figure.
        Total => "total",
    }
}

/// One row of `pump/{cid}/intervals`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PumpInterval {
    /// When the session was, in seconds.
    pub start: Number,
    /// Whether the sides were recorded apart or together.
    #[serde(rename = "entryMode")]
    pub entry_mode: PumpEntryMode,
    /// The left side, or half the combined figure when the mode is `total`.
    #[serde(
        rename = "leftAmount",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub left_amount: Option<Number>,
    /// The right side.
    #[serde(
        rename = "rightAmount",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub right_amount: Option<Number>,
    /// In what units.
    pub units: VolumeUnits,
    /// The timezone offset, in minutes.
    pub offset: Number,
    /// How long it took, in seconds.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub duration: Option<Number>,
    /// The timezone offset at the end, in minutes.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub end_offset: Option<Number>,
    /// When the row last changed.
    #[serde(
        rename = "lastUpdated",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub last_updated: Option<Number>,
    /// Whatever the parent typed.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub notes: Option<String>,
}

impl PumpInterval {
    /// How much was expressed, in millilitres.
    ///
    /// `None` when neither side was recorded, which is different from zero:
    /// a session logged for its duration alone is not a session that produced
    /// nothing.
    #[must_use]
    pub fn total_millilitres(&self) -> Option<f64> {
        let left = self.left_amount.map(Number::as_f64);
        let right = self.right_amount.map(Number::as_f64);
        if left.is_none() && right.is_none() {
            return None;
        }
        Some(
            self.units
                .to_millilitres(left.unwrap_or(0.0) + right.unwrap_or(0.0)),
        )
    }
}

/// A batch of pump rows.
pub type PumpMultiContainer = super::common::MultiContainer<PumpInterval>;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(left: Option<f64>, right: Option<f64>, units: &str) -> PumpInterval {
        serde_json::from_value(json!({
            "start": 1.0,
            "entryMode": "leftright",
            "leftAmount": left,
            "rightAmount": right,
            "units": units,
            "offset": 0.0,
        }))
        .expect("a pump row")
    }

    #[test]
    fn both_sides_add_up() {
        assert_eq!(
            row(Some(60.0), Some(40.0), "ml").total_millilitres(),
            Some(100.0)
        );
    }

    #[test]
    fn one_side_alone_is_still_a_total() {
        assert_eq!(row(Some(60.0), None, "ml").total_millilitres(), Some(60.0));
    }

    #[test]
    fn a_session_with_no_amounts_has_no_total_rather_than_a_zero() {
        assert_eq!(row(None, None, "ml").total_millilitres(), None);
    }

    #[test]
    fn ounces_are_converted() {
        let total = row(Some(2.0), Some(2.0), "oz")
            .total_millilitres()
            .expect("a total");
        assert!((total - 118.294_118_25).abs() < 0.001);
    }
}
