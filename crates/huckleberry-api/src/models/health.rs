//! `health/{cid}`: growth, medication and temperature.
//!
//! One thing sets this tracker apart: its history lives in a subcollection
//! called `data`, not `intervals` like every other tracker.

use serde::{Deserialize, Serialize};

use super::common::{Number, ReminderV2, Timestamp};

string_enum! {
    /// How a weight is measured.
    WeightUnits {
        Kilograms => "kg",
        PoundsOunces => "lbs.oz",
    }
}

string_enum! {
    /// How a length is measured.
    HeightUnits {
        Centimetres => "cm",
        FeetInches => "ft.in",
    }
}

string_enum! {
    /// How a head circumference is measured. The `h` prefix is the app's.
    HeadUnits {
        Centimetres => "hcm",
        Inches => "hin",
    }
}

string_enum! {
    /// How a dose is measured.
    MedicationUnits {
        Millilitres => "ml",
        Cups => "cup",
        Teaspoons => "tsp",
        Drops => "drops",
    }
}

string_enum! {
    /// Which temperature scale.
    TemperatureUnits {
        Celsius => "C",
        Fahrenheit => "F",
    }
}

/// Which system of units a measurement was taken in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementSystem {
    /// Kilograms and centimetres.
    Metric,
    /// Pounds and inches.
    Imperial,
}

impl MeasurementSystem {
    /// The three unit names this system writes.
    #[must_use]
    pub const fn units(self) -> (WeightUnits, HeightUnits, HeadUnits) {
        match self {
            Self::Metric => (
                WeightUnits::Kilograms,
                HeightUnits::Centimetres,
                HeadUnits::Centimetres,
            ),
            Self::Imperial => (
                WeightUnits::PoundsOunces,
                HeightUnits::FeetInches,
                HeadUnits::Inches,
            ),
        }
    }
}

/// One growth measurement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrowthEntry {
    /// The row's own identifier.
    #[serde(rename = "_id", default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Always `health`.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// When it was taken, in seconds.
    pub start: Number,
    /// When the row last changed.
    #[serde(rename = "lastUpdated")]
    pub last_updated: Number,
    /// The timezone offset, in minutes.
    pub offset: Number,
    /// Whether the app filed it as an overnight entry.
    #[serde(rename = "isNight", default, skip_serializing_if = "Option::is_none")]
    pub is_night: Option<bool>,
    /// The batch this row came out of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multientry_key: Option<String>,
    /// Weight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<Number>,
    /// In what units.
    #[serde(
        rename = "weightUnits",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub weight_units: Option<WeightUnits>,
    /// Length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<Number>,
    /// In what units.
    #[serde(
        rename = "heightUnits",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub height_units: Option<HeightUnits>,
    /// Head circumference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<Number>,
    /// In what units.
    #[serde(rename = "headUnits", default, skip_serializing_if = "Option::is_none")]
    pub head_units: Option<HeadUnits>,
}

/// How many kilograms are in a pound.
pub const KILOGRAMS_PER_POUND: f64 = 0.453_592_37;

impl GrowthEntry {
    /// The weight in kilograms, whichever unit it was taken in.
    #[must_use]
    pub fn weight_kilograms(&self) -> Option<f64> {
        let weight = self.weight?.as_f64();
        match self.weight_units.as_ref() {
            Some(WeightUnits::PoundsOunces) => Some(weight * KILOGRAMS_PER_POUND),
            Some(WeightUnits::Kilograms) => Some(weight),
            // The app has written growth rows with no unit on them. Treating
            // an unlabelled weight as kilograms would turn a 7 lb newborn into
            // a 7 kg one, so it is left unconverted and the caller decides.
            _ => None,
        }
    }
}

/// One dose of medication.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MedicationEntry {
    /// Always `health`.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// When it was given, in seconds.
    pub start: Number,
    /// When the row last changed.
    #[serde(rename = "lastUpdated")]
    pub last_updated: Number,
    /// The timezone offset, in minutes.
    pub offset: Number,
    /// Which medication, by id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub medication_id: Option<String>,
    /// Which medication, by name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub medication_name: Option<String>,
    /// How much.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<Number>,
    /// In what units.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<MedicationUnits>,
    /// Whatever the parent typed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// The batch this row came out of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multientry_key: Option<String>,
}

/// One temperature reading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemperatureEntry {
    /// Always `health`.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// When it was taken, in seconds.
    pub start: Number,
    /// When the row last changed.
    #[serde(rename = "lastUpdated")]
    pub last_updated: Number,
    /// The timezone offset, in minutes.
    pub offset: Number,
    /// The reading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<Number>,
    /// Which scale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub units: Option<TemperatureUnits>,
    /// The batch this row came out of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multientry_key: Option<String>,
}

/// One row of `health/{cid}/data`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum HealthEntry {
    /// A growth measurement.
    Growth(GrowthEntry),
    /// A dose of medication.
    Medication(MedicationEntry),
    /// A temperature reading.
    Temperature(TemperatureEntry),
}

impl HealthEntry {
    /// When it was, in seconds.
    #[must_use]
    pub const fn start(&self) -> f64 {
        match self {
            Self::Growth(entry) => entry.start.as_f64(),
            Self::Medication(entry) => entry.start.as_f64(),
            Self::Temperature(entry) => entry.start.as_f64(),
        }
    }
}

/// `health/{cid}.prefs`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HealthPrefs {
    /// The last growth measurement, which is how `latest growth` is read
    /// without touching the history at all.
    #[serde(
        rename = "lastGrowthEntry",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_growth_entry: Option<GrowthEntry>,
    /// The last dose.
    #[serde(
        rename = "lastMedication",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_medication: Option<MedicationEntry>,
    /// The last temperature.
    #[serde(
        rename = "lastTemperature",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_temperature: Option<TemperatureEntry>,
    /// The medication reminder.
    #[serde(
        rename = "reminderV2",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub reminder: Option<ReminderV2>,
    /// When the preferences last changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<Timestamp>,
    /// The same moment, as a bare number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_timestamp: Option<Number>,
}

/// `health/{cid}`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HealthDocument {
    /// The last entry of each kind, and the tracker's settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefs: Option<HealthPrefs>,
}

/// A batch of health rows.
pub type HealthMultiContainer = super::common::MultiContainer<HealthEntry>;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_growth_row_decodes_by_its_mode() {
        let entry: HealthEntry = serde_json::from_value(json!({
            "mode": "growth", "start": 1.0, "lastUpdated": 1.0, "offset": 0.0,
            "weight": 7.5, "weightUnits": "lbs.oz",
        }))
        .expect("a growth row");
        let HealthEntry::Growth(growth) = &entry else {
            panic!("expected growth, got {entry:?}");
        };
        assert!((growth.weight_kilograms().expect("a weight") - 3.401_942_775).abs() < 0.001);
    }

    #[test]
    fn a_weight_already_in_kilograms_is_not_converted_again() {
        let growth = GrowthEntry {
            id: None,
            kind: None,
            start: Number::Float(1.0),
            last_updated: Number::Float(1.0),
            offset: Number::Float(0.0),
            is_night: None,
            multientry_key: None,
            weight: Some(Number::Float(3.4)),
            weight_units: Some(WeightUnits::Kilograms),
            height: None,
            height_units: None,
            head: None,
            head_units: None,
        };
        assert!((growth.weight_kilograms().expect("a weight") - 3.4).abs() < f64::EPSILON);
    }

    #[test]
    fn an_unlabelled_weight_is_left_alone_rather_than_assumed() {
        let entry: GrowthEntry = serde_json::from_value(json!({
            "start": 1.0, "lastUpdated": 1.0, "offset": 0.0, "weight": 7.5,
        }))
        .expect("a growth row");
        assert_eq!(entry.weight_kilograms(), None);
    }

    #[test]
    fn each_system_names_its_three_units() {
        assert_eq!(
            MeasurementSystem::Metric.units(),
            (
                WeightUnits::Kilograms,
                HeightUnits::Centimetres,
                HeadUnits::Centimetres
            )
        );
        assert_eq!(MeasurementSystem::Imperial.units().2, HeadUnits::Inches);
    }

    #[test]
    fn a_temperature_row_decodes_by_its_mode() {
        let entry: HealthEntry = serde_json::from_value(json!({
            "mode": "temperature", "start": 5.0, "lastUpdated": 5.0, "offset": 0.0,
            "amount": 37.2, "units": "C",
        }))
        .expect("a temperature row");
        assert!((entry.start() - 5.0).abs() < f64::EPSILON);
        assert!(matches!(entry, HealthEntry::Temperature(_)));
    }
}
