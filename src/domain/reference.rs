//! Age-aware typical ranges.
//!
//! Two rules govern this file, and both came from the parent it was built for.
//!
//! 1. **There is no band for milk volume, at any age.** The obvious candidate,
//!    150 to 200 ml per kilogram per day, describes established feeding from
//!    roughly two weeks on, not the first week, when intake is still ramping
//!    steeply from a few millilitres a feed. Drawing it for a seven-day-old
//!    would quietly tell a frightened first-time parent they are underfeeding
//!    their baby, every day, in a chart. `MilkPerDay` is simply not a metric
//!    this module has, so it returns nothing by construction rather than by a
//!    special case somebody could delete.
//!
//! 2. **Labels are declarative, never imperative.** No "should", no "call your
//!    doctor". The parent decides what to do; this only says what is typical.

/// A metric a typical range exists for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    /// Milk feeds in a day.
    FeedsPerDay,
    /// Wet nappies in a day.
    WetPerDay,
    /// Dirty nappies in a day.
    DirtyPerDay,
    /// Total sleep in twenty-four hours, in seconds.
    SleepPerDay,
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

/// One row of the table: a range that applies up to an age.
struct Entry {
    max_age_days: i64,
    low: f64,
    high: Option<f64>,
    label: &'static str,
}

const FEEDS_PER_DAY: &[Entry] = &[
    Entry {
        max_age_days: 28,
        low: 8.0,
        high: Some(12.0),
        label: "typical at this age: 8 to 12 feeds a day",
    },
    Entry {
        max_age_days: 120,
        low: 6.0,
        high: Some(10.0),
        label: "typical at this age: 6 to 10 feeds a day",
    },
];

const WET_PER_DAY: &[Entry] = &[
    Entry {
        max_age_days: 0,
        low: 1.0,
        high: Some(2.0),
        label: "typical on day 1: 1 to 2 wet nappies",
    },
    Entry {
        max_age_days: 1,
        low: 2.0,
        high: Some(3.0),
        label: "typical on day 2: 2 to 3 wet nappies",
    },
    Entry {
        max_age_days: 2,
        low: 3.0,
        high: Some(4.0),
        label: "typical on day 3: 3 to 4 wet nappies",
    },
    Entry {
        max_age_days: 4,
        low: 4.0,
        high: Some(6.0),
        label: "typical at this age: 4 to 6 wet nappies a day",
    },
    Entry {
        max_age_days: 120,
        low: 6.0,
        high: None,
        label: "typical from day 5: 6 or more wet nappies a day",
    },
];

const DIRTY_PER_DAY: &[Entry] = &[
    Entry {
        max_age_days: 2,
        low: 1.0,
        high: None,
        label: "typical in the first days: 1 or more dirty nappies",
    },
    // After about six weeks the pattern becomes genuinely variable, and a band
    // past here would imply a target where none exists.
    Entry {
        max_age_days: 42,
        low: 3.0,
        high: None,
        label: "typical at this age: 3 or more dirty nappies a day",
    },
];

const SLEEP_PER_DAY: &[Entry] = &[
    Entry {
        max_age_days: 90,
        low: 14.0 * 3600.0,
        high: Some(17.0 * 3600.0),
        label: "typical at this age: 14 to 17 hours in 24",
    },
    Entry {
        max_age_days: 365,
        low: 12.0 * 3600.0,
        high: Some(16.0 * 3600.0),
        label: "typical at this age: 12 to 16 hours in 24",
    },
];

/// The typical range for a metric at an age, if there is one.
///
/// Past the last entry's age there is no band, because the pattern stops being
/// typical enough to name.
#[must_use]
pub fn band_for(metric: Metric, age_days: Option<i64>) -> Option<Band> {
    let age = age_days?;
    let table = match metric {
        Metric::FeedsPerDay => FEEDS_PER_DAY,
        Metric::WetPerDay => WET_PER_DAY,
        Metric::DirtyPerDay => DIRTY_PER_DAY,
        Metric::SleepPerDay => SLEEP_PER_DAY,
    };
    table
        .iter()
        .find(|entry| age <= entry.max_age_days)
        .map(|entry| Band {
            low: entry.low,
            high: entry.high,
            label: entry.label,
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
        assert_eq!(band_for(Metric::FeedsPerDay, Some(400)), None);
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
    fn dirty_nappies_stop_having_a_band_once_the_pattern_stops_being_typical() {
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
        let tables = [FEEDS_PER_DAY, WET_PER_DAY, DIRTY_PER_DAY, SLEEP_PER_DAY];
        for entry in tables.into_iter().flatten() {
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
