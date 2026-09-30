//! The facts on the 3am screen.
//!
//! This is what a parent reads while holding a crying baby, so two rules hold
//! throughout: nothing goes negative, and nothing that was never recorded
//! comes back as a zero. A "0m ago" that really means "we have no idea" is the
//! kind of number that sends somebody to wake a sleeping baby.

use super::time::Calendar;
use super::today::{self, DayRule, Totals, Window};
use super::types::{Dataset, FeedEvent};

/// How far back "recently" reaches on the 3am screen.
///
/// Four hours, because that spans a newborn's usual feeding interval with room
/// to spare, and the question being asked is whether one is due, not what the
/// day looks like.
pub const RECENT_HOURS: f64 = 4.0;

/// Where an instant falls relative to the family's night.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NightPosition {
    /// Whether the instant is inside a night.
    pub inside: bool,
    /// Which night it belongs to, named by the day the night began on.
    pub night_of: jiff::civil::Date,
}

/// Whether an instant is inside a night, and which night that is.
///
/// Outside a night, `night_of` names the most recent completed one, which is
/// what "last night" has to mean at noon.
#[must_use]
pub fn night_position(
    calendar: &Calendar,
    at: f64,
    night_start_hour: f64,
    morning_cutoff_hour: f64,
) -> NightPosition {
    let today = calendar.day_of(at);
    let tonight_begins = calendar.at_hour_fraction(today, night_start_hour);
    if at >= tonight_begins {
        return NightPosition {
            inside: true,
            night_of: today,
        };
    }
    let yesterday = calendar.offset_day(today, -1);
    let last_night_ends = calendar.at_hour_fraction(today, morning_cutoff_hour);
    NightPosition {
        inside: at < last_night_ends,
        night_of: yesterday,
    }
}

/// The window of the night that begins on a day.
#[must_use]
pub fn night_window(
    calendar: &Calendar,
    night_of: jiff::civil::Date,
    night_start_hour: f64,
    morning_cutoff_hour: f64,
) -> (f64, f64) {
    (
        calendar.at_hour_fraction(night_of, night_start_hour),
        calendar.at_hour_fraction(calendar.offset_day(night_of, 1), morning_cutoff_hour),
    )
}

/// The last feed, and how long ago it was.
#[derive(Debug, Clone, PartialEq)]
pub struct LastFeed {
    /// When it was.
    pub start: f64,
    /// How long ago, never negative.
    pub ago_seconds: f64,
    /// The feed itself.
    pub feed: FeedEvent,
}

/// The last diaper, and how long ago it was.
#[derive(Debug, Clone, PartialEq)]
pub struct LastDiaper {
    /// When it was.
    pub start: f64,
    /// How long ago, never negative.
    pub ago_seconds: f64,
    /// Whether it was wet.
    pub wet: bool,
    /// Whether it was dirty.
    pub dirty: bool,
}

impl LastDiaper {
    /// `wet + dirty`, `wet`, `dirty`, or `dry`.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match (self.wet, self.dirty) {
            (true, true) => "wet + dirty",
            (true, false) => "wet",
            (false, true) => "dirty",
            (false, false) => "dry",
        }
    }
}

/// Asleep or awake, and for how long.
#[derive(Debug, Clone, PartialEq)]
pub struct SleepState {
    /// Whether a sleep is running right now.
    pub asleep: bool,
    /// Whether that sleep is paused.
    pub paused: bool,
    /// How long asleep, when asleep.
    pub asleep_seconds: Option<f64>,
    /// How long awake, when awake and there is a last sleep to measure from.
    pub awake_seconds: Option<f64>,
    /// When the last recorded sleep ended.
    pub last_sleep_end: Option<f64>,
}

/// The longest unbroken stretch of a night.
#[derive(Debug, Clone, PartialEq)]
pub struct LongestStretch {
    /// Whether this is the night in progress or the last completed one.
    pub tonight: bool,
    /// Which night, named by the day it began on.
    pub night_of: jiff::civil::Date,
    /// The longest stretch, when there was one.
    pub seconds: Option<f64>,
    /// When that stretch began.
    pub start: Option<f64>,
}

/// A nursing session in progress.
#[derive(Debug, Clone, PartialEq)]
pub struct NursingNow {
    /// Which side.
    pub side: String,
    /// How long so far.
    pub elapsed_seconds: f64,
    /// Whether it is paused.
    pub paused: bool,
}

/// Everything the 3am screen shows.
#[derive(Debug, Clone, PartialEq)]
pub struct NowView {
    /// The last feed.
    pub last_feed: Option<LastFeed>,
    /// The last diaper.
    pub last_diaper: Option<LastDiaper>,
    /// Asleep or awake.
    pub sleep_state: SleepState,
    /// The longest stretch of the relevant night.
    pub longest_stretch: LongestStretch,
    /// A nursing session in progress.
    pub nursing_now: Option<NursingNow>,
    /// What has gone in over the last [`RECENT_HOURS`].
    pub recent: Totals,
    /// What today amounts to, today meaning whatever this family counts.
    pub today: Totals,
    /// Which window that was, so a label never has to guess what it means.
    pub today_window: Window,
}

/// Builds the 3am screen's facts.
///
/// The rule is passed in rather than read from the dataset, because how a
/// family counts a day is a decision they made and not something Huckleberry
/// knows about.
#[must_use]
pub fn build(dataset: &Dataset, calendar: &Calendar, rule: DayRule, now: f64) -> NowView {
    let last_feed = dataset.last_feed().map(|feed| LastFeed {
        start: feed.start(),
        ago_seconds: since(feed.start(), now),
        feed: feed.clone(),
    });

    let last_diaper = dataset.last_diaper().map(|diaper| LastDiaper {
        start: diaper.start,
        ago_seconds: since(diaper.start, now),
        wet: diaper.wet,
        dirty: diaper.dirty,
    });

    // The live timer wins over history: a sleep in progress has not been
    // written to the intervals collection yet, so history cannot know.
    let asleep = dataset.live.sleep_active && dataset.live.sleep_start.is_some();
    let last_sleep_end = dataset.last_sleep().map(super::types::SleepEvent::end);
    let sleep_state = SleepState {
        asleep,
        paused: dataset.live.sleep_paused,
        asleep_seconds: if asleep {
            dataset.live.sleep_start.map(|start| since(start, now))
        } else {
            None
        },
        awake_seconds: if asleep {
            None
        } else {
            last_sleep_end.map(|end| since(end, now))
        },
        last_sleep_end,
    };

    let position = night_position(
        calendar,
        now,
        dataset.child.night_start_hour,
        dataset.child.morning_cutoff_hour,
    );
    let (window_start, window_end) = night_window(
        calendar,
        position.night_of,
        dataset.child.night_start_hour,
        dataset.child.morning_cutoff_hour,
    );
    let longest = dataset
        .sleep
        .iter()
        .filter(|sleep| sleep.start >= window_start && sleep.start < window_end)
        .max_by(|left, right| {
            left.duration
                .partial_cmp(&right.duration)
                .unwrap_or(core::cmp::Ordering::Equal)
        });

    let today_window = rule.window(calendar, now);
    NowView {
        recent: today::totals(dataset, now - RECENT_HOURS * 3600.0, now),
        today: today::totals(dataset, today_window.start, today_window.end),
        today_window,
        last_feed,
        last_diaper,
        sleep_state,
        longest_stretch: LongestStretch {
            tonight: position.inside,
            night_of: position.night_of,
            seconds: longest.map(|sleep| sleep.duration),
            start: longest.map(|sleep| sleep.start),
        },
        nursing_now: nursing_now(dataset, now),
    }
}

fn nursing_now(dataset: &Dataset, now: f64) -> Option<NursingNow> {
    if !dataset.live.nursing_active {
        return None;
    }
    let start = dataset.live.nursing_start?;
    Some(NursingNow {
        side: dataset
            .live
            .nursing_side
            .clone()
            .unwrap_or_else(|| "none".to_owned()),
        elapsed_seconds: since(start, now),
        paused: dataset.live.nursing_paused,
    })
}

/// How long ago, clamped at zero. A clock a few seconds out of step must never
/// produce a feed that happened in the future.
fn since(then: f64, now: f64) -> f64 {
    (now - then).max(0.0)
}

#[cfg(test)]
mod nights {
    use super::super::fixtures::{AFTERNOON, THREE_AM};
    use super::*;

    fn calendar() -> Calendar {
        Calendar::new("America/New_York").expect("a real timezone")
    }

    #[test]
    fn three_in_the_morning_is_inside_the_night_that_began_yesterday() {
        let position = night_position(&calendar(), THREE_AM, 20.0, 7.0);
        assert!(position.inside);
        assert_eq!(position.night_of.to_string(), "2025-09-22");
    }

    #[test]
    fn the_afternoon_is_outside_a_night_and_last_night_is_the_one_before() {
        let position = night_position(&calendar(), AFTERNOON, 20.0, 7.0);
        assert!(!position.inside);
        assert_eq!(position.night_of.to_string(), "2025-09-21");
    }

    #[test]
    fn the_evening_is_inside_tonight() {
        // 2025-09-23T01:00:00Z is 9pm on the 22nd in New York.
        let position = night_position(&calendar(), 1_758_589_200.0, 20.0, 7.0);
        assert!(position.inside);
        assert_eq!(position.night_of.to_string(), "2025-09-22");
    }

    #[test]
    fn a_night_window_spans_midnight() {
        let calendar = calendar();
        let (start, end) =
            night_window(&calendar, "2025-09-22".parse().expect("a date"), 20.0, 7.0);
        assert_eq!(calendar.zoned(start).hour(), 20);
        assert_eq!(calendar.zoned(end).hour(), 7);
        assert_eq!(calendar.day_of(end).to_string(), "2025-09-23");
    }

    #[test]
    fn a_family_that_starts_the_night_later_gets_their_own_window() {
        let position = night_position(&calendar(), 1_758_589_200.0, 22.0, 7.0);
        assert!(!position.inside, "9pm is not yet night for a 10pm family");
    }
}

#[cfg(test)]
mod facts {
    use super::super::fixtures::{AFTERNOON, THREE_AM, bottle, dataset, diaper, sleep};
    use super::*;

    fn calendar() -> Calendar {
        Calendar::new("America/New_York").expect("a real timezone")
    }

    #[test]
    fn with_nothing_logged_every_fact_is_absent_rather_than_zero() {
        let view = build(&dataset(), &calendar(), DayRule::assumed(), AFTERNOON);
        assert!(view.last_feed.is_none());
        assert!(view.last_diaper.is_none());
        assert!(view.sleep_state.awake_seconds.is_none());
        assert!(view.longest_stretch.seconds.is_none());
    }

    #[test]
    fn the_last_feed_is_the_newest_one() {
        let mut data = dataset();
        data.feeds = vec![
            bottle(AFTERNOON - 7200.0, 60.0),
            bottle(AFTERNOON - 3600.0, 90.0),
        ];
        let view = build(&data, &calendar(), DayRule::assumed(), AFTERNOON);
        let last = view.last_feed.expect("a feed");
        assert!((last.ago_seconds - 3600.0).abs() < f64::EPSILON);
        assert_eq!(last.feed.millilitres(), Some(90.0));
    }

    #[test]
    fn a_feed_logged_a_moment_in_the_future_reads_as_now_rather_than_negative() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON + 30.0, 90.0)];
        let view = build(&data, &calendar(), DayRule::assumed(), AFTERNOON);
        assert!((view.last_feed.expect("a feed").ago_seconds).abs() < f64::EPSILON);
    }

    #[test]
    fn a_running_sleep_counts_up_and_no_awake_time_is_shown() {
        let mut data = dataset();
        data.sleep = vec![sleep(AFTERNOON - 20_000.0, 3_600.0)];
        data.live.sleep_active = true;
        data.live.sleep_start = Some(AFTERNOON - 1_800.0);
        let view = build(&data, &calendar(), DayRule::assumed(), AFTERNOON);
        assert!(view.sleep_state.asleep);
        assert_eq!(view.sleep_state.asleep_seconds, Some(1_800.0));
        assert_eq!(view.sleep_state.awake_seconds, None);
    }

    #[test]
    fn awake_time_is_measured_from_the_end_of_the_last_sleep() {
        let mut data = dataset();
        data.sleep = vec![sleep(AFTERNOON - 7_200.0, 3_600.0)];
        let view = build(&data, &calendar(), DayRule::assumed(), AFTERNOON);
        assert!(!view.sleep_state.asleep);
        assert_eq!(view.sleep_state.awake_seconds, Some(3_600.0));
    }

    #[test]
    fn at_three_in_the_morning_the_stretch_is_tonights_so_far() {
        let mut data = dataset();
        // 11pm on the 22nd, for two hours: inside the night of the 22nd.
        data.sleep = vec![sleep(THREE_AM - 14_400.0, 7_200.0)];
        let view = build(&data, &calendar(), DayRule::assumed(), THREE_AM);
        assert!(view.longest_stretch.tonight);
        assert_eq!(view.longest_stretch.night_of.to_string(), "2025-09-22");
        assert_eq!(view.longest_stretch.seconds, Some(7_200.0));
    }

    #[test]
    fn in_the_afternoon_the_stretch_is_last_nights_and_names_that_night() {
        let mut data = dataset();
        // Inside the night of the 21st.
        data.sleep = vec![sleep(AFTERNOON - 50_400.0, 10_800.0)];
        let view = build(&data, &calendar(), DayRule::assumed(), AFTERNOON);
        assert!(!view.longest_stretch.tonight);
        assert_eq!(view.longest_stretch.night_of.to_string(), "2025-09-21");
        assert_eq!(view.longest_stretch.seconds, Some(10_800.0));
    }

    #[test]
    fn the_longest_stretch_is_the_longest_not_the_latest() {
        let mut data = dataset();
        data.sleep = vec![
            sleep(THREE_AM - 21_600.0, 10_800.0),
            sleep(THREE_AM - 3_600.0, 1_800.0),
        ];
        let view = build(&data, &calendar(), DayRule::assumed(), THREE_AM);
        assert_eq!(view.longest_stretch.seconds, Some(10_800.0));
    }

    #[test]
    fn a_sleep_outside_the_night_window_is_not_a_night_stretch() {
        let mut data = dataset();
        // A nap at 2pm on the 22nd, read at 3am on the 23rd.
        data.sleep = vec![sleep(AFTERNOON, 5_400.0)];
        let view = build(&data, &calendar(), DayRule::assumed(), THREE_AM);
        assert_eq!(view.longest_stretch.seconds, None);
    }

    #[test]
    fn a_diaper_is_labelled_by_what_was_in_it() {
        let mut data = dataset();
        data.diapers = vec![diaper(AFTERNOON - 600.0, true, true)];
        let view = build(&data, &calendar(), DayRule::assumed(), AFTERNOON);
        assert_eq!(view.last_diaper.expect("a diaper").label(), "wet + dirty");
    }

    #[test]
    fn a_nursing_session_in_progress_is_reported_with_its_side() {
        let mut data = dataset();
        data.live.nursing_active = true;
        data.live.nursing_start = Some(AFTERNOON - 300.0);
        data.live.nursing_side = Some("right".to_owned());
        let view = build(&data, &calendar(), DayRule::assumed(), AFTERNOON);
        let nursing = view.nursing_now.expect("a session");
        assert_eq!(nursing.side, "right");
        assert!((nursing.elapsed_seconds - 300.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_stale_inactive_timer_is_not_a_session_in_progress() {
        let mut data = dataset();
        data.live.nursing_start = Some(AFTERNOON - 300.0);
        assert!(
            build(&data, &calendar(), DayRule::assumed(), AFTERNOON)
                .nursing_now
                .is_none()
        );
    }
}
