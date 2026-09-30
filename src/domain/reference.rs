//! Age-aware typical ranges.
//!
//! The ranges themselves live in [`data/reference.toml`](../../data/reference.toml),
//! read in at compile time. They are guidance a pediatrician gives a parent,
//! not clinical thresholds and not targets, and each one says in the file
//! where it came from so a future reader can check it rather than trust it.
//!
//! Two rules govern the file, and both came from the parent this was built
//! for.
//!
//! 1. **There is no band for milk volume, at any age.** The obvious candidate,
//!    150 to 200 ml per kilogram per day, describes established feeding from
//!    roughly two weeks on, not the first week, when intake is still ramping
//!    steeply from a few millilitres a feed. Drawing it for a seven-day-old
//!    would quietly tell a frightened first-time parent they are underfeeding
//!    their baby, every day, in a chart. It is absent by construction rather
//!    than by a special case somebody could delete, and a test asserts no
//!    metric in the file mentions volume.
//!
//! 2. **Labels are declarative, never imperative.** No "should", no "call your
//!    doctor". The parent decides what to do; this only says what is typical.

use std::sync::OnceLock;

use serde::Deserialize;

/// The table as it is written down.
const SOURCE: &str = include_str!("../../data/reference.toml");

/// The one line beside the ranges that is not itself a range.
///
/// Everything in this table describes populations. A pediatrician has the one
/// thing the table structurally cannot have, which is the actual child, so the
/// tool says so where the ranges are printed rather than only in the manual: a
/// caveat nobody sees is a caveat that does not exist.
///
/// This is the single exception to the rule that labels never tell anybody
/// what to do. It is a statement about the standing of the whole table rather
/// than advice triggered by any particular reading, every screen that prints a
/// range prints this one string, and there is exactly one of it.
pub const PEDIATRICIAN_NOTE: &str =
    "typical ranges are not your baby: where your pediatrician disagrees, they are right";

/// Every metric that has bands, as read from the file.
#[derive(Debug, Deserialize)]
pub struct Table {
    /// One entry per metric.
    pub metrics: Vec<MetricBands>,
}

/// One metric, and the bands that apply to it as a child gets older.
#[derive(Debug, Deserialize)]
pub struct MetricBands {
    /// The name the file uses, which [`Metric::as_str`] matches.
    pub metric: String,
    /// What the numbers are counted in, for a reader of the file.
    pub unit: String,
    /// The bands, oldest age last.
    pub bands: Vec<Entry>,
}

/// One row of the table: a range that applies up to an age.
#[derive(Debug, Deserialize)]
pub struct Entry {
    /// The last age in days this band applies to.
    pub max_age_days: i64,
    /// The bottom of the range.
    pub low: f64,
    /// The top, or `None` when the file leaves it out, meaning "or more".
    #[serde(default)]
    pub high: Option<f64>,
    /// The sentence to print beside the number.
    pub label: String,
    /// Where the range came from.
    pub source: String,
}

/// The whole table, for anything checking the file itself.
#[must_use]
pub fn table_ref() -> &'static Table {
    table()
}

/// The table, parsed once.
///
/// A file this build cannot read is a bug in the file, and the test below is
/// what catches it; nothing at runtime can reach a half-read table.
fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| toml::from_str(SOURCE).expect("data/reference.toml is part of the build"))
}

/// A metric a typical range exists for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    /// Milk feeds in a day.
    FeedsPerDay,
    /// Wet diapers in a day.
    WetPerDay,
    /// Dirty diapers in a day.
    DirtyPerDay,
    /// Total sleep in twenty-four hours, in seconds.
    SleepPerDay,
}

impl Metric {
    /// The name the file calls this metric.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FeedsPerDay => "feeds_per_day",
            Self::WetPerDay => "wet_per_day",
            Self::DirtyPerDay => "dirty_per_day",
            Self::SleepPerDay => "sleep_per_day",
        }
    }
}

/// A typical range, and how to say it.
#[derive(Debug, Clone, PartialEq)]
pub struct Band {
    /// The bottom of the range.
    pub low: f64,
    /// The top, or `None` for "or more".
    pub high: Option<f64>,
    /// The sentence to print beside the number.
    pub label: &'static str,
}

/// The typical range for a metric at an age, if there is one.
///
/// Past the last band's age there is no range, because the pattern stops being
/// typical enough to name.
#[must_use]
pub fn band_for(metric: Metric, age_days: Option<i64>) -> Option<Band> {
    let age = age_days?;
    table()
        .metrics
        .iter()
        .find(|table| table.metric == metric.as_str())?
        .bands
        .iter()
        .find(|band| age <= band.max_age_days)
        .map(|band| Band {
            low: band.low,
            high: band.high,
            label: band.label.as_str(),
        })
}

/// Where a value sits relative to a band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// Below the typical range.
    Below,
    /// Inside it.
    Inside,
    /// Above it.
    Above,
}

impl Band {
    /// Where a value sits. This is the whole of the judgement this module
    /// makes: a value is inside a range or it is not, and what to do about
    /// that is the reader's call.
    #[must_use]
    pub fn standing(&self, value: f64) -> Standing {
        if value < self.low {
            return Standing::Below;
        }
        match self.high {
            Some(high) if value > high => Standing::Above,
            _ => Standing::Inside,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newborn_gets_the_newborn_band() {
        let band = band_for(Metric::FeedsPerDay, Some(7)).expect("a band");
        assert!((band.low - 8.0).abs() < f64::EPSILON);
        assert_eq!(band.high, Some(12.0));
    }

    #[test]
    fn an_older_baby_gets_the_next_band_down() {
        let band = band_for(Metric::FeedsPerDay, Some(60)).expect("a band");
        assert!((band.low - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn past_the_last_entry_there_is_no_band_at_all() {
        assert_eq!(
            band_for(Metric::FeedsPerDay, Some(3 * 365)),
            None,
            "milk feeds stop being a countable thing once solids are the meal"
        );
    }

    /// The bands that were researched and written down, spot-checked at the
    /// ages either side of each boundary.
    #[test]
    fn sleep_has_a_band_at_every_age_up_to_five_years() {
        for (age, low_hours, high_hours) in [
            (0, 14.0, 17.0),
            (89, 14.0, 17.0),
            (90, 12.0, 16.0),
            (364, 12.0, 16.0),
            (365, 11.0, 14.0),
            (1094, 11.0, 14.0),
            (1095, 10.0, 13.0),
            (2190, 10.0, 13.0),
        ] {
            let band = band_for(Metric::SleepPerDay, Some(age))
                .unwrap_or_else(|| panic!("no sleep band at {age} days"));
            assert!(
                (band.low - low_hours * 3600.0).abs() < f64::EPSILON,
                "at {age} days the bottom is {} hours",
                band.low / 3600.0
            );
            assert_eq!(band.high, Some(high_hours * 3600.0), "at {age} days");
        }
        assert_eq!(
            band_for(Metric::SleepPerDay, Some(2191)),
            None,
            "past five years this table stops"
        );
    }

    #[test]
    fn feeds_thin_out_as_solids_arrive_and_then_stop_being_counted() {
        for (age, low, high) in [
            (0, 8.0, 12.0),
            (28, 8.0, 12.0),
            (29, 6.0, 10.0),
            (120, 6.0, 10.0),
            (121, 5.0, 8.0),
            (180, 5.0, 8.0),
            (181, 4.0, 6.0),
            (365, 4.0, 6.0),
        ] {
            let band = band_for(Metric::FeedsPerDay, Some(age))
                .unwrap_or_else(|| panic!("no feed band at {age} days"));
            assert!((band.low - low).abs() < f64::EPSILON, "at {age} days");
            assert_eq!(band.high, Some(high), "at {age} days");
        }
        assert_eq!(band_for(Metric::FeedsPerDay, Some(366)), None);
    }

    #[test]
    fn wet_diapers_have_a_floor_that_holds_through_the_nappy_years() {
        for (age, low) in [(5, 6.0), (180, 6.0), (181, 4.0), (730, 4.0)] {
            let band = band_for(Metric::WetPerDay, Some(age))
                .unwrap_or_else(|| panic!("no wet band at {age} days"));
            assert!((band.low - low).abs() < f64::EPSILON, "at {age} days");
            assert_eq!(band.high, None, "a floor has no ceiling");
        }
        assert_eq!(
            band_for(Metric::WetPerDay, Some(731)),
            None,
            "past two years a nappy count says more about potty training"
        );
    }

    /// The one rule this table exists to keep.
    #[test]
    fn there_is_no_band_for_milk_volume_at_any_age() {
        let named: Vec<&str> = table_ref()
            .metrics
            .iter()
            .map(|table| table.metric.as_str())
            .collect();
        for forbidden in ["milk", "volume", "ml", "weight", "ounces"] {
            assert!(
                !named.iter().any(|name| name.contains(forbidden)),
                "`{forbidden}` has no band at any age, by construction: {named:?}"
            );
        }
    }

    #[test]
    fn every_band_is_reachable_and_the_ages_only_go_up() {
        for table in &table_ref().metrics {
            assert!(!table.bands.is_empty(), "{} has no bands", table.metric);
            let ages: Vec<i64> = table.bands.iter().map(|band| band.max_age_days).collect();
            let mut sorted = ages.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(ages, sorted, "{} is out of order", table.metric);
        }
    }

    /// A band with no ceiling has to say so, or a missing line in the file
    /// would quietly turn a range into a floor.
    #[test]
    fn a_band_with_no_ceiling_says_or_more_in_its_label() {
        for band in table_ref().metrics.iter().flat_map(|table| &table.bands) {
            assert_eq!(
                band.high.is_none(),
                band.label.contains("or more"),
                "`{}` and its ceiling disagree",
                band.label
            );
        }
    }

    #[test]
    fn every_band_names_where_it_came_from() {
        for table in &table_ref().metrics {
            for band in &table.bands {
                assert!(
                    !band.source.trim().is_empty(),
                    "`{}` does not say where it came from",
                    band.label
                );
            }
        }
    }

    #[test]
    fn with_no_age_there_is_no_band() {
        assert_eq!(band_for(Metric::WetPerDay, None), None);
    }

    #[test]
    fn the_first_days_have_a_band_each() {
        assert_eq!(
            band_for(Metric::WetPerDay, Some(0)).expect("a band").high,
            Some(2.0)
        );
        assert_eq!(
            band_for(Metric::WetPerDay, Some(1)).expect("a band").high,
            Some(3.0)
        );
        assert_eq!(
            band_for(Metric::WetPerDay, Some(2)).expect("a band").high,
            Some(4.0)
        );
    }

    #[test]
    fn an_open_ended_band_has_no_top() {
        let band = band_for(Metric::WetPerDay, Some(10)).expect("a band");
        assert_eq!(band.high, None);
        assert_eq!(
            band.standing(20.0),
            Standing::Inside,
            "no top means no ceiling"
        );
        assert_eq!(band.standing(3.0), Standing::Below);
    }

    #[test]
    fn a_value_inside_the_band_is_inside_it() {
        let band = band_for(Metric::FeedsPerDay, Some(7)).expect("a band");
        assert_eq!(band.standing(8.0), Standing::Inside, "the bottom is inside");
        assert_eq!(band.standing(12.0), Standing::Inside, "so is the top");
        assert_eq!(band.standing(7.9), Standing::Below);
        assert_eq!(band.standing(12.1), Standing::Above);
    }

    #[test]
    fn dirty_diapers_stop_having_a_band_once_the_pattern_stops_being_typical() {
        assert!(band_for(Metric::DirtyPerDay, Some(30)).is_some());
        assert_eq!(band_for(Metric::DirtyPerDay, Some(60)), None);
    }

    #[test]
    fn sleep_bands_are_in_seconds_so_they_compare_to_a_total_directly() {
        let band = band_for(Metric::SleepPerDay, Some(30)).expect("a band");
        assert!((band.low - 50_400.0).abs() < f64::EPSILON);
    }

    #[test]
    fn every_label_states_what_is_typical_and_tells_nobody_what_to_do() {
        for entry in table_ref().metrics.iter().flat_map(|table| &table.bands) {
            assert!(
                entry.label.starts_with("typical"),
                "`{}` does not read as an observation",
                entry.label
            );
            for imperative in ["should", "must", "need", "doctor", "call "] {
                assert!(
                    !entry.label.contains(imperative),
                    "`{}` contains `{imperative}`",
                    entry.label
                );
            }
        }
    }
}
