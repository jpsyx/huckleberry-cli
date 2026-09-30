//! The 3am screen.
//!
//! Four facts, large and unadorned, in the order a parent needs them. The
//! screen answers "when did he last eat" before anything else because that is
//! the question that gets asked at 3am, and it carries an honest "as of" line
//! because a stale "last fed two hours ago" is what sends somebody to wake a
//! sleeping baby.
//!
//! **This module is the only place that decides what `now` says.** [`screen`]
//! returns the rows as text with a role on each piece; [`lines`] paints them
//! for stdout and the shell's Now drawer draws the same rows as widgets. The
//! drawer is `h now` left on the screen and nothing else, so a change to one
//! is a change to both, by construction rather than by remembering.

use crate::cli::Units;
use crate::domain::clock::TimeOfDay;
use crate::domain::now::{NowView, RECENT_HOURS};
use crate::domain::time::{Calendar, format_ago, format_duration, split_hour};
use crate::domain::today::{DayMode, Totals};
use crate::domain::types::{Dataset, FeedEvent};
use crate::listing::Piece;
use crate::theme::{Theme, Tone};

use super::format;

/// One row of the screen: its text, and the role each piece is painted in.
pub type Row = Vec<Piece>;

/// The screen, row by row, with nothing painted yet.
///
/// The single source of what `now` says. Anything that shows these facts draws
/// from here, so no second copy of the wording can drift from this one.
#[must_use]
pub fn screen(
    view: &NowView,
    dataset: &Dataset,
    calendar: &Calendar,
    units: Units,
    now: f64,
) -> Vec<Row> {
    let mut rows = vec![
        vec![Piece::new(dataset.child.name.clone(), Tone::Heading)],
        Row::new(),
        fact("Last fed", &feed_line(view, calendar, units, now)),
        fact("Diaper", &diaper_line(view, calendar, now)),
    ];
    rows.extend(sleep_rows(view, dataset, now));
    rows.push(fact(&stretch_label(view), &stretch_line(view, calendar)));

    if let Some(nursing) = &view.nursing_now {
        let state = if nursing.paused { " (paused)" } else { "" };
        rows.push(fact(
            "Nursing now",
            &format!(
                "{} on the {}{state}",
                format_duration(nursing.elapsed_seconds),
                nursing.side
            ),
        ));
    }

    // The running totals come after the four facts, because they answer the
    // second question rather than the first: not "when did she last eat" but
    // "has she had enough".
    rows.push(fact(
        &format!("Last {}h", RECENT_HOURS as i64),
        &intake(&view.recent, units),
    ));
    rows.push(fact(
        &format!("Fed {}", window_label(view)),
        &today_intake_line(view, units),
    ));
    rows.push(fact(
        &format!("Slept {}", window_label(view)),
        &slept(&view.today),
    ));

    rows.push(Row::new());
    rows.push(vec![Piece::new(as_of(dataset, now), Tone::Muted)]);
    for note in &dataset.notes {
        rows.push(vec![Piece::new(
            format!("  {} could not be read: {}", note.collection, note.problem),
            Tone::Muted,
        )]);
    }
    rows
}

/// The screen as lines of text, ready for stdout.
#[must_use]
pub fn lines(
    view: &NowView,
    dataset: &Dataset,
    calendar: &Calendar,
    theme: Theme,
    units: Units,
    now: f64,
) -> Vec<String> {
    screen(view, dataset, calendar, units, now)
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|piece| theme.paint(piece.tone, &piece.text))
                .collect()
        })
        .collect()
}

/// How wide the label column is, so a note under a value lines up with it
/// rather than with the label it belongs to.
const LABEL: usize = 13;

/// The gap between a label and its value.
const GAP: usize = 2;

/// One labelled fact. The gap travels with the label so that a screen with no
/// colour is the same text either way.
fn fact(label: &str, value: &str) -> Row {
    vec![
        Piece::new(
            format!("{}{}", format::pad(label, LABEL), " ".repeat(GAP)),
            Tone::Accent,
        ),
        Piece::new(value.to_owned(), Tone::Value),
    ]
}

/// A quieter line under a fact, indented to the value it is about.
///
/// The indent is its own piece so the note itself is one span of text: what is
/// said and what it is painted are then the same thing.
fn note(text: &str) -> Row {
    vec![
        Piece::new(" ".repeat(LABEL + GAP), Tone::Value),
        Piece::new(text.to_owned(), Tone::Muted),
    ]
}

/// How stale the numbers are.
///
/// Always present, and always honest: a screen that says "two hours ago"
/// without saying when it looked is a screen that can be wrong without
/// admitting it.
#[must_use]
pub fn as_of(dataset: &Dataset, now: f64) -> String {
    let age = now - dataset.fetched_at;
    if age < 90.0 {
        return "as of just now".to_owned();
    }
    format!("as of {}", format_ago(dataset.fetched_at, now))
}

fn feed_line(view: &NowView, calendar: &Calendar, units: Units, now: f64) -> String {
    let Some(last) = &view.last_feed else {
        return "nothing logged".to_owned();
    };
    let what = match &last.feed {
        FeedEvent::Bottle {
            amount_ml,
            bottle_type,
            ..
        } => format!(
            "{} of {}",
            format::volume(*amount_ml, units),
            bottle_type.as_deref().unwrap_or("milk")
        ),
        FeedEvent::Nursing {
            left_seconds,
            right_seconds,
            last_side,
            ..
        } => format!(
            "nursed {} on the {}",
            format_duration(left_seconds + right_seconds),
            last_side.as_deref().unwrap_or("breast")
        ),
        FeedEvent::Solids { foods, .. } => {
            if foods.is_empty() {
                "solids".to_owned()
            } else {
                foods.join(", ")
            }
        }
    };
    format!(
        "{} · {what} · {}",
        format_ago(last.start, now),
        format::clock(last.start, calendar)
    )
}

fn diaper_line(view: &NowView, calendar: &Calendar, now: f64) -> String {
    let Some(last) = &view.last_diaper else {
        return "nothing logged".to_owned();
    };
    format!(
        "{} · {} · {}",
        format_ago(last.start, now),
        last.label(),
        format::clock(last.start, calendar)
    )
}

/// Current sleep leads; the last completed sleep is secondary while asleep.
///
/// Two lines rather than one while a sleep is running. The running one is what
/// somebody opened this for, and one long line holding both is a line a tired
/// eye reads twice to find where the first fact ends.
///
/// The note says *finished*, because that is what it measures: how long ago
/// the previous sleep ended, not when it began. It carries how long that sleep
/// ran as well, which is the question asked straight after.
fn sleep_rows(view: &NowView, dataset: &Dataset, now: f64) -> Vec<Row> {
    let state = &view.sleep_state;
    if !state.asleep {
        let previous = dataset.last_sleep().map_or_else(
            || "no sleep logged".to_owned(),
            |sleep| {
                format!(
                    "{} · slept for {}",
                    format_ago(sleep.end(), now),
                    format_duration(sleep.duration)
                )
            },
        );
        return vec![fact("Sleep", &previous)];
    }

    let paused = if state.paused { " (timer paused)" } else { "" };
    let current = state.asleep_seconds.map_or_else(
        || format!("currently sleeping{paused}"),
        |seconds| {
            format!(
                "currently sleeping for {}{paused}",
                format_duration(seconds)
            )
        },
    );
    let mut rows = vec![fact("Sleep", &current)];
    if let Some(sleep) = dataset.last_sleep() {
        rows.push(note(&format!(
            "(previous sleep finished {} · slept for {})",
            format_ago(sleep.end(), now),
            format_duration(sleep.duration)
        )));
    }
    rows
}

/// What the running totals call their window.
///
/// A rolling window says so, because "today" for a family counting a rolling
/// day would be a word doing the opposite of its job at 3am: the whole point
/// of that mode is that there is no today.
#[must_use]
pub const fn window_label(view: &NowView) -> &'static str {
    match view.today_window.mode {
        DayMode::Continuous => "in 24h",
        DayMode::Discrete => "today",
    }
}

/// What went in over a window, in as few words as it takes to be exact.
///
/// Shared with the shell's Now widget, so the two never word the same figure
/// differently.
#[must_use]
pub fn intake(totals: &Totals, units: Units) -> String {
    let mut parts = Vec::new();
    if totals.millilitres > 0.0 {
        parts.push(format::volume(Some(totals.millilitres), units));
    }
    if totals.nursing_seconds > 0.0 {
        parts.push(format!(
            "nursed {}",
            format_duration(totals.nursing_seconds)
        ));
    }
    if totals.milk_feeds > 0 {
        parts.push(plural(totals.milk_feeds, "feed", "feeds"));
    }
    if totals.solids > 0 {
        parts.push(plural(totals.solids, "meal", "meals"));
    }
    if parts.is_empty() {
        // Not "0 ml": that is a claim about the baby, and this is a claim
        // about the record.
        return "nothing logged".to_owned();
    }
    parts.join(" · ")
}

/// Today's intake, with the hour the day began on it when there was one.
///
/// The hour is said once, on this line rather than on both, because two lines
/// carrying the same qualifier reads as two different windows.
fn today_intake_line(view: &NowView, units: Units) -> String {
    let line = intake(&view.today, units);
    let Some(hour) = view.today_window.began_at_hour else {
        return line;
    };
    let (hours, minutes) = split_hour(hour);
    TimeOfDay::new(hours, minutes).map_or_else(
        || line.clone(),
        |time| format!("{line} · since {}", time.label()),
    )
}

/// How much sleep the window holds.
#[must_use]
pub fn slept(totals: &Totals) -> String {
    if totals.sleep_seconds > 0.0 {
        return format_duration(totals.sleep_seconds);
    }
    "nothing logged".to_owned()
}

/// `1 feed`, `6 feeds`.
fn plural(count: usize, one: &str, many: &str) -> String {
    if count == 1 {
        format!("{count} {one}")
    } else {
        format!("{count} {many}")
    }
}

/// The label the longest-stretch line gets.
///
/// It relabels itself rather than saying something that reads correctly at
/// noon and wrongly at 3am. Inside the night it is "tonight so far"; outside
/// it names the night it is talking about.
#[must_use]
pub fn stretch_label(view: &NowView) -> String {
    if view.longest_stretch.tonight {
        "Tonight".to_owned()
    } else {
        format!(
            "Night of {}",
            format::day_short(view.longest_stretch.night_of)
        )
    }
}

fn stretch_line(view: &NowView, calendar: &Calendar) -> String {
    let stretch = &view.longest_stretch;
    let Some(seconds) = stretch.seconds else {
        // "no sleep yet" would contradict the line above it when a sleep is
        // running: a stretch is a *finished* sleep, and the difference is
        // exactly what somebody reads this line for.
        return if stretch.tonight {
            "nothing finished yet".to_owned()
        } else {
            "nothing logged".to_owned()
        };
    };
    let qualifier = if stretch.tonight { " so far" } else { "" };
    stretch.start.map_or_else(
        || format!("longest{qualifier} {}", format_duration(seconds)),
        |start| {
            format!(
                "longest{qualifier} {} · from {}",
                format_duration(seconds),
                format::clock(start, calendar)
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::{AFTERNOON, THREE_AM, bottle, dataset, diaper, sleep};
    use crate::domain::now;

    fn calendar() -> Calendar {
        Calendar::new("America/New_York").expect("a real timezone")
    }

    /// Days from 6am, which is what setup asks a family with an older baby.
    fn rule() -> crate::domain::today::DayRule {
        crate::domain::today::DayRule::discrete(6.0, 19.5)
    }

    fn screen(dataset: &Dataset, at: f64) -> Vec<String> {
        let calendar = calendar();
        let view = now::build(dataset, &calendar, rule(), at);
        lines(&view, dataset, &calendar, Theme::dark(false), Units::Ml, at)
    }

    fn joined(dataset: &Dataset, at: f64) -> String {
        screen(dataset, at).join("\n")
    }

    #[test]
    fn previous_sleep_is_measured_from_its_end_and_shows_its_duration() {
        let mut data = dataset();
        data.sleep = vec![sleep(AFTERNOON - 12600.0, 4800.0)];
        let text = joined(&data, AFTERNOON);
        assert!(text.contains("2h 10m ago · slept for 1h 20m"), "{text}");
    }

    /// A running sleep, and the one before it on a line of its own.
    #[test]
    fn a_live_sleep_keeps_previous_sleep_as_muted_context() {
        let mut data = dataset();
        data.sleep = vec![sleep(AFTERNOON - 12600.0, 4800.0)];
        data.live.sleep_active = true;
        data.live.sleep_start = Some(AFTERNOON - 1800.0);
        let calendar = calendar();
        let view = now::build(&data, &calendar, rule(), AFTERNOON);
        let theme = Theme::dark(true);
        let text = lines(&view, &data, &calendar, theme, Units::Ml, AFTERNOON).join("\n");
        assert!(text.contains("currently sleeping for 30m"), "{text}");
        assert!(
            text.contains(&theme.muted("(previous sleep finished 2h 10m ago · slept for 1h 20m)")),
            "the note says when it ended and how long it ran: {text}"
        );
    }

    /// Two lines, because one long line is one a tired eye reads twice.
    #[test]
    fn the_previous_sleep_sits_on_its_own_line_under_the_running_one() {
        let mut data = dataset();
        data.sleep = vec![sleep(AFTERNOON - 12600.0, 4800.0)];
        data.live.sleep_active = true;
        data.live.sleep_start = Some(AFTERNOON - 1800.0);
        let screen = screen(&data, AFTERNOON);

        let running = screen
            .iter()
            .position(|line| line.contains("currently sleeping for 30m"))
            .expect("a running sleep");
        assert!(
            !screen[running].contains("previous sleep"),
            "the note is not on the same line: {}",
            screen[running]
        );
        let note = &screen[running + 1];
        assert!(
            note.contains("(previous sleep finished 2h 10m ago · slept for 1h 20m)"),
            "it is on the next one: {note}"
        );
    }

    /// The note lines up under the value it is about, not under the label.
    #[test]
    fn the_note_is_indented_to_where_the_value_column_starts() {
        let mut data = dataset();
        data.sleep = vec![sleep(AFTERNOON - 12600.0, 4800.0)];
        data.live.sleep_active = true;
        data.live.sleep_start = Some(AFTERNOON - 1800.0);
        let screen = screen(&data, AFTERNOON);

        let running = screen
            .iter()
            .position(|line| line.contains("currently sleeping"))
            .expect("a running sleep");
        let value_at = screen[running]
            .find("currently sleeping")
            .expect("a value column");
        let note_at = screen[running + 1].find("(previous sleep").expect("a note");
        assert_eq!(note_at, value_at, "{:?}", &screen[running..=running + 1]);
    }

    #[test]
    fn a_running_sleep_with_nothing_before_it_gets_no_note_at_all() {
        let mut data = dataset();
        data.live.sleep_active = true;
        data.live.sleep_start = Some(AFTERNOON - 1800.0);
        let text = joined(&data, AFTERNOON);
        assert!(text.contains("currently sleeping for 30m"), "{text}");
        assert!(!text.contains("previous sleep"), "{text}");
    }

    #[test]
    fn with_nothing_logged_the_screen_says_so_rather_than_showing_zeroes() {
        let text = joined(&dataset(), AFTERNOON);
        assert!(text.contains("Last fed"), "{text}");
        assert!(text.contains("nothing logged"), "{text}");
        assert!(!text.contains("0 ml"), "{text}");
    }

    #[test]
    fn the_last_feed_leads_with_how_long_ago_it_was() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 7_200.0, 90.0)];
        let text = joined(&data, AFTERNOON);
        assert!(text.contains("2h 0m ago"), "{text}");
        assert!(text.contains("90 ml of Formula"), "{text}");
    }

    #[test]
    fn a_volume_follows_the_unit_the_reader_chose() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 3_600.0, 88.72)];
        let calendar = calendar();
        let view = now::build(&data, &calendar, rule(), AFTERNOON);
        let text = lines(
            &view,
            &data,
            &calendar,
            Theme::dark(false),
            Units::Oz,
            AFTERNOON,
        )
        .join("\n");
        assert!(text.contains("3.0 oz"), "{text}");
    }

    #[test]
    fn at_three_in_the_morning_the_stretch_line_reads_tonight() {
        let mut data = dataset();
        data.sleep = vec![sleep(THREE_AM - 14_400.0, 7_200.0)];
        let text = joined(&data, THREE_AM);
        assert!(text.contains("Tonight"), "{text}");
        assert!(text.contains("longest so far 2h 0m"), "{text}");
    }

    #[test]
    fn in_the_afternoon_the_stretch_line_names_the_night_it_means() {
        let mut data = dataset();
        data.sleep = vec![sleep(AFTERNOON - 50_400.0, 10_800.0)];
        let text = joined(&data, AFTERNOON);
        assert!(text.contains("Night of Sun 21 Sep"), "{text}");
        assert!(
            !text.contains("Tonight"),
            "an ambiguous label is the bug: {text}"
        );
    }

    #[test]
    fn a_running_sleep_does_not_make_tonights_stretch_say_no_sleep() {
        let mut data = dataset();
        data.live.sleep_active = true;
        data.live.sleep_start = Some(THREE_AM - 1_800.0);
        let text = joined(&data, THREE_AM);
        assert!(text.contains("currently sleeping for 30m"), "{text}");
        assert!(
            text.contains("nothing finished yet"),
            "a running sleep is not a finished stretch, and the line has to \
             say which it means: {text}"
        );
    }

    #[test]
    fn a_running_sleep_counts_up_and_says_nothing_about_being_awake() {
        let mut data = dataset();
        data.live.sleep_active = true;
        data.live.sleep_start = Some(AFTERNOON - 1_800.0);
        let text = joined(&data, AFTERNOON);
        assert!(text.contains("currently sleeping for 30m"), "{text}");
        assert!(!text.contains("awake"), "{text}");
    }

    #[test]
    fn a_paused_sleep_says_that_the_timer_is_paused() {
        let mut data = dataset();
        data.live.sleep_active = true;
        data.live.sleep_paused = true;
        data.live.sleep_start = Some(AFTERNOON - 1_800.0);
        assert!(joined(&data, AFTERNOON).contains("timer paused"));
    }

    #[test]
    fn a_nursing_session_in_progress_gets_a_line_of_its_own() {
        let mut data = dataset();
        data.live.nursing_active = true;
        data.live.nursing_start = Some(AFTERNOON - 420.0);
        data.live.nursing_side = Some("right".to_owned());
        let text = joined(&data, AFTERNOON);
        assert!(text.contains("Nursing now"), "{text}");
        assert!(text.contains("7m on the right"), "{text}");
    }

    #[test]
    fn nothing_running_means_no_nursing_line_at_all() {
        assert!(!joined(&dataset(), AFTERNOON).contains("Nursing now"));
    }

    #[test]
    fn every_screen_says_how_stale_it_is() {
        let mut data = dataset();
        data.fetched_at = AFTERNOON - 600.0;
        assert!(joined(&data, AFTERNOON).contains("as of 10m ago"));
    }

    #[test]
    fn a_fresh_read_says_just_now_rather_than_a_confusing_zero() {
        let mut data = dataset();
        data.fetched_at = AFTERNOON - 5.0;
        assert_eq!(as_of(&data, AFTERNOON), "as of just now");
    }

    #[test]
    fn a_collection_that_could_not_be_read_is_named_rather_than_left_blank() {
        let mut data = dataset();
        data.notes.push(crate::domain::types::CollectionNote {
            collection: "pump".to_owned(),
            problem: "not readable on this account".to_owned(),
        });
        let text = joined(&data, AFTERNOON);
        assert!(text.contains("pump could not be read"), "{text}");
    }

    #[test]
    fn a_diaper_is_described_by_what_was_in_it() {
        let mut data = dataset();
        data.diapers = vec![diaper(AFTERNOON - 900.0, true, true)];
        assert!(joined(&data, AFTERNOON).contains("wet + dirty"));
    }

    fn with_rule(data: &Dataset, rule: crate::domain::today::DayRule, at: f64) -> String {
        let calendar = calendar();
        let view = now::build(data, &calendar, rule, at);
        lines(&view, data, &calendar, Theme::dark(false), Units::Ml, at).join("\n")
    }

    /// A day's worth of feeds: two before a 6am start, two after.
    fn across_a_day_start() -> Dataset {
        let mut data = dataset();
        data.feeds = vec![
            bottle(AFTERNOON - 36_000.0, 100.0),
            bottle(AFTERNOON - 32_400.0, 100.0),
            bottle(AFTERNOON - 7_200.0, 90.0),
            bottle(AFTERNOON - 1_800.0, 60.0),
        ];
        data
    }

    #[test]
    fn the_screen_says_how_much_went_in_over_the_last_three_hours() {
        let mut data = dataset();
        data.feeds = vec![
            bottle(AFTERNOON - 1_800.0, 60.0),
            // Four hours ago, which is outside the window.
            bottle(AFTERNOON - 14_400.0, 200.0),
        ];
        let text = joined(&data, AFTERNOON);
        let recent = text
            .lines()
            .find(|line| line.starts_with("Last 3h"))
            .unwrap_or_else(|| panic!("no `Last 3h` line in {text}"));
        assert!(recent.contains("60 ml · 1 feed"), "{recent}");
        assert!(
            !recent.contains("260 ml"),
            "the four-hour-old feed is outside the window: {recent}"
        );
        assert!(
            text.contains("260 ml"),
            "and inside today's, which is the point of having both: {text}"
        );
    }

    #[test]
    fn the_screen_says_how_much_went_in_and_how_much_sleep_there_was_today() {
        let mut data = across_a_day_start();
        data.sleep = vec![sleep(AFTERNOON - 10_800.0, 7_200.0)];
        let text = joined(&data, AFTERNOON);
        assert!(text.contains("Fed today"), "{text}");
        assert!(text.contains("Slept today"), "{text}");
        assert!(text.contains("2h 0m"), "{text}");
    }

    #[test]
    fn a_discrete_day_leaves_out_what_happened_before_it_began() {
        let data = across_a_day_start();
        // 2pm with a 6am day start: only the two feeds after 6am count.
        let text = with_rule(
            &data,
            crate::domain::today::DayRule::discrete(6.0, 19.5),
            AFTERNOON,
        );
        assert!(text.contains("150 ml · 2 feeds"), "{text}");
        assert!(
            text.contains("since 6:00 am"),
            "the window says where it began: {text}"
        );
    }

    #[test]
    fn a_continuous_day_counts_the_whole_rolling_twenty_four_hours() {
        let data = across_a_day_start();
        let text = with_rule(
            &data,
            crate::domain::today::DayRule::continuous(6.0, 19.5),
            AFTERNOON,
        );
        assert!(text.contains("Fed in 24h"), "{text}");
        assert!(text.contains("Slept in 24h"), "{text}");
        assert!(text.contains("350 ml · 4 feeds"), "{text}");
        assert!(
            !text.contains("since"),
            "a rolling window has no hour to have begun at: {text}"
        );
    }

    #[test]
    fn a_four_in_the_morning_feed_belongs_to_the_day_that_has_not_ended() {
        let mut data = dataset();
        // THREE_AM is 3am in New York; a feed an hour earlier is 2am.
        data.feeds = vec![bottle(THREE_AM - 3_600.0, 80.0)];
        let text = with_rule(
            &data,
            crate::domain::today::DayRule::discrete(6.0, 19.5),
            THREE_AM,
        );
        assert!(
            text.contains("Fed today") && text.contains("80 ml"),
            "a 2am feed is still part of the day that began at 6am yesterday: {text}"
        );
    }

    #[test]
    fn nursing_and_meals_are_counted_in_their_own_words() {
        let mut data = dataset();
        data.feeds = vec![
            crate::domain::fixtures::nursing(AFTERNOON - 1_800.0, 600.0, 300.0),
            crate::domain::types::FeedEvent::Solids {
                at: None,
                id: "m".into(),
                start: AFTERNOON - 3_000.0,
                foods: vec!["Avocado".into()],
                reaction: None,
                notes: None,
            },
        ];
        let text = joined(&data, AFTERNOON);
        assert!(text.contains("nursed 15m"), "{text}");
        assert!(text.contains("1 feed"), "{text}");
        assert!(text.contains("1 meal"), "{text}");
    }

    #[test]
    fn a_window_with_nothing_in_it_says_so_rather_than_printing_a_zero() {
        let text = joined(&dataset(), AFTERNOON);
        for label in ["Last 3h", "Fed today", "Slept today"] {
            let line = text
                .lines()
                .find(|line| line.starts_with(label))
                .unwrap_or_else(|| panic!("no `{label}` line in {text}"));
            assert!(line.contains("nothing logged"), "{line}");
        }
        assert!(!text.contains("0 ml"), "{text}");
        assert!(!text.contains("0m"), "{text}");
    }

    #[test]
    fn a_sleep_in_progress_counts_towards_what_was_slept_today() {
        let mut data = dataset();
        data.live.sleep_active = true;
        data.live.sleep_start = Some(AFTERNOON - 3_600.0);
        let text = joined(&data, AFTERNOON);
        let line = text
            .lines()
            .find(|line| line.starts_with("Slept today"))
            .unwrap_or_default();
        assert!(line.contains("1h 0m"), "{text}");
    }

    #[test]
    fn piped_output_carries_no_escape_sequences() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 3_600.0, 90.0)];
        assert!(!joined(&data, AFTERNOON).contains('\u{1b}'));
    }
}
