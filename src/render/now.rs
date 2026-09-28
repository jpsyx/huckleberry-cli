//! The 3am screen, as lines of text.
//!
//! Four facts, large and unadorned, in the order a parent needs them. The
//! screen answers "when did he last eat" before anything else because that is
//! the question that gets asked at 3am, and it carries an honest "as of" line
//! because a stale "last fed two hours ago" is what sends somebody to wake a
//! sleeping baby.

use crate::cli::Units;
use crate::domain::now::NowView;
use crate::domain::time::{Calendar, format_ago, format_duration};
use crate::domain::types::{Dataset, FeedEvent};
use crate::theme::Theme;

use super::format;

/// The screen, one line per element, ready for stdout.
#[must_use]
pub fn lines(
    view: &NowView,
    dataset: &Dataset,
    calendar: &Calendar,
    theme: Theme,
    units: Units,
    now: f64,
) -> Vec<String> {
    let mut lines = vec![
        theme.heading(&dataset.child.name),
        String::new(),
        fact(theme, "Last fed", &feed_line(view, calendar, units, now)),
        fact(theme, "Diaper", &diaper_line(view, calendar, now)),
        sleep_line(view, dataset, theme, now),
        fact(theme, &stretch_label(view), &stretch_line(view, calendar)),
    ];

    if let Some(nursing) = &view.nursing_now {
        let state = if nursing.paused { " (paused)" } else { "" };
        lines.push(fact(
            theme,
            "Nursing now",
            &format!(
                "{} on the {}{state}",
                format_duration(nursing.elapsed_seconds),
                nursing.side
            ),
        ));
    }

    lines.push(String::new());
    lines.push(theme.muted(&as_of(dataset, now)));
    for note in &dataset.notes {
        lines.push(theme.muted(&format!(
            "  {} could not be read: {}",
            note.collection, note.problem
        )));
    }
    lines
}

/// One labelled fact.
fn fact(theme: Theme, label: &str, value: &str) -> String {
    format!(
        "{}  {}",
        theme.accent(&format::pad(label, 13)),
        theme.value(value)
    )
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
fn sleep_line(view: &NowView, dataset: &Dataset, theme: Theme, now: f64) -> String {
    let state = &view.sleep_state;
    if state.asleep {
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
        let mut line = fact(theme, "Sleep", &current);
        if let Some(end) = state.last_sleep_end {
            line.push(' ');
            line.push_str(&theme.muted(&format!("(previous sleep was {})", format_ago(end, now))));
        }
        return line;
    }
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
    fact(theme, "Sleep", &previous)
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

    fn screen(dataset: &Dataset, at: f64) -> Vec<String> {
        let calendar = calendar();
        let view = now::build(dataset, &calendar, at);
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

    #[test]
    fn a_live_sleep_keeps_previous_sleep_as_muted_context() {
        let mut data = dataset();
        data.sleep = vec![sleep(AFTERNOON - 12600.0, 4800.0)];
        data.live.sleep_active = true;
        data.live.sleep_start = Some(AFTERNOON - 1800.0);
        let calendar = calendar();
        let view = now::build(&data, &calendar, AFTERNOON);
        let theme = Theme::dark(true);
        let text = lines(&view, &data, &calendar, theme, Units::Ml, AFTERNOON).join("\n");
        assert!(text.contains("currently sleeping for 30m"), "{text}");
        assert!(
            text.contains(&theme.muted("(previous sleep was 2h 10m ago)")),
            "{text}"
        );
        assert!(
            joined(&data, AFTERNOON)
                .contains("currently sleeping for 30m (previous sleep was 2h 10m ago)")
        );
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
        let view = now::build(&data, &calendar, AFTERNOON);
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

    #[test]
    fn piped_output_carries_no_escape_sequences() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 3_600.0, 90.0)];
        assert!(!joined(&data, AFTERNOON).contains('\u{1b}'));
    }
}
