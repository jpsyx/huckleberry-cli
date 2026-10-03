//! The dial, configured to answer an amount and the units it is in.
//!
//! Two columns rather than two questions. The units were never a separate
//! thing to decide: "how much" is one answer with a number and a unit in it,
//! and asking twice made somebody confirm a choice they had already made.

use anyhow::Result;

use super::model::Dial;
use super::wheel::Wheel;
use crate::cli::Units;
use crate::render::format;
use crate::theme::Theme;

/// The columns, left to right.
const AMOUNT: usize = 0;
const UNITS: usize = 1;

/// The largest bottle the dial offers, in millilitres.
const MOST_ML: i32 = 400;

/// How much one notch of the ounce wheel is.
const OZ_NOTCH: f64 = 0.25;

/// The largest bottle the dial offers, in notches of an ounce.
const MOST_OZ_NOTCHES: i32 = 64;

/// What the dial starts on when nothing has been recorded yet.
const UNKNOWN_ML: f64 = 60.0;

/// Asks how much, and in what.
///
/// # Errors
///
/// Cancelled when somebody backs out, and a failure when there is no screen.
pub fn ask(label: &str, units: Units, amount: Option<f64>, theme: Theme) -> Result<(Units, f64)> {
    let start = amount.unwrap_or_else(|| format::convert(UNKNOWN_ML, Units::Ml, units));
    let dial = Dial::new(vec![amounts(units, start), units_wheel(units)]);
    let turned = super::terminal::ask(label, dial, theme, settle)?;
    Ok(read(&turned))
}

/// Keeps the amount meaning the same thing when the units change.
///
/// Turning the units column is a change of wording, not of quantity: sixty
/// millilitres becomes two ounces, not two millilitres. The new wheel stands
/// on whichever of its notches is nearest, because an exact conversion is
/// almost never one of them.
fn settle(dial: &mut Dial, column: usize) {
    if column != UNITS {
        return;
    }
    let now = read_units(dial);
    let held: f64 = dial.value(AMOUNT).parse().unwrap_or_default();
    let wanted = format::convert(held, other(now), now);
    if let Some(wheel) = dial.wheels.get_mut(AMOUNT) {
        *wheel = amounts(now, wanted);
    }
}

/// The amount wheel for these units, standing nearest `wanted`.
///
/// Millilitres are whole numbers and turn one at a time, leaping five.
/// Ounces are quarters, and leap four of them, which is a whole ounce.
fn amounts(units: Units, wanted: f64) -> Wheel {
    let (labels, leap): (Vec<String>, usize) = match units {
        Units::Ml => ((1..=MOST_ML).map(|ml| ml.to_string()).collect(), 5),
        Units::Oz => (
            (1..=MOST_OZ_NOTCHES)
                .map(|notch| format!("{:.2}", f64::from(notch) * OZ_NOTCH))
                .collect(),
            4,
        ),
    };
    let mut wheel = Wheel::new(labels, 0, leap, "");
    wheel.stand_nearest(|label| (label.parse::<f64>().unwrap_or_default() - wanted).abs());
    wheel
}

/// The units column, which is a toggle between the only two there are.
fn units_wheel(units: Units) -> Wheel {
    Wheel::new(
        vec![Units::Ml.as_str().to_owned(), Units::Oz.as_str().to_owned()],
        usize::from(matches!(units, Units::Oz)),
        1,
        "   ",
    )
}

/// The other of the two.
const fn other(units: Units) -> Units {
    match units {
        Units::Ml => Units::Oz,
        Units::Oz => Units::Ml,
    }
}

/// Which units the dial is standing on.
fn read_units(dial: &Dial) -> Units {
    if dial.value(UNITS) == Units::Oz.as_str() {
        Units::Oz
    } else {
        Units::Ml
    }
}

/// The amount and the units it is standing on.
fn read(dial: &Dial) -> (Units, f64) {
    (
        read_units(dial),
        dial.value(AMOUNT).parse().unwrap_or_default(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn shifted(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::SHIFT)
    }

    fn dial_at(units: Units, amount: f64) -> Dial {
        Dial::new(vec![amounts(units, amount), units_wheel(units)])
    }

    #[test]
    fn millilitres_are_whole_numbers_and_ounces_are_quarters() {
        assert_eq!(dial_at(Units::Ml, 60.0).value(AMOUNT), "60");
        assert_eq!(dial_at(Units::Oz, 2.0).value(AMOUNT), "2.00");
        assert_eq!(dial_at(Units::Oz, 1.25).value(AMOUNT), "1.25");
    }

    #[test]
    fn millilitres_turn_one_at_a_time_and_leap_five() {
        let mut dial = dial_at(Units::Ml, 60.0);
        dial.apply(press(KeyCode::Down));
        assert_eq!(dial.value(AMOUNT), "61");
        dial.apply(shifted(KeyCode::Down));
        assert_eq!(dial.value(AMOUNT), "66");
        dial.apply(shifted(KeyCode::Up));
        assert_eq!(dial.value(AMOUNT), "61");
    }

    /// A quarter plainly, a whole ounce with shift, which is four of them.
    #[test]
    fn ounces_turn_a_quarter_at_a_time_and_leap_a_whole_one() {
        let mut dial = dial_at(Units::Oz, 2.0);
        dial.apply(press(KeyCode::Down));
        assert_eq!(dial.value(AMOUNT), "2.25");
        dial.apply(shifted(KeyCode::Down));
        assert_eq!(dial.value(AMOUNT), "3.25");
        dial.apply(shifted(KeyCode::Up));
        assert_eq!(dial.value(AMOUNT), "2.25");
    }

    /// Changing the wording is not changing the quantity.
    #[test]
    fn turning_the_units_keeps_the_amount_meaning_the_same_thing() {
        let mut dial = dial_at(Units::Ml, 60.0);
        dial.column = UNITS;
        let column = match dial.apply(press(KeyCode::Down)) {
            super::super::model::DialAction::Turned(column) => column,
            other => panic!("a turn, not {other:?}"),
        };
        settle(&mut dial, column);
        let (units, amount) = read(&dial);
        assert_eq!(units.as_str(), "oz");
        assert!(
            (amount - 2.0).abs() < 0.13,
            "sixty millilitres is about two ounces, not {amount}"
        );
    }

    #[test]
    fn turning_back_again_lands_near_where_it_started() {
        let mut dial = dial_at(Units::Ml, 120.0);
        dial.column = UNITS;
        for _ in 0..2 {
            if let super::super::model::DialAction::Turned(column) =
                dial.apply(press(KeyCode::Down))
            {
                settle(&mut dial, column);
            }
        }
        let (units, amount) = read(&dial);
        assert_eq!(units.as_str(), "ml");
        assert!((amount - 120.0).abs() <= 2.0, "{amount}");
    }

    #[test]
    fn the_units_column_is_a_toggle_between_the_only_two_there_are() {
        let mut dial = dial_at(Units::Ml, 60.0);
        dial.column = UNITS;
        dial.apply(press(KeyCode::Up));
        assert_eq!(read_units(dial_ref(&dial)).as_str(), "oz");
    }

    fn dial_ref(dial: &Dial) -> &Dial {
        dial
    }

    #[test]
    fn an_amount_is_never_zero_because_the_wheel_does_not_offer_one() {
        assert_eq!(amounts(Units::Ml, 0.0).at(0), "1");
        assert_eq!(amounts(Units::Oz, 0.0).at(0), "0.25");
    }
}
