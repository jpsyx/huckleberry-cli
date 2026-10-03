//! The dial, configured to answer a time.

use anyhow::Result;

use super::model::Dial;
use super::terminal::Outcome;
use super::wheel::Wheel;
use crate::domain::clock::{self, TimeOfDay};
use crate::domain::time::Calendar;
use crate::theme::Theme;

/// How far a shifted turn moves the minutes, in either way of saying a time.
///
/// Five. A plain turn is one minute, because a minute is the correction
/// somebody came to make; five is the reach for when it is further off.
const MINUTE_LEAP: usize = 5;

/// The furthest back the relative dial reaches without being told otherwise.
///
/// Just under a day. Past that, "when" is a clock time rather than a count of
/// hours, and Tab is right there.
const MOST_HOURS_AGO: i64 = 23;

/// The columns of the clock, left to right.
const HOUR: usize = 0;
const MINUTE: usize = 1;
const HALF: usize = 2;

/// The columns of the relative dial.
const HOURS_AGO: usize = 0;
const MINUTES_AGO: usize = 1;

/// What the row that is the current time is called.
const NOW: &str = "[now]";

/// What the relative dial is counting, said once on the answering row.
const AGO: &str = "ago";

/// Which way of saying a time the dial is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// How long ago, in hours and minutes.
    Relative,
    /// What the clock said.
    Absolute,
}

/// Where a time dial opens, and what it may answer.
#[derive(Debug, Clone, Copy)]
pub struct Opening {
    /// The instant it opens standing on.
    pub at: f64,
    /// Which way of saying it to open in.
    pub mode: Mode,
    /// The earliest instant the answer may be, if something already fixes
    /// one: a sleep's end may not land before its start.
    pub not_before: Option<f64>,
}

/// Asks for a time, in whichever way the answer comes more easily.
///
/// Answers in words the typed field would have taken, so every parser and
/// date rule below this is untouched by which dial said it: `20 min ago` or
/// `1:44 pm`.
///
/// # Errors
///
/// Cancelled when somebody backs out, and a failure when there is no screen.
pub fn ask(
    label: &str,
    calendar: &Calendar,
    now: f64,
    opening: Opening,
    theme: Theme,
) -> Result<String> {
    let furthest = furthest_back(now, opening.not_before);
    let mut mode = opening.mode;
    let mut at = opening.at;
    loop {
        match turn(label, calendar, now, at, mode, furthest, theme)? {
            Said::Answer(answer) => return Ok(answer),
            Said::TheOtherWay(instant) => {
                at = instant;
                mode = other(mode);
            }
        }
    }
}

/// What one pass at the dial produced.
enum Said {
    /// The answer, in words the typed field would have taken.
    Answer(String),
    /// Tab: say it the other way, standing on this instant.
    TheOtherWay(f64),
}

/// One pass at the dial, in one way of saying it.
fn turn(
    label: &str,
    calendar: &Calendar,
    now: f64,
    at: f64,
    mode: Mode,
    furthest: i64,
    theme: Theme,
) -> Result<Said> {
    match mode {
        Mode::Relative => {
            let mut settle = settling(furthest);
            let dial = relative_dial(ago(now, at).clamp(0, furthest), furthest);
            Ok(
                match super::terminal::ask(label, dial, theme, &mut settle)? {
                    Outcome::Submitted(dial) => Said::Answer(said(read_relative(&dial))),
                    Outcome::Switched(dial) => {
                        Said::TheOtherWay(now - read_relative(&dial) as f64 * 60.0)
                    }
                },
            )
        }
        Mode::Absolute => {
            let dial = clock_dial(calendar.time_of_day(at), calendar.time_of_day(now));
            let mut nothing = |_: &mut Dial, _: usize| {};
            Ok(
                match super::terminal::ask(label, dial, theme, &mut nothing)? {
                    Outcome::Submitted(dial) => Said::Answer(read_clock(&dial).label()),
                    Outcome::Switched(dial) => {
                        Said::TheOtherWay(clock::most_recent(read_clock(&dial), now, calendar))
                    }
                },
            )
        }
    }
}

/// The other way of saying it.
const fn other(mode: Mode) -> Mode {
    match mode {
        Mode::Relative => Mode::Absolute,
        Mode::Absolute => Mode::Relative,
    }
}

/// How many whole minutes ago an instant was, never fewer than none.
fn ago(now: f64, at: f64) -> i64 {
    (((now - at) / 60.0).round() as i64).max(0)
}

/// The furthest back the relative dial may reach, in minutes.
///
/// A sleep that ends before it began is not a duration anybody can store, so
/// where a start is already fixed the dial simply does not offer anything
/// earlier. Refusing afterwards would be a worse answer to the same problem:
/// it makes somebody undo a turn they were never meant to be able to make.
fn furthest_back(now: f64, not_before: Option<f64>) -> i64 {
    let unbounded = MOST_HOURS_AGO * 60 + 59;
    not_before.map_or(unbounded, |earliest| {
        (((now - earliest) / 60.0).floor() as i64).clamp(0, unbounded)
    })
}

/// How a relative answer reads back.
fn said(minutes: i64) -> String {
    if minutes == 0 {
        "now".to_owned()
    } else {
        format!("{minutes} min ago")
    }
}

// -- Saying it as how long ago ----------------------------------------------

/// Hours and minutes ago, reaching no further than `furthest`.
fn relative_dial(minutes: i64, furthest: i64) -> Dial {
    let hours = (minutes / 60).min(furthest / 60);
    Dial::new(vec![
        hours_wheel(furthest, hours),
        minutes_wheel(allowed(hours, furthest), minutes % 60),
    ])
    .marking(NOW, vec![hour_label(0), minute_label(0)])
    .reading(AGO)
    .switching_to("clock")
}

/// How many minutes the dial may still offer once `hours` are spoken for.
fn allowed(hours: i64, furthest: i64) -> i64 {
    (furthest - hours * 60).clamp(0, 59)
}

fn hour_label(hours: i64) -> String {
    format!("{hours}h")
}

fn minute_label(minutes: i64) -> String {
    format!("{minutes:02}m")
}

/// The hours column holds at nought rather than coming round.
///
/// Nothing is less than no hours ago, and a column that came round would log
/// yesterday on one press of the wrong key. The minutes still come round,
/// because the hour above them carries it.
fn hours_wheel(furthest: i64, standing: i64) -> Wheel {
    Wheel::new(
        (0..=furthest / 60).map(hour_label).collect(),
        usize::try_from(standing).unwrap_or_default(),
        1,
        "",
    )
    .holding()
}

fn minutes_wheel(most: i64, standing: i64) -> Wheel {
    Wheel::new(
        (0..=most).map(minute_label).collect(),
        usize::try_from(standing.min(most)).unwrap_or_default(),
        MINUTE_LEAP,
        "   ",
    )
}

/// Keeps the minutes within what the hours have left.
///
/// Turning the hours to the furthest back the dial reaches leaves fewer than
/// sixty minutes on offer, so the minutes column is rebuilt to just those.
/// Turning the hours back again gives them back.
fn settling(furthest: i64) -> impl FnMut(&mut Dial, usize) {
    move |dial, column| {
        if column != HOURS_AGO {
            return;
        }
        let hours = read_number(dial.value(HOURS_AGO));
        let held = read_number(dial.value(MINUTES_AGO));
        if let Some(wheel) = dial.wheels.get_mut(MINUTES_AGO) {
            *wheel = minutes_wheel(allowed(hours, furthest), held);
        }
    }
}

/// How many minutes ago the relative dial is standing on.
fn read_relative(dial: &Dial) -> i64 {
    read_number(dial.value(HOURS_AGO)) * 60 + read_number(dial.value(MINUTES_AGO))
}

/// The digits in a label such as `2h` or `05m`.
fn read_number(label: &str) -> i64 {
    label
        .trim_end_matches(|glyph: char| !glyph.is_ascii_digit())
        .parse()
        .unwrap_or_default()
}

// -- Saying it as a clock time ----------------------------------------------

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
fn clock_dial(time: TimeOfDay, now: TimeOfDay) -> Dial {
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
    .switching_to("ago")
}

/// The time the clock dial is standing on.
fn read_clock(dial: &Dial) -> TimeOfDay {
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

    fn down() -> KeyEvent {
        KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)
    }

    fn leap_down() -> KeyEvent {
        KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT)
    }

    fn up() -> KeyEvent {
        KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)
    }

    /// Nothing is less than no hours ago, and a column that came round would
    /// log yesterday on one press of the wrong key.
    #[test]
    fn the_hours_stop_at_nothing_ago_rather_than_coming_round() {
        let mut dial = relative_dial(0, 1439);
        dial.apply(up());
        assert_eq!(dial.value(HOURS_AGO), "0h");
        assert_eq!(read_relative(&dial), 0);
    }

    #[test]
    fn nothing_is_drawn_above_nothing_ago() {
        let dial = relative_dial(0, 1439);
        assert_eq!(dial.wheels[HOURS_AGO].shown_at(-1), "");
        assert_eq!(dial.wheels[HOURS_AGO].shown_at(1), "1h", "but below it is");
    }

    /// The minutes still come round, because the hour above them carries it.
    #[test]
    fn the_minutes_still_come_round() {
        let mut dial = relative_dial(0, 1439);
        dial.column = MINUTES_AGO;
        dial.apply(up());
        assert_eq!(dial.value(MINUTES_AGO), "59m");
    }

    /// Two numbers and a letter each is not a sentence. The word says which
    /// way the dial is counting without anybody having to work it out.
    #[test]
    fn the_relative_dial_spells_out_ago() {
        let drawn = super::super::draw::render(
            "When?",
            &relative_dial(66, 1439),
            60,
            crate::theme::Theme::dark(false),
        );
        let centre = drawn
            .iter()
            .find(|line| line.contains("1h") && line.contains("06m"))
            .expect("a centre row");
        assert!(centre.trim_end().ends_with("ago"), "{centre:?}");
        let others = drawn
            .iter()
            .filter(|line| line.contains("ago") && line.contains('m'))
            .count();
        assert_eq!(others, 1, "and says it once, on the row that is the answer");
    }

    /// The clock says what time it was, not how long ago, so it says nothing.
    #[test]
    fn the_clock_dial_spells_out_nothing() {
        let drawn = super::super::draw::render(
            "When?",
            &clock_dial(at(13, 44), at(13, 44)),
            60,
            crate::theme::Theme::dark(false),
        );
        assert!(
            !drawn
                .iter()
                .any(|line| line.contains(" : ") && line.contains("ago")),
            "{drawn:?}"
        );
    }

    /// Nothing ago is now, and the dial says so without being asked.
    #[test]
    fn the_relative_dial_opens_on_nothing_ago() {
        let dial = relative_dial(0, 600);
        assert_eq!(dial.value(HOURS_AGO), "0h");
        assert_eq!(dial.value(MINUTES_AGO), "00m");
        assert_eq!(read_relative(&dial), 0);
        assert!(dial.is_landmark(0), "and marks it as now");
    }

    #[test]
    fn nothing_ago_reads_back_as_now_rather_than_as_no_minutes() {
        assert_eq!(said(0), "now");
        assert_eq!(said(1), "1 min ago");
        assert_eq!(said(90), "90 min ago");
    }

    #[test]
    fn the_two_columns_add_up_to_the_answer() {
        let mut dial = relative_dial(0, 600);
        dial.apply(down());
        dial.apply(down());
        dial.column = MINUTES_AGO;
        for _ in 0..7 {
            dial.apply(down());
        }
        assert_eq!(read_relative(&dial), 2 * 60 + 7);
        assert_eq!(said(read_relative(&dial)), "127 min ago");
    }

    #[test]
    fn the_minutes_turn_one_and_leap_five() {
        let mut dial = relative_dial(0, 600);
        dial.column = MINUTES_AGO;
        dial.apply(down());
        assert_eq!(dial.value(MINUTES_AGO), "01m");
        dial.apply(leap_down());
        assert_eq!(dial.value(MINUTES_AGO), "06m");
    }

    /// A sleep that ends before it begins is not a duration anybody can
    /// store, so the dial simply does not offer anything earlier.
    #[test]
    fn the_dial_cannot_reach_back_past_a_start_already_fixed() {
        let furthest = 90;
        let mut dial = relative_dial(0, furthest);
        let mut settle = settling(furthest);

        // The hours reach one and no further.
        for _ in 0..5 {
            dial.apply(down());
            settle(&mut dial, HOURS_AGO);
        }
        assert_eq!(dial.wheels[HOURS_AGO].count(), 2, "nought and one");

        // And at one hour, only thirty minutes are left to give.
        dial.apply(down());
        settle(&mut dial, HOURS_AGO);
        while dial.value(HOURS_AGO) != "1h" {
            dial.apply(down());
            settle(&mut dial, HOURS_AGO);
        }
        assert_eq!(dial.wheels[MINUTES_AGO].count(), 31, "nought to thirty");
        dial.column = MINUTES_AGO;
        for _ in 0..90 {
            dial.apply(down());
            assert!(
                read_relative(&dial) <= furthest,
                "{} minutes is past the start",
                read_relative(&dial)
            );
        }
    }

    /// Turning the hours back gives the minutes their range again.
    #[test]
    fn the_minutes_get_their_range_back_when_the_hours_come_down() {
        let furthest = 90;
        let mut dial = relative_dial(60, furthest);
        let mut settle = settling(furthest);
        assert_eq!(dial.wheels[MINUTES_AGO].count(), 31);
        dial.apply(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        settle(&mut dial, HOURS_AGO);
        assert_eq!(dial.value(HOURS_AGO), "0h");
        assert_eq!(dial.wheels[MINUTES_AGO].count(), 60, "all of them again");
    }

    #[test]
    fn without_a_start_to_respect_the_dial_reaches_most_of_a_day() {
        let now = 1_000_000.0;
        assert_eq!(furthest_back(now, None), 23 * 60 + 59);
        assert_eq!(furthest_back(now, Some(now - 5400.0)), 90);
        assert_eq!(
            furthest_back(now, Some(now + 100.0)),
            0,
            "a start in the future leaves nowhere to go"
        );
    }

    /// Both ways of saying it offer the other, so neither is a dead end.
    #[test]
    fn tab_leads_from_each_way_of_saying_it_to_the_other() {
        assert_eq!(relative_dial(0, 600).switch.as_deref(), Some("clock"));
        assert_eq!(
            clock_dial(at(13, 44), at(13, 44)).switch.as_deref(),
            Some("ago")
        );
        assert_eq!(other(Mode::Relative), Mode::Absolute);
        assert_eq!(other(Mode::Absolute), Mode::Relative);
    }

    /// The dial opens on now, so it says so.
    #[test]
    fn the_dial_points_out_now_when_it_is_standing_on_it() {
        let now = at(13, 44);
        let dial = clock_dial(now, now);
        assert!(dial.is_landmark(0));
        for offset in [-2, -1, 1, 2] {
            assert!(!dial.is_landmark(offset), "offset {offset}");
        }
    }

    /// Turning back by hand is a column at a time; one key is the way back.
    #[test]
    fn pressing_n_puts_the_dial_back_on_now() {
        let now = at(13, 44);
        let mut dial = clock_dial(at(9, 15), now);
        assert_eq!(read_clock(&dial), at(9, 15));
        dial.apply(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
        assert_eq!(read_clock(&dial), now, "every column at once");
        assert!(dial.is_landmark(0), "and it says so");
    }

    /// Turn away and the mark stays on the hour you left, which is the row
    /// to turn back to.
    #[test]
    fn turning_away_leaves_the_mark_on_the_row_that_puts_it_back() {
        let now = at(13, 44);
        let mut dial = clock_dial(now, now);
        dial.apply(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(read_clock(&dial), at(14, 44), "two in the afternoon now");
        assert!(dial.is_landmark(-1), "and one is the row above");
        assert!(!dial.is_landmark(0));
    }

    /// The mark is about the column being turned, so it moves with the
    /// cursor rather than staying on the hour.
    #[test]
    fn the_mark_moves_to_the_minutes_when_the_minutes_are_being_turned() {
        let now = at(13, 44);
        let mut dial = clock_dial(at(14, 44), now);
        assert!(dial.is_landmark(-1), "the hour it came from");
        dial.column = MINUTE;
        assert!(
            dial.is_landmark(0),
            "the minutes never moved, so theirs is the middle row"
        );
    }

    #[test]
    fn a_dial_opened_on_some_other_time_points_out_nothing() {
        let dial = clock_dial(at(9, 15), at(13, 44));
        for offset in -2..=2 {
            assert!(!dial.is_landmark(offset), "offset {offset}");
        }
    }

    /// Turning back to the current time brings it back.
    #[test]
    fn coming_back_to_now_points_it_out_again() {
        let now = at(13, 44);
        let mut dial = clock_dial(at(13, 42), now);
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
                assert_eq!(read_clock(&clock_dial(time, time)), time, "{hour}:{minute}");
            }
        }
    }

    #[test]
    fn midnight_and_noon_are_both_twelve_on_the_dial() {
        assert_eq!(clock_dial(at(0, 0), at(0, 0)).value(HOUR), "12");
        assert_eq!(clock_dial(at(0, 0), at(0, 0)).value(HALF), "am");
        assert_eq!(clock_dial(at(12, 0), at(12, 0)).value(HOUR), "12");
        assert_eq!(clock_dial(at(12, 0), at(12, 0)).value(HALF), "pm");
    }

    /// A plain turn is one minute and shift is five, which is the way round
    /// a correction usually wants.
    #[test]
    fn the_minutes_turn_one_at_a_time_and_leap_five_with_shift() {
        let mut dial = clock_dial(at(13, 44), at(13, 44));
        dial.column = MINUTE;
        dial.apply(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(read_clock(&dial), at(13, 45));
        dial.apply(KeyEvent::new(KeyCode::Down, KeyModifiers::SHIFT));
        assert_eq!(read_clock(&dial), at(13, 50));
        dial.apply(KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT));
        assert_eq!(read_clock(&dial), at(13, 45));
    }

    #[test]
    fn the_half_of_the_day_never_moves_when_the_hour_passes_twelve() {
        let mut dial = clock_dial(at(11, 0), at(11, 0));
        for _ in 0..3 {
            dial.apply(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        assert_eq!(dial.value(HOUR), "2");
        assert_eq!(dial.value(HALF), "am", "and stayed where it was put");
        assert_eq!(read_clock(&dial), at(2, 0));
    }

    #[test]
    fn the_half_of_the_day_toggles_whichever_way_it_is_turned() {
        let mut dial = clock_dial(at(1, 0), at(1, 0));
        dial.column = HALF;
        dial.apply(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(read_clock(&dial), at(13, 0));
        dial.apply(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(read_clock(&dial), at(1, 0));
    }
}
