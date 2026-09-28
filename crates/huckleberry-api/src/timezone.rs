//! The timezone offset every Huckleberry row carries.
//!
//! Two things about this are easy to get backwards.
//!
//! **The sign is inverted.** Huckleberry stores what JavaScript's
//! `getTimezoneOffset` returns: minutes to *add* to local time to get UTC. New
//! York in summer is UTC-4 and stores `240`; Berlin in summer is UTC+2 and
//! stores `-120`.
//!
//! **It is computed per instant, not once.** A row written in January and a
//! row written in July carry different offsets for the same timezone, and a
//! sleep can begin on one side of a DST change and end on the other, which is
//! why an interval has both `offset` and `end_offset`.

use jiff::tz::TimeZone;

use crate::error::{Error, Result};

/// A timezone, resolved from an IANA name.
#[derive(Debug, Clone)]
pub struct Zone {
    name: String,
    zone: TimeZone,
}

impl Zone {
    /// Looks up an IANA timezone by name, for example `America/New_York`.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownTimezone`] when the name is not in the IANA database.
    pub fn new(name: &str) -> Result<Self> {
        let zone = TimeZone::get(name).map_err(|_| Error::UnknownTimezone(name.to_owned()))?;
        Ok(Self {
            name: name.to_owned(),
            zone,
        })
    }

    /// UTC, for a caller that has no timezone to offer.
    #[must_use]
    pub fn utc() -> Self {
        Self {
            name: "UTC".to_owned(),
            zone: TimeZone::UTC,
        }
    }

    /// The IANA name this zone was built from.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The offset Huckleberry stores for an instant, in minutes.
    ///
    /// Positive west of Greenwich, negative east of it: the opposite of the
    /// usual UTC offset, and the same convention the app writes.
    #[must_use]
    pub fn offset_minutes(&self, at: f64) -> f64 {
        let timestamp =
            jiff::Timestamp::from_second(at as i64).unwrap_or(jiff::Timestamp::UNIX_EPOCH);
        let seconds = f64::from(self.zone.to_offset(timestamp).seconds());
        -seconds / 60.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2025-07-01T12:00:00Z, which is summer in both hemispheres' examples
    /// below and firmly inside daylight saving where it applies.
    const SUMMER: f64 = 1_751_371_200.0;
    /// 2025-01-01T12:00:00Z.
    const WINTER: f64 = 1_735_732_800.0;

    #[test]
    fn a_zone_west_of_greenwich_stores_a_positive_offset() {
        let zone = Zone::new("America/New_York").expect("a real timezone");
        assert!((zone.offset_minutes(SUMMER) - 240.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_zone_east_of_greenwich_stores_a_negative_offset() {
        let zone = Zone::new("Europe/Berlin").expect("a real timezone");
        assert!((zone.offset_minutes(SUMMER) + 120.0).abs() < f64::EPSILON);
    }

    #[test]
    fn the_offset_follows_daylight_saving_rather_than_being_fixed() {
        let zone = Zone::new("America/New_York").expect("a real timezone");
        assert!((zone.offset_minutes(WINTER) - 300.0).abs() < f64::EPSILON);
        assert!((zone.offset_minutes(SUMMER) - 240.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_half_hour_zone_keeps_its_half_hour() {
        let zone = Zone::new("Asia/Kolkata").expect("a real timezone");
        assert!((zone.offset_minutes(SUMMER) + 330.0).abs() < f64::EPSILON);
    }

    #[test]
    fn utc_is_zero() {
        assert!(Zone::utc().offset_minutes(SUMMER).abs() < f64::EPSILON);
    }

    #[test]
    fn a_name_the_database_does_not_know_is_a_failure_rather_than_a_silent_utc() {
        assert!(matches!(
            Zone::new("Mars/Olympus_Mons"),
            Err(Error::UnknownTimezone(_))
        ));
    }
}
