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
use crate::domain::reference::{self, Metric, Standing};
use crate::domain::time::{Calendar, format_ago, format_duration, split_hour};
use crate::domain::today::{DayMode, Totals};
use crate::domain::types::Child;
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
        &format!("Fed in last {}h", RECENT_HOURS as i64),
        &intake(&view.recent, units),
    ));
    let (fed, slept_label) = total_labels(view);
    rows.push(fact(fed, &today_intake_line(view, units)));
    rows.push(fact(slept_label, &slept(&view.today)));

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
///
/// `width` is how many columns there are to draw in, when that is known. With
/// room, the typical ranges for this baby's age go in a second column beside
/// the facts; without it they follow underneath, saying the same things.
#[must_use]
pub fn lines(
    view: &NowView,
    dataset: &Dataset,
    calendar: &Calendar,
    theme: Theme,
    units: Units,
    now: f64,
    width: Option<usize>,
) -> Vec<String> {
    laid_out(
        view,
        dataset,
        calendar,
        units,
        now,
        width,
        WhenNarrow::Stack,
    )
    .into_iter()
    .map(|row| {
        row.into_iter()
            .map(|piece| theme.paint(piece.tone, &piece.text))
            .collect()
    })
    .collect()
}

/// How wide the ranges column has to be before it is worth having one.
///
/// A range sentence that wraps every other word is harder to read than one
/// under the facts, so below this they go underneath.
const RANGES_FLOOR: usize = 46;

/// The gap between the two columns.
const GUTTER: usize = 3;

/// What to do with the ranges when there is no room beside the facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhenNarrow {
    /// Put them underneath. What a command does: its output can be as long as
    /// it likes, because nothing else is sharing the screen with it.
    Stack,
    /// Leave them out. What a panel does, because it cannot grow without
    /// taking the room from whatever is beside it.
    Drop,
}

/// The facts, and the ranges beside or under them.
#[must_use]
pub fn laid_out(
    view: &NowView,
    dataset: &Dataset,
    calendar: &Calendar,
    units: Units,
    now: f64,
    width: Option<usize>,
    narrow: WhenNarrow,
) -> Vec<Row> {
    let facts = screen(view, dataset, calendar, units, now);
    let ranges = ranges(view, &dataset.child, calendar, now);
    if ranges.is_empty() {
        return facts;
    }
    let left = facts.iter().map(plain_width).max().unwrap_or_default();
    let room = width.unwrap_or(0).saturating_sub(left + GUTTER);
    if room < RANGES_FLOOR {
        return match narrow {
            WhenNarrow::Stack => stacked(facts, ranges),
            WhenNarrow::Drop => facts,
        };
    }
    beside(&facts, &ranges, left + GUTTER, room)
}

/// The two columns, zipped, each row padded out to the gutter.
fn beside(facts: &[Row], ranges: &[Row], at: usize, room: usize) -> Vec<Row> {
    let wrapped: Vec<Row> = ranges.iter().flat_map(|row| wrap(row, room)).collect();
    let height = facts.len().max(wrapped.len());
    (0..height)
        .map(|index| {
            let mut row = facts.get(index).cloned().unwrap_or_default();
            let Some(right) = wrapped.get(index) else {
                return row;
            };
            let pad = at.saturating_sub(plain_width(&row)).max(1);
            row.push(Piece::new(" ".repeat(pad), Tone::Value));
            row.extend(right.iter().cloned());
            row
        })
        .collect()
}

/// The ranges under the facts, for a screen with no room beside them.
fn stacked(mut facts: Vec<Row>, ranges: Vec<Row>) -> Vec<Row> {
    facts.push(Row::new());
    facts.extend(ranges);
    facts
}

/// One row broken to a width, keeping every piece's tone.
///
/// A range sentence is one piece, so this only ever splits on spaces.
fn wrap(row: &Row, width: usize) -> Vec<Row> {
    if plain_width(row) <= width || row.len() != 1 {
        return vec![row.clone()];
    }
    let piece = &row[0];
    let mut rows = Vec::new();
    let mut line = String::new();
    for word in piece.text.split(' ') {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if candidate.chars().count() > width && !line.is_empty() {
            rows.push(vec![Piece::new(std::mem::take(&mut line), piece.tone)]);
            // Continued lines are indented, so a wrapped sentence still reads
            // as one thing rather than as two.
            line = format!("  {word}");
        } else {
            line = candidate;
        }
    }
    rows.push(vec![Piece::new(line, piece.tone)]);
    rows
}

/// How wide a row is once its colour is taken off.
fn plain_width(row: &Row) -> usize {
    row.iter().map(|piece| piece.text.chars().count()).sum()
}

/// The typical ranges for this baby's age, each judged against today.
///
/// The same sentences the summary shows, against today rather than the week.
/// Yellow, never red: a figure outside a typical range is worth a second look
/// and is not an emergency, and this tool is in no position to say which.
fn ranges(view: &NowView, child: &Child, calendar: &Calendar, now: f64) -> Vec<Row> {
    let Some(age) = child
        .birthdate
        .as_deref()
        .and_then(|birthdate| calendar.age_in_days(birthdate, now))
    else {
        return Vec::new();
    };
    let today = &view.today;
    let figures: [Figure; 4] = [
        (Metric::FeedsPerDay, today.milk_feeds as f64, Saying::Count),
        (Metric::SleepPerDay, today.sleep_seconds, Saying::Duration),
        (Metric::WetPerDay, today.wet as f64, Saying::Count),
        (Metric::DirtyPerDay, today.dirty as f64, Saying::Count),
    ];
    let mut rows: Vec<Row> = Vec::new();
    for (metric, value, say) in figures {
        let Some(band) = reference::band_for(metric, Some(age)) else {
            continue;
        };
        let standing = band.standing(value);
        rows.push(vec![Piece::new(
            format!("{} {}", band.label, aside(standing, value, say)),
            tone_for(standing),
        )]);
    }
    if rows.is_empty() {
        return rows;
    }
    // The left column already says the name, so this says the thing it does
    // not: how old the baby is, which is what makes these ranges the right
    // ones.
    let mut lines = vec![vec![Piece::new(format!("{age} days old"), Tone::Muted)]];
    lines.extend(rows);
    // Last, because a caveat that arrives before the thing it qualifies is a
    // caveat nobody reads.
    lines.push(Row::new());
    lines.push(vec![Piece::new(
        reference::PEDIATRICIAN_NOTE.to_owned(),
        Tone::Muted,
    )]);
    lines
}

/// One figure judged against a range: what it is, where it stands, how to
/// say it.
type Figure = (Metric, f64, Saying);

/// How a figure reads back when today has not reached its range yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Saying {
    /// A plain number of things.
    Count,
    /// A stretch of time.
    Duration,
}

impl Saying {
    fn say(self, value: f64) -> String {
        match self {
            Self::Count => format!("{value:.0}"),
            Self::Duration => format_duration(value),
        }
    }
}

/// What to say beside a range, given where today has got to.
///
/// **A day still going is never called short.** Every figure is under every
/// range at eight in the morning, because the day is an hour old, and saying
/// so would be a false alarm every morning on a screen a frightened parent
/// opens at 3am. Below the range it says where the day has reached and leaves
/// the reading to the reader; above it, and inside it, are verdicts a partial
/// day can support, because they have already happened.
///
/// Said in words as well as in colour, so the meaning survives a pipe, a
/// screenshot, and colour blindness.
fn aside(standing: Standing, value: f64, say: Saying) -> String {
    match standing {
        // Zero reads as nothing, the way it does in the column beside it: a
        // screen that says `0s` about a baby is counting where it should be
        // admitting it has nothing to count.
        Standing::Below if value <= 0.0 => "(nothing yet today)".to_owned(),
        Standing::Below => format!("({} so far today)", say.say(value)),
        Standing::Above => "(today is over that)".to_owned(),
        Standing::Inside => "(today is in that range)".to_owned(),
    }
}

/// How a figure is painted, given where it sits.
///
/// A day still going is not judged, so it is not coloured as though it were.
const fn tone_for(standing: Standing) -> Tone {
    match standing {
        Standing::Inside => Tone::Good,
        Standing::Above => Tone::Attention,
        Standing::Below => Tone::Muted,
    }
}

/// How wide the label column is, so a note under a value lines up with it
/// rather than with the label it belongs to.
const LABEL: usize = 17;

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

/// What the running totals are called: `(fed, slept)`.
///
/// A rolling window reads as the span it covers, because "today" for a family
/// counting a rolling day would be a word doing the opposite of its job at
/// 3am. A discrete day reads as the day's total, because that is the question
/// it answers.
#[must_use]
pub const fn total_labels(view: &NowView) -> (&'static str, &'static str) {
    match view.today_window.mode {
        DayMode::Continuous => ("Fed in last 24h", "Slept in last 24h"),
        DayMode::Discrete => ("Total fed today", "Total slept today"),
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
        lines(
            &view,
            dataset,
            &calendar,
            Theme::dark(false),
            Units::Ml,
            at,
            None,
        )
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
        let text = lines(&view, &data, &calendar, theme, Units::Ml, AFTERNOON, None).join("\n");
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
            None,
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
        lines(
            &view,
            data,
            &calendar,
            Theme::dark(false),
            Units::Ml,
            at,
            None,
        )
        .join("\n")
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
    fn the_screen_says_how_much_went_in_over_the_last_four_hours() {
        let mut data = dataset();
        data.feeds = vec![
            bottle(AFTERNOON - 1_800.0, 60.0),
            // Three and a half hours ago, which is inside the window.
            bottle(AFTERNOON - 12_600.0, 40.0),
            // Five hours ago, which is outside the window.
            bottle(AFTERNOON - 18_000.0, 200.0),
        ];
        let text = joined(&data, AFTERNOON);
        let recent = text
            .lines()
            .find(|line| line.starts_with("Fed in last 4h"))
            .unwrap_or_else(|| panic!("no `Fed in last 4h` line in {text}"));
        assert!(recent.contains("100 ml · 2 feeds"), "{recent}");
        assert!(
            !recent.contains("300 ml"),
            "the five-hour-old feed is outside the window: {recent}"
        );
        assert!(
            text.contains("300 ml"),
            "and inside today's, which is the point of having both: {text}"
        );
    }

    #[test]
    fn the_screen_says_how_much_went_in_and_how_much_sleep_there_was_today() {
        let mut data = across_a_day_start();
        data.sleep = vec![sleep(AFTERNOON - 10_800.0, 7_200.0)];
        let text = joined(&data, AFTERNOON);
        assert!(text.contains("Total fed today"), "{text}");
        assert!(text.contains("Total slept today"), "{text}");
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
        assert!(text.contains("Fed in last 24h"), "{text}");
        assert!(text.contains("Slept in last 24h"), "{text}");
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
            text.contains("Total fed today") && text.contains("80 ml"),
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
    fn every_running_total_lines_its_value_up_with_the_facts_above_it() {
        let column = |text: &str, label: &str| {
            let line = text
                .lines()
                .find(|line| line.starts_with(label))
                .unwrap_or_else(|| panic!("no `{label}` line in {text}"));
            line.len() - line[label.len()..].trim_start().len()
        };
        for rule in [
            crate::domain::today::DayRule::discrete(6.0, 19.5),
            crate::domain::today::DayRule::continuous(6.0, 19.5),
        ] {
            let text = with_rule(&dataset(), rule, AFTERNOON);
            let expected = column(&text, "Last fed");
            for label in [
                "Fed in last 4h",
                "Total fed today",
                "Fed in last 24h",
                "Total slept today",
                "Slept in last 24h",
            ] {
                if text.contains(label) {
                    assert_eq!(column(&text, label), expected, "{text}");
                }
            }
        }
    }

    #[test]
    fn a_window_with_nothing_in_it_says_so_rather_than_printing_a_zero() {
        let text = joined(&dataset(), AFTERNOON);
        for label in ["Fed in last 4h", "Total fed today", "Total slept today"] {
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
            .find(|line| line.starts_with("Total slept today"))
            .unwrap_or_default();
        assert!(line.contains("1h 0m"), "{text}");
    }

    /// A day's worth of a newborn: enough feeds, enough diapers, little sleep.
    fn a_newborns_day() -> Dataset {
        let mut data = dataset();
        data.child.birthdate = Some("2025-09-08".to_owned());
        data.feeds = (0..9)
            .map(|index| bottle(AFTERNOON - 3_600.0 * f64::from(index), 70.0))
            .collect();
        data.diapers = (0..7)
            .map(|index| diaper(AFTERNOON - 3_600.0 * f64::from(index), true, index < 3))
            .collect();
        data.sleep = vec![sleep(AFTERNOON - 40_000.0, 4.0 * 3600.0)];
        data
    }

    fn wide(data: &Dataset, at: f64) -> String {
        let calendar = calendar();
        let view = now::build(data, &calendar, rule(), at);
        lines(
            &view,
            data,
            &calendar,
            Theme::dark(false),
            Units::Ml,
            at,
            Some(142),
        )
        .join("\n")
    }

    #[test]
    fn a_wide_screen_carries_the_typical_ranges_beside_the_facts() {
        let text = wide(&a_newborns_day(), AFTERNOON);
        assert!(
            text.contains("typical in the first weeks: 8 or more feeds a day"),
            "{text}"
        );
        assert!(
            text.contains("typical from day 3: 5 or more wet diapers a day"),
            "{text}"
        );
        assert!(
            text.contains("typical at this age: 8 to 20 hours in 24"),
            "{text}"
        );
    }

    /// The summary says "this week". This screen is about today.
    #[test]
    fn the_ranges_are_judged_against_today_and_say_so() {
        let text = wide(&a_newborns_day(), AFTERNOON);
        assert!(text.contains("(today is in that range)"), "{text}");
        assert!(!text.contains("this week"), "{text}");
    }

    /// A day that is still going cannot be short of anything yet.
    ///
    /// At eight in the morning every figure is under every range, because the
    /// day is an hour old. Saying so would be a false alarm every morning, and
    /// this is a tool a frightened parent opens at 3am.
    #[test]
    fn a_day_still_going_is_never_called_short_only_reported() {
        let text = wide(&a_newborns_day(), AFTERNOON);
        assert!(
            !text.contains("under that"),
            "a partial day is never called short: {text}"
        );
        assert!(
            text.contains("so far today"),
            "it says where the day has got to instead: {text}"
        );
        // The sleep began before the day did, so only its tail counts today.
        assert!(text.contains("(53m so far today)"), "{text}");
    }

    /// Over is a verdict a partial day can support: it has already happened.
    /// Zero reads as nothing, the way it does in the column beside it.
    #[test]
    fn nothing_yet_today_says_so_rather_than_counting_to_zero() {
        let mut data = a_newborns_day();
        data.sleep.clear();
        let text = wide(&data, AFTERNOON);
        assert!(text.contains("(nothing yet today)"), "{text}");
        assert!(!text.contains("0s so far"), "{text}");
        assert!(!text.contains("(0 so far"), "{text}");
    }

    /// Sleep is the only metric with a ceiling, because it is the only one a
    /// source puts a top on: no body calls a high number of feeds or diapers
    /// atypical, so those bands are floors and nothing can be over them.
    #[test]
    fn a_figure_already_past_the_top_of_its_range_says_so() {
        let mut data = a_newborns_day();
        // Twelve sleeps of just under two hours, one every two hours, so they
        // never overlap and together run past the top of the newborn band.
        data.sleep = (0..12)
            .map(|index| sleep(AFTERNOON - 7_200.0 * f64::from(index) - 7_200.0, 7_000.0))
            .collect();
        let calendar = calendar();
        // A rolling twenty-four hours, which is the window the band is stated
        // over. A day that started this morning cannot hold nineteen hours.
        let rolling = crate::domain::today::DayRule::continuous(6.0, 19.5);
        let view = now::build(&data, &calendar, rolling, AFTERNOON);
        let text = lines(
            &view,
            &data,
            &calendar,
            Theme::dark(false),
            Units::Ml,
            AFTERNOON,
            Some(140),
        )
        .join("\n");
        assert!(text.contains("(today is over that)"), "{text}");
    }

    /// The pediatrician outranks this table, and the screen says so where the
    /// ranges are rather than only in the manual.
    #[test]
    fn the_ranges_carry_the_note_that_a_pediatrician_outranks_them() {
        let text = wide(&a_newborns_day(), AFTERNOON);
        assert!(
            text.contains(
                "typical ranges are not your baby: where your pediatrician disagrees, they are right"
            ),
            "{text}"
        );
    }

    /// The note qualifies the ranges, so with no ranges there is nothing for
    /// it to qualify and it would only be noise.
    #[test]
    fn with_no_ranges_there_is_no_pediatrician_note_either() {
        let mut data = a_newborns_day();
        data.child.birthdate = None;
        let text = wide(&data, AFTERNOON);
        assert!(!text.contains("pediatrician"), "{text}");
    }

    /// It is the last thing in the ranges column: a caveat that came before
    /// the thing it qualifies is a caveat nobody reads.
    #[test]
    fn the_pediatrician_note_comes_after_every_range() {
        let text = joined(&a_newborns_day(), AFTERNOON);
        let lines: Vec<&str> = text.lines().collect();
        let last_range = lines
            .iter()
            .rposition(|line| line.contains("typical at this age"))
            .expect("a range");
        let note = lines
            .iter()
            .position(|line| line.contains("pediatrician"))
            .expect("the note");
        assert!(note > last_range, "{text}");
    }

    #[test]
    fn the_ranges_sit_beside_the_facts_rather_than_under_them() {
        let text = wide(&a_newborns_day(), AFTERNOON);
        let beside = text
            .lines()
            .find(|line| line.contains("Last fed") && line.contains("typical"))
            .unwrap_or_else(|| panic!("no line holding both: {text}"));
        assert!(
            beside.find("Last fed") < beside.find("typical"),
            "the facts are the left column: {beside}"
        );
    }

    #[test]
    fn a_narrow_screen_keeps_one_column_and_says_the_same_things() {
        let text = joined(&a_newborns_day(), AFTERNOON);
        assert!(text.contains("Last fed"), "{text}");
        assert!(
            text.contains("typical in the first weeks: 8 or more feeds a day"),
            "the ranges are still there, under the facts: {text}"
        );
        for line in text.lines() {
            assert!(
                !(line.contains("Last fed") && line.contains("typical")),
                "nothing shares a row when there is no room: {line}"
            );
        }
    }

    #[test]
    fn a_baby_too_old_for_any_range_gets_facts_and_nothing_else() {
        let mut data = a_newborns_day();
        data.child.birthdate = Some("2019-01-01".to_owned());
        let text = wide(&data, AFTERNOON);
        assert!(text.contains("Last fed"), "{text}");
        assert!(!text.contains("typical"), "{text}");
    }

    #[test]
    fn piped_output_carries_no_escape_sequences() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 3_600.0, 90.0)];
        assert!(!joined(&data, AFTERNOON).contains('\u{1b}'));
    }
}
