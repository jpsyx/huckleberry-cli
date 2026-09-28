//! Growth measurements.
//!
//! Health is the tracker whose history lives in a subcollection called `data`
//! rather than `intervals`; [`crate::paths`] is where that is decided, and
//! this module simply asks for the health history path.

use serde_json::json;

use crate::client::{Huckleberry, now_seconds};
use crate::error::{Error, Result};
use crate::firestore::FieldUpdate;
use crate::ids;
use crate::models::common::Number;
use crate::models::health::{GrowthEntry, HealthDocument, HealthEntry, MeasurementSystem};
use crate::models::{to_fields, to_json};
use crate::paths;

/// A set of measurements to record. At least one has to be given.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GrowthMeasurements {
    /// Kilograms, or pounds under [`MeasurementSystem::Imperial`].
    pub weight: Option<f64>,
    /// Centimetres, or inches.
    pub height: Option<f64>,
    /// Head circumference, in the same length unit.
    pub head: Option<f64>,
}

impl GrowthMeasurements {
    /// Whether there is anything to record.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.weight.is_none() && self.height.is_none() && self.head.is_none()
    }
}

/// Builds the row a set of measurements writes.
///
/// A measurement that was not taken is left out entirely rather than written
/// as zero, and the unit field is written only beside the measurement it
/// describes: a lone `weightUnits` on a row with no weight is a field the app
/// reads and cannot use.
#[must_use]
pub fn growth_entry(
    id: &str,
    measurements: &GrowthMeasurements,
    system: MeasurementSystem,
    offset_minutes: f64,
    now: f64,
) -> GrowthEntry {
    let (weight_units, height_units, head_units) = system.units();
    GrowthEntry {
        id: Some(id.to_owned()),
        kind: Some("health".to_owned()),
        start: Number::Float(now),
        last_updated: Number::Float(now),
        offset: Number::Float(offset_minutes),
        is_night: Some(false),
        multientry_key: None,
        weight: measurements.weight.map(Number::Float),
        weight_units: measurements.weight.map(|_| weight_units),
        height: measurements.height.map(Number::Float),
        height_units: measurements.height.map(|_| height_units),
        head: measurements.head.map(Number::Float),
        head_units: measurements.head.map(|_| head_units),
    }
}

impl Huckleberry {
    /// Records a growth measurement.
    ///
    /// # Errors
    ///
    /// [`Error::Invalid`] when no measurement was given, and otherwise as
    /// every write: a refused or unreachable Firestore.
    pub async fn log_growth(
        &self,
        cid: &str,
        measurements: &GrowthMeasurements,
        system: MeasurementSystem,
    ) -> Result<()> {
        if measurements.is_empty() {
            return Err(Error::Invalid(
                "a growth entry needs a weight, a height or a head measurement".to_owned(),
            ));
        }
        let now = now_seconds();
        let offset = self.zone().offset_minutes(now);
        let id = ids::interval_id(now);
        // Wrapped in `HealthEntry` so the row carries `mode: "growth"`, which
        // is how the app tells growth from a temperature in the same
        // subcollection.
        let entry = HealthEntry::Growth(growth_entry(&id, measurements, system, offset, now));

        let token = self.token().await?;
        self.firestore()
            .set(
                &token,
                &paths::history_row(paths::HEALTH, cid, &id),
                &to_fields(&entry)?,
                "recording the growth measurement",
            )
            .await?;

        self.firestore()
            .update(
                &token,
                &paths::tracker(paths::HEALTH, cid),
                &[
                    FieldUpdate::set("prefs.lastGrowthEntry", to_json(&entry)?),
                    FieldUpdate::set("prefs.timestamp", json!({ "seconds": now })),
                    FieldUpdate::set("prefs.local_timestamp", json!(now)),
                ],
                "recording the growth measurement",
            )
            .await
    }

    /// The health tracker's document.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::log_growth`], minus the validation.
    pub async fn health_document(&self, cid: &str) -> Result<Option<HealthDocument>> {
        self.document(
            &paths::tracker(paths::HEALTH, cid),
            "reading the health tracker",
        )
        .await
    }
}

#[cfg(test)]
mod entries {
    use super::*;
    use crate::models::health::{HeadUnits, HeightUnits, WeightUnits};

    fn weight_only() -> GrowthMeasurements {
        GrowthMeasurements {
            weight: Some(3.4),
            ..GrowthMeasurements::default()
        }
    }

    #[test]
    fn a_measurement_that_was_not_taken_is_left_out_entirely() {
        let entry = growth_entry("id", &weight_only(), MeasurementSystem::Metric, -240.0, 1.0);
        assert_eq!(entry.height, None);
        assert_eq!(entry.height_units, None, "no unit without a measurement");
        assert_eq!(entry.head_units, None);
    }

    #[test]
    fn metric_writes_the_metric_unit_names() {
        let entry = growth_entry("id", &weight_only(), MeasurementSystem::Metric, 0.0, 1.0);
        assert_eq!(entry.weight_units, Some(WeightUnits::Kilograms));
    }

    #[test]
    fn imperial_writes_the_apps_own_compound_unit_names() {
        let all = GrowthMeasurements {
            weight: Some(7.5),
            height: Some(20.0),
            head: Some(13.5),
        };
        let entry = growth_entry("id", &all, MeasurementSystem::Imperial, 0.0, 1.0);
        assert_eq!(entry.weight_units, Some(WeightUnits::PoundsOunces));
        assert_eq!(entry.height_units, Some(HeightUnits::FeetInches));
        assert_eq!(entry.head_units, Some(HeadUnits::Inches));
    }

    #[test]
    fn an_empty_set_of_measurements_knows_it_is_empty() {
        assert!(GrowthMeasurements::default().is_empty());
        assert!(!weight_only().is_empty());
    }

    #[test]
    fn the_row_carries_its_own_id_the_way_the_app_writes_it() {
        let entry = growth_entry("row-1", &weight_only(), MeasurementSystem::Metric, 0.0, 1.0);
        assert_eq!(entry.id.as_deref(), Some("row-1"));
        assert_eq!(entry.kind.as_deref(), Some("health"));
    }
}
