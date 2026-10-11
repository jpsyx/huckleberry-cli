//! Pure pump defaults and duration validation shared by retained forms.
use crate::{cli::Units, render::format};
use anyhow::{Result, bail};
use huckleberry_api::models::{
    Number,
    pump::{LastPump, PumpAmounts},
};

/// Converts entered minutes to the seconds stored by the API.
pub(super) fn duration_seconds(minutes: f64) -> Result<f64> {
    let seconds = minutes * 60.0;
    if !seconds.is_finite() || minutes < 0.0 {
        bail!("pump duration must be a finite number of minutes at least zero");
    }
    Ok(seconds)
}

/// Parses custom duration minutes through the shared finite, nonnegative validator.
pub(super) fn parse_duration(text: &str) -> Result<f64> {
    duration_seconds(
        text.trim()
            .parse()
            .map_err(|_| anyhow::anyhow!("`{text}` is not a number of minutes"))?,
    )
}

/// Resolved volumes with one common storage unit.
pub(super) struct Volumes {
    /// The selected entry mode and quantities.
    pub amounts: PumpAmounts,
    /// Units shared by every recorded quantity.
    pub units: Units,
}

/// The quantity being asked for, including its scriptable flag.
#[derive(Clone, Copy)]
pub(super) enum Part {
    Total,
    Left,
    Right,
}

/// Offers the matching last amount in the units the new dial starts on.
pub(super) fn offered(last: Option<&LastPump>, part: Part, units: Units) -> Option<f64> {
    let last = last?;
    let left = last.left_amount.map(Number::as_f64);
    let right = last.right_amount.map(Number::as_f64);
    let amount = match part {
        Part::Total if left.is_some() || right.is_some() => {
            Some(left.unwrap_or(0.0) + right.unwrap_or(0.0))
        }
        Part::Total => None,
        Part::Left => left,
        Part::Right => right,
    }?;
    let previous_units = last
        .units
        .as_ref()
        .and_then(|unit| Units::from_stored(unit.as_str()))
        .unwrap_or(units);
    let converted = format::convert(amount, previous_units, units);
    (converted.is_finite() && converted >= 0.0).then_some(converted)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fractional_minutes_become_seconds_and_zero_is_valid() {
        assert!((duration_seconds(1.25).unwrap() - 75.0).abs() < f64::EPSILON);
        assert!(duration_seconds(0.0).unwrap().abs() < f64::EPSILON);
    }
    #[test]
    fn invalid_minutes_never_reach_the_api() {
        for minutes in [-1.0, f64::INFINITY, f64::NAN, f64::MAX] {
            assert!(duration_seconds(minutes).is_err());
        }
    }
    #[test]
    fn last_total_combines_both_stored_halves_without_rounding() {
        let last = LastPump {
            left_amount: Some(Number::Float(1.0)),
            right_amount: Some(Number::Float(1.0)),
            units: Some(huckleberry_api::models::feed::VolumeUnits::Ounces),
            ..LastPump::default()
        };
        assert!(
            (offered(Some(&last), Part::Total, Units::Ml).unwrap() - 59.147_059_125).abs() < 1e-9
        );
    }
}
