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
pub fn ask(label: &str, initial: TimeOfDay, now: TimeOfDay, theme: Theme) -> Result<TimeOfDay> {
    // Nothing to put right between turns: the three columns are independent,
    // which is the whole reason the half of the day never moves on its own.
    let turned = super::terminal::ask(label, dial_for(initial, now), theme, |_, _| {})?;
    Ok(read(&turned))
}

/// How a time reads across the three columns, in the order they are drawn.
///
/// One function, so the labels a wheel is built from and the labels a
/// landmark is matched against can never disagree about how to write half
/// past one.
fn columns_of(time: TimeOfDay) -> [String; 3] {
    let (hour, afternoon) = split(time);
    [
        hour.to_string(),
        format!("{:02}", time.minute),
        if afternoon { "pm" } else { "am" }.to_owned(),
    ]
}

/// The hour as the dial shows it, and which half of the day it is in.
const fn split(time: TimeOfDay) -> (i8, bool) {
    match time.hour {
        0 => (12, false),
        12 => (12, true),
        hour if hour > 12 => (hour - 12, true),
        hour => (hour, false),
    }
}

/// The three wheels, standing at a time, pointing out the current one.
///
/// The mark goes on the row that is the current time and on no other. A row
/// showing the right minute under the wrong hour is not a time anybody meant,
/// and labelling it "now" would be worse than labelling nothing.
fn dial_for(time: TimeOfDay, now: TimeOfDay) -> Dial {
    let (hour, afternoon) = split(time);
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
    .marking(NOW, columns_of(now).to_vec())
}

/// What the row that is the current time is called.
const NOW: &str = "[now]";

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

    /// The dial opens on now, so it says so.
    #[test]
    fn the_dial_points_out_now_when_it_is_standing_on_it() {
        let now = at(13, 44);
        let dial = dial_for(now, now);
        assert!(dial.is_landmark(0));
        for offset in [-2, -1, 1, 2] {
            assert!(!dial.is_landmark(offset), "offset {offset}");
        }
    }

    /// Turning back by hand is a column at a time; one key is the way back.
    #[test]
    fn pressing_n_puts_the_dial_back_on_now() {
        let now = at(13, 44);
        let mut dial = dial_for(at(9, 15), now);
        assert_eq!(read(&dial), at(9, 15));
        dial.apply(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
        assert_eq!(read(&dial), now, "every column at once");
        assert!(dial.is_landmark(0), "and it says so");
    }

    /// Turn away and the mark stays on the hour you left, which is the row
    /// to turn back to.
    #[test]
    fn turning_away_leaves_the_mark_on_the_row_that_puts_it_back() {
        let now = at(13, 44);
        let mut dial = dial_for(now, now);
        dial.apply(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(read(&dial), at(14, 44), "two in the afternoon now");
        assert!(dial.is_landmark(-1), "and one is the row above");
        assert!(!dial.is_landmark(0));
    }

    /// The mark is about the column being turned, so it moves with the
    /// cursor rather than staying on the hour.
    #[test]
    fn the_mark_moves_to_the_minutes_when_the_minutes_are_being_turned() {
        let now = at(13, 44);
        let mut dial = dial_for(at(14, 44), now);
        assert!(dial.is_landmark(-1), "the hour it came from");
        dial.column = MINUTE;
        assert!(
            dial.is_landmark(0),
            "the minutes never moved, so theirs is the middle row"
        );
    }

    #[test]
    fn a_dial_opened_on_some_other_time_points_out_nothing() {
        let dial = dial_for(at(9, 15), at(13, 44));
        for offset in -2..=2 {
            assert!(!dial.is_landmark(offset), "offset {offset}");
        }
    }

    /// Turning back to the current time brings it back.
    #[test]
    fn coming_back_to_now_points_it_out_again() {
        let now = at(13, 44);
        let mut dial = dial_for(at(13, 42), now);
        dial.column = MINUTE;
        for _ in 0..2 {
            dial.apply(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        assert!(dial.is_landmark(0), "it is that time again");
    }

    #[test]
    fn a_time_goes_onto_the_dial_and_comes_back_unchanged() {
        for hour in 0..24 {
            for minute in [0, 7, 44, 59] {
                let time = at(hour, minute);
                assert_eq!(read(&dial_for(time, time)), time, "{hour}:{minute}");
            }
        }
    }

    #[test]
    fn midnight_and_noon_are_both_twelve_on_the_dial() {
        assert_eq!(dial_for(at(0, 0), at(0, 0)).value(HOUR), "12");
        assert_eq!(dial_for(at(0, 0), at(0, 0)).value(HALF), "am");
        assert_eq!(dial_for(at(12, 0), at(12, 0)).value(HOUR), "12");
        assert_eq!(dial_for(at(12, 0), at(12, 0)).value(HALF), "pm");
    }

    /// A plain turn is one minute and shift is five, which is the way round
    /// a correction usually wants.
    #[test]
    fn the_minutes_turn_one_at_a_time_and_leap_five_with_shift() {
        let mut dial = dial_for(at(13, 44), at(13, 44));
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
        let mut dial = dial_for(at(11, 0), at(11, 0));
        for _ in 0..3 {
            dial.apply(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        assert_eq!(dial.value(HOUR), "2");
        assert_eq!(dial.value(HALF), "am", "and stayed where it was put");
        assert_eq!(read(&dial), at(2, 0));
    }

    #[test]
    fn the_half_of_the_day_toggles_whichever_way_it_is_turned() {
        let mut dial = dial_for(at(1, 0), at(1, 0));
        dial.column = HALF;
        dial.apply(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(read(&dial), at(13, 0));
        dial.apply(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(read(&dial), at(1, 0));
    }
}
