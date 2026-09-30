//! Age-aware typical ranges.
//!
//! The ranges themselves live in [`data/reference.toml`](../../data/reference.toml),
//! read in at compile time. They are guidance a pediatrician gives a parent,
//! not clinical thresholds and not targets, and each one says in the file
//! where it came from so a future reader can check it rather than trust it.
//!
//! Every band was adversarially reviewed against primary sources on
//! 2026-09-30. The record is in `docs/research/`, and the method for doing it
//! again, for this or for anything else this tool ever asserts about a child,
//! is in `docs/research/methodology.md`. Read it before adding a band.
//!
//! Four rules govern the file, and the file's own header gives the evidence
//! for each.
//!
//! 1. **There is no band for milk volume, at any age.** The figure people
//!    reach for, 150 to 200 ml per kilogram per day, does get published, so
//!    the reason it stays out is not that nobody says it. The reason is what
//!    it would do: measured intake in healthy exclusively breastfed term
//!    infants runs 135 ml/kg/day at one month, 126 at three and 107 at six, so
//!    a floor of 150 would tell the parent of a thriving breastfed baby they
//!    were underfeeding, every day, for most of the first year. The sources
//!    also disagree at the age where the figure peaks, and the number depends
//!    on whether a bottle holds milk or formula, which this tool cannot know.
//!    It is absent by construction rather than by a special case somebody
//!    could delete, and a test asserts no metric in the file mentions volume.
//!
//! 2. **Labels say what kind of claim they make.** "typical" describes what
//!    was observed; "recommended" reports what a body advises. The sleep bands
//!    from four months on are recommendations, and observed normal is wider.
//!
//! 3. **Labels are declarative, never imperative.** No "should", no "call your
//!    doctor". The parent decides what to do; this only says what is typical.
//!    [`PEDIATRICIAN_NOTE`] is the single exception, and it is about the
//!    standing of the whole table rather than about any one reading.
//!
//! 4. **A floor sits under every qualifying source, never above one.** Where
//!    sources disagree, the floor goes beneath the lowest of them.
//!
//! Counts have no ceilings, because no qualifying source calls a high number
//! of feeds or diapers atypical. Sleep is the one metric with a top.
//!
//! Ages here are **days since birth**, so age 0 is the first day of life,
//! while clinical guidance counts from 1. The labels are written in days of
//! life, so a label reading "day 5" belongs to the band whose ages reach 4.

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
        assert_eq!(
            band.high, None,
            "no source calls a high feed count atypical"
        );
    }

    #[test]
    fn an_older_baby_gets_the_next_band_down() {
        let band = band_for(Metric::FeedsPerDay, Some(60)).expect("a band");
        assert!((band.low - 4.0).abs() < f64::EPSILON);
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
    fn sleep_has_a_band_at_every_age_through_the_fifth_year() {
        for (age, low_hours, high_hours) in [
            // Observed normal, which is far wider than the recommendation.
            (0, 8.0, 20.0),
            (121, 8.0, 20.0),
            // The AASM bands, which start at four months and not before.
            (122, 12.0, 16.0),
            (364, 12.0, 16.0),
            (365, 11.0, 14.0),
            (1094, 11.0, 14.0),
            (1095, 10.0, 13.0),
            (2189, 10.0, 13.0),
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
            band_for(Metric::SleepPerDay, Some(2190)),
            None,
            "the sixth birthday is the AASM's next band, not this one"
        );
    }

    /// Two bands, both floors, and then nothing.
    ///
    /// The newborn floor is the one every body agrees on. After a month the
    /// floor drops to what was actually observed, because the observed mean is
    /// 7.9 sessions a day and a floor of 8 would call half of a normal
    /// population short.
    #[test]
    fn feeds_have_a_newborn_floor_then_an_observed_one_and_then_stop() {
        for (age, low) in [(0, 8.0), (28, 8.0), (29, 4.0), (180, 4.0)] {
            let band = band_for(Metric::FeedsPerDay, Some(age))
                .unwrap_or_else(|| panic!("no feed band at {age} days"));
            assert!((band.low - low).abs() < f64::EPSILON, "at {age} days");
            assert_eq!(band.high, None, "at {age} days a count has no ceiling");
        }
        assert_eq!(
            band_for(Metric::FeedsPerDay, Some(181)),
            None,
            "past six months the CDC declines to give a number and the AAP's is formula-only"
        );
    }

    /// Ages here are days since birth, so age 4 is the fifth day of life.
    #[test]
    fn wet_diapers_climb_over_the_first_days_and_then_hold() {
        for (age, low) in [(0, 1.0), (1, 2.0), (2, 5.0), (3, 5.0), (4, 5.0), (180, 5.0)] {
            let band = band_for(Metric::WetPerDay, Some(age))
                .unwrap_or_else(|| panic!("no wet band at {age} days"));
            assert!((band.low - low).abs() < f64::EPSILON, "at {age} days");
            assert_eq!(band.high, None, "a floor has no ceiling");
        }
        assert_eq!(
            band_for(Metric::WetPerDay, Some(181)),
            None,
            "past six months every figure is a dehydration threshold, not a typical count"
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

    /// The count climbing day by day is the thing being watched, so the first
    /// days get their own bands rather than one average across them.
    #[test]
    fn the_first_days_have_a_band_each() {
        let floor = |age| band_for(Metric::WetPerDay, Some(age)).expect("a band").low;
        assert!(floor(0) < floor(1), "day 1 expects less than day 2");
        assert!(
            floor(1) < floor(2),
            "and the third day is where output climbs"
        );
    }

    /// Observed voiding runs to about twenty a day in the first month, so a
    /// high count is never the thing worth flagging.
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
        let band = band_for(Metric::SleepPerDay, Some(7)).expect("a band");
        assert_eq!(
            band.standing(8.0 * 3600.0),
            Standing::Inside,
            "the bottom is inside"
        );
        assert_eq!(
            band.standing(20.0 * 3600.0),
            Standing::Inside,
            "so is the top"
        );
        assert_eq!(band.standing(7.9 * 3600.0), Standing::Below);
        assert_eq!(band.standing(20.1 * 3600.0), Standing::Above);
    }

    /// Ages are days since birth, so age 2 is the third day of life.
    ///
    /// The metric covers the meconium days and stops. Past them a breastfed
    /// baby and a formula-fed one differ about threefold, and no single floor
    /// is honest for both.
    #[test]
    fn dirty_diapers_cover_the_meconium_days_and_then_stop() {
        for (age, low) in [(0, 1.0), (1, 1.0), (2, 2.0), (3, 2.0)] {
            let band = band_for(Metric::DirtyPerDay, Some(age))
                .unwrap_or_else(|| panic!("no dirty band at {age} days"));
            assert!((band.low - low).abs() < f64::EPSILON, "at {age} days");
            assert_eq!(band.high, None, "no source calls a high count atypical");
        }
        assert_eq!(
            band_for(Metric::DirtyPerDay, Some(4)),
            None,
            "from the fifth day the two feeding patterns come apart"
        );
        assert_eq!(band_for(Metric::DirtyPerDay, Some(60)), None);
    }

    /// A floor of one from the fifth day paints a breastfed baby with a single
    /// stool green, which is the reading the CDC names as a warning sign and
    /// the AAP puts three to four times higher. Rather than pick the
    /// population whose low output is least dangerous, there is no band.
    #[test]
    fn no_dirty_band_survives_where_the_feeding_patterns_diverge() {
        assert_eq!(band_for(Metric::DirtyPerDay, Some(6)), None);
    }

    /// The floor a day-4 baby is judged against must not be the number the
    /// literature uses to flag inadequate intake. Ours used to be exactly
    /// that: three soiled diapers on day 4 is the published warning value, and
    /// we called it typical.
    #[test]
    fn no_dirty_floor_sits_at_the_published_warning_value() {
        let day_four = band_for(Metric::DirtyPerDay, Some(3)).expect("a band");
        assert!(
            day_four.low < 3.0,
            "three on day 4 is a warning sign, not a floor to reassure with"
        );
    }

    #[test]
    fn sleep_bands_are_in_seconds_so_they_compare_to_a_total_directly() {
        let band = band_for(Metric::SleepPerDay, Some(30)).expect("a band");
        assert!((band.low - 28_800.0).abs() < f64::EPSILON);
    }

    #[test]
    fn every_label_states_what_is_typical_and_tells_nobody_what_to_do() {
        for entry in table_ref().metrics.iter().flat_map(|table| &table.bands) {
            // Two openings, because the sources make two different kinds of
            // claim. "typical" describes what was observed; "recommended"
            // reports what a body advises, which is not the same thing and is
            // not honestly said in the other's words.
            assert!(
                entry.label.starts_with("typical") || entry.label.starts_with("recommended"),
                "`{}` reads as neither an observation nor a recommendation",
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
