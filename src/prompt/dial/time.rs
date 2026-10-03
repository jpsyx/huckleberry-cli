//! The dial, configured to answer a time.

use anyhow::Result;

use super::model::Dial;
use super::wheel::Wheel;
use crate::domain::clock::TimeOfDay;
use crate::theme::Theme;

/// How far a shifted turn moves the minutes.
///
/// Five. A plain turn is one minute, because a minute is the correction
/// somebody came to make; five is the reach for when it is further off.
const MINUTE_LEAP: usize = 5;

/// The columns, left to right.
const HOUR: usize = 0;
const MINUTE: usize = 1;
const HALF: usize = 2;

/// Asks for a time.
///
/// # Errors
///
/// Cancelled when somebody backs out, and a failure when there is no screen.
pub fn ask(label: &str, initial: TimeOfDay, theme: Theme) -> Result<TimeOfDay> {
    // Nothing to put right between turns: the three columns are independent,
    // which is the whole reason the half of the day never moves on its own.
    let turned = super::terminal::ask(label, dial_for(initial), theme, |_, _| {})?;
    Ok(read(&turned))
}

/// The three wheels, standing at a time.
fn dial_for(time: TimeOfDay) -> Dial {
    let (hour, afternoon) = match time.hour {
        0 => (12, false),
        12 => (12, true),
        hour if hour > 12 => (hour - 12, true),
        hour => (hour, false),
    };
    Dial::new(vec![
        Wheel::new(
            (1..=12).map(|hour| hour.to_string()).collect(),
            usize::try_from(hour - 1).unwrap_or_default(),
            1,
            "",
        ),
        Wheel::new(
            (0..60).map(|minute| format!("{minute:02}")).collect(),
            usize::try_from(time.minute).unwrap_or_default(),
            MINUTE_LEAP,
            " : ",
        ),
        Wheel::new(
            vec!["am".to_owned(), "pm".to_owned()],
            usize::from(afternoon),
            1,
            "   ",
        ),
    ])
}

/// The time it is standing on.
fn read(dial: &Dial) -> TimeOfDay {
    let hour: i8 = dial.value(HOUR).parse().unwrap_or(12);
    let minute: i8 = dial.value(MINUTE).parse().unwrap_or_default();
    let hour = match (hour, dial.value(HALF)) {
        (12, "am") => 0,
        (12, _) => 12,
        (hour, "pm") => hour + 12,
        (hour, _) => hour,
    };
    TimeOfDay::new(hour, minute).unwrap_or(TimeOfDay { hour: 0, minute: 0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn at(hour: i8, minute: i8) -> TimeOfDay {
        TimeOfDay::new(hour, minute).expect("a time")
    }

    #[test]
    fn a_time_goes_onto_the_dial_and_comes_back_unchanged() {
        for hour in 0..24 {
            for minute in [0, 7, 44, 59] {
                let time = at(hour, minute);
                assert_eq!(read(&dial_for(time)), time, "{hour}:{minute}");
            }
        }
    }

    #[test]
    fn midnight_and_noon_are_both_twelve_on_the_dial() {
        assert_eq!(dial_for(at(0, 0)).value(HOUR), "12");
        assert_eq!(dial_for(at(0, 0)).value(HALF), "am");
        assert_eq!(dial_for(at(12, 0)).value(HOUR), "12");
        assert_eq!(dial_for(at(12, 0)).value(HALF), "pm");
    }

    /// A plain turn is one minute and shift is five, which is the way round
    /// a correction usually wants.
    #[test]
    fn the_minutes_turn_one_at_a_time_and_leap_five_with_shift() {
        let mut dial = dial_for(at(13, 44));
        dial.column = MINUTE;
        dial.apply(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(read(&dial), at(13, 45));
        dial.apply(KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT));
        assert_eq!(read(&dial), at(13, 50));
        dial.apply(KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT));
        assert_eq!(read(&dial), at(13, 45));
    }

    #[test]
    fn the_half_of_the_day_never_moves_when_the_hour_passes_twelve() {
        let mut dial = dial_for(at(11, 0));
        for _ in 0..3 {
            dial.apply(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        assert_eq!(dial.value(HOUR), "2");
        assert_eq!(dial.value(HALF), "am", "and stayed where it was put");
        assert_eq!(read(&dial), at(2, 0));
    }

    #[test]
    fn the_half_of_the_day_toggles_whichever_way_it_is_turned() {
        let mut dial = dial_for(at(1, 0));
        dial.column = HALF;
        dial.apply(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(read(&dial), at(13, 0));
        dial.apply(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(read(&dial), at(1, 0));
    }
}
