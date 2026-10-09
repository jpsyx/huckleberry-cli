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

mod layout;

#[cfg(test)]
mod layout_tests;

/// One row of the screen: its text, and the role each piece is painted in.
pub type Row = Vec<Piece>;

/// The screen, row by row, with nothing painted yet.
///
/// The single source of what `now` says. Anything that shows these facts draws
/// from here, so no second copy of the wording can drift from this one.
/// `width` chooses three, two, or one column and bounds the wrapped content.
#[must_use]
pub fn screen(
    view: &NowView,
    dataset: &Dataset,
    calendar: &Calendar,
    units: Units,
    now: f64,
    width: Option<usize>,
) -> Vec<Row> {
    let ranges = ranges(view, &dataset.child, calendar, now);
    let columns = layout::Columns::for_width(width, !ranges.is_empty());
    let tables = totals_rows(view, units, now, columns.table_width());
    columns.arrange(facts(view, dataset, calendar, units, now), tables, ranges)
}

/// Last events and freshness, kept together independently of the totals.
fn facts(
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
        fact("Last diaper", &diaper_line(view, calendar, now)),
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
/// room, facts, tables, and typical ranges occupy three columns. Medium widths
/// put facts and ranges together on the left; narrow widths stack all three.
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
    screen(view, dataset, calendar, units, now, width)
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|piece| theme.paint(piece.tone, &piece.text))
                .collect()
        })
        .collect()
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
    // The age heads the column and the ranges under it are a separate
    // thought, so they are not run together.
    let mut lines = vec![
        vec![Piece::new(format!("{age} days old"), Tone::Muted)],
        Row::new(),
    ];
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

/// The running totals, and the times behind the recent ones.
///
/// These come after the four facts because they answer the second question
/// rather than the first: not "when did she last eat" but "has she had
/// enough". Each recent total is followed by when those feeds or sleeps
/// actually were, because "3 feeds" does not say whether they were spread out
/// or all at once.
fn totals_rows(view: &NowView, units: Units, now: f64, width: Option<usize>) -> Vec<Row> {
    let hours = RECENT_HOURS as i64;
    let recent = [as_total(intake(&view.recent, units)), recent_sleep(view)];
    let times = [
        event_times(&view.recent_feed_starts, now),
        event_times(&view.recent_sleep_ends, now),
    ];
    let daily = [
        as_total(today_intake_line(view, units)),
        as_total(slept(&view.today)),
    ];
    let mut rows = Vec::new();
    rows.extend(window_table(
        &format!("In last {hours}h"),
        &recent,
        Some(&times),
        width,
    ));
    rows.push(Row::new());
    rows.extend(window_table(day_heading(view), &daily, None, width));
    rows
}

/// One header row and one data row, whose optional second line has no divider.
fn window_table(
    heading: &str,
    totals: &[String; 2],
    times: Option<&[String; 2]>,
    width: Option<usize>,
) -> Vec<Row> {
    let mut widths = std::array::from_fn(|column| {
        totals[column]
            .chars()
            .count()
            .max(times.map_or(0, |values| values[column].chars().count()))
            .max(5)
    });
    if let Some(width) = width {
        let budget = width.saturating_sub(7).max(10);
        while widths.iter().sum::<usize>() > budget {
            let column = usize::from(widths[1] > widths[0]);
            widths[column] -= 1;
        }
    }
    let border = |left, middle, right| {
        vec![Piece::new(
            format!(
                "{left}{}{middle}{}{right}",
                "─".repeat(widths[0] + 2),
                "─".repeat(widths[1] + 2)
            ),
            Tone::Muted,
        )]
    };
    let mut rows = vec![
        vec![Piece::new(heading.to_owned(), Tone::Heading)],
        border('┌', '┬', '┐'),
        table_line(["Feed", "Sleep"], widths, Tone::Accent),
        border('├', '┼', '┤'),
    ];
    rows.extend(table_cell_lines(totals, widths, Tone::Value));
    if let Some(times) = times {
        rows.extend(table_cell_lines(times, widths, Tone::Muted));
    }
    rows.push(border('└', '┴', '┘'));
    rows
}

/// Wrap each cell within the same data row, keeping timestamps together.
fn table_cell_lines(cells: &[String; 2], widths: [usize; 2], tone: Tone) -> Vec<Row> {
    let wrapped: [Vec<String>; 2] = std::array::from_fn(|column| {
        let mut lines = vec![String::new()];
        for part in cells[column].split(" · ") {
            let line = lines.last_mut().expect("a cell always has a line");
            if !line.is_empty() && line.chars().count() + 3 + part.chars().count() > widths[column]
            {
                line.push_str(" ·");
                lines.push(part.to_owned());
            } else {
                if !line.is_empty() {
                    line.push_str(" · ");
                }
                line.push_str(part);
            }
        }
        lines
            .into_iter()
            .flat_map(|line| wrap_cell_words(&line, widths[column]))
            .collect()
    });
    (0..wrapped[0].len().max(wrapped[1].len()))
        .map(|index| {
            table_line(
                std::array::from_fn(|column| wrapped[column].get(index).map_or("", String::as_str)),
                widths,
                tone,
            )
        })
        .collect()
}

/// Wrap long quantities at spaces without inserting indentation inside a cell.
fn wrap_cell_words(text: &str, width: usize) -> Vec<String> {
    layout::wrap_words(text, width)
}

/// Keep borders quiet while each cell retains its semantic tone.
fn table_line(cells: [&str; 2], widths: [usize; 2], tone: Tone) -> Row {
    let mut row = vec![Piece::new("│ ".to_owned(), Tone::Muted)];
    for (column, cell) in cells.into_iter().enumerate() {
        row.push(Piece::new(cell.to_owned(), tone));
        row.push(Piece::new(
            format!(
                "{} │{}",
                " ".repeat(widths[column].saturating_sub(cell.chars().count())),
                if column == 0 { " " } else { "" }
            ),
            Tone::Muted,
        ));
    }
    row
}

/// How much sleep the recent window holds, and how many completed sleeps.
fn recent_sleep(view: &NowView) -> String {
    let ends = &view.recent_sleep_ends;
    if ends.is_empty() {
        return slept(&view.recent);
    }
    as_total(format!(
        "{} · {}",
        slept(&view.recent),
        plural(ends.len(), "sleep", "sleeps")
    ))
}

/// What an empty window says, in one place because [`as_total`] has to
/// recognise it: "nothing logged total" is not a sentence, and in a discrete
/// day the phrase carries a `· since` after it that a naive split would put
/// the word in the middle of.
///
/// Not "0 ml" or "0m": a zero is a claim about the baby, and this is a claim
/// about the record.
const NOTHING: &str = "nothing logged";

/// Marks the quantity as the whole window's, not one feed's or one sleep's.
///
/// "350 ml · 5 feeds" reads as 350 ml each about as easily as it reads as 350
/// altogether, and one of those is four hundred per cent of the truth. The
/// word goes on the quantity rather than on the count, because the count was
/// never the ambiguous half.
///
/// Every window takes it, recent and day alike. A window with nothing in it
/// does not, because it has no total to qualify.
fn as_total(value: String) -> String {
    if value.starts_with(NOTHING) {
        return value;
    }
    match value.split_once(" · ") {
        Some((quantity, rest)) => format!("{quantity} total · {rest}"),
        None => format!("{value} total"),
    }
}

/// Recent event times, newest first, using the same separator for every count.
fn event_times(instants: &[f64], now: f64) -> String {
    instants
        .iter()
        .map(|at| format_ago(*at, now))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// How wide the label column is, so a note under a value lines up with it
/// rather than with the label it belongs to.
const LABEL: usize = 18;

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
        return NOTHING.to_owned();
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
        return NOTHING.to_owned();
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
        return vec![fact("Last sleep", &previous)];
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
    let mut rows = vec![fact("Last sleep", &current)];
    if let Some(sleep) = dataset.last_sleep() {
        rows.push(note(&format!(
            "(previous sleep finished {} · slept for {})",
            format_ago(sleep.end(), now),
            format_duration(sleep.duration)
        )));
    }
    rows
}

/// The shared heading names the family's aggregation window.
const fn day_heading(view: &NowView) -> &'static str {
    match view.today_window.mode {
        DayMode::Continuous => "In last 24h",
        DayMode::Discrete => "Today",
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
        return NOTHING.to_owned();
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
    NOTHING.to_owned()
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
/// it is last night, which is what anybody actually calls it.
#[must_use]
pub fn stretch_label(view: &NowView) -> String {
    if view.longest_stretch.tonight {
        "Tonight".to_owned()
    } else {
        "Last night's sleep".to_owned()
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
            NOTHING.to_owned()
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

    /// The window owns both metrics, and event times stay inside its data row.
    #[test]
    fn recent_and_daily_totals_share_feed_and_sleep_tables() {
        let mut data = dataset();
        data.feeds = vec![
            bottle(AFTERNOON - 2_940.0, 30.0),
            bottle(AFTERNOON - 8_640.0, 60.0),
        ];
        data.sleep = vec![
            sleep(AFTERNOON - 4_980.0, 1_800.0),
            sleep(AFTERNOON - 10_080.0, 1_800.0),
        ];
        let text = joined(&data, AFTERNOON);
        let recent = table_cells(&text, "In last 4h");
        assert_eq!(
            recent,
            vec![
                vec!["Feed", "Sleep"],
                vec!["90 ml total · 2 feeds", "1h 0m total · 2 sleeps"],
                vec!["49m ago · 2h 24m ago", "53m ago · 2h 18m ago"],
            ]
        );
        let daily = table_cells(&text, "Today");
        assert_eq!(
            daily,
            vec![
                vec!["Feed", "Sleep"],
                vec!["90 ml total · 2 feeds · since 6:00 am", "1h 0m total"]
            ]
        );
        let recent_block = text
            .split("In last 4h")
            .nth(1)
            .unwrap()
            .split("Today")
            .next()
            .unwrap();
        assert_eq!(
            recent_block.matches('├').count(),
            1,
            "only the header has a separator: {text}"
        );
        assert!(text.contains("Last diaper"), "{text}");
        assert!(text.contains("Last sleep"), "{text}");
    }

    /// Read visible cells without depending on column padding.
    fn table_cells<'a>(text: &'a str, heading: &str) -> Vec<Vec<&'a str>> {
        text.lines()
            .skip_while(|line| *line != heading)
            .skip(1)
            .take_while(|line| !line.starts_with('└'))
            .filter(|line| line.starts_with('│'))
            .map(|line| line.split('│').skip(1).take(2).map(str::trim).collect())
            .collect()
    }

    /// The night line named the day it was talking about, which is precise and
    /// is not what anybody calls it at 3am.
    /// The age heads the column; the ranges under it are a separate thought.
    /// "350 ml · 5 feeds" can be read as 350 ml each, which is four hundred
    /// per cent of the truth and exactly the kind of number that gets acted on.
    #[test]
    fn the_recent_totals_say_total_so_the_figure_is_not_read_as_one_feed() {
        let text = joined(&a_newborns_day(), AFTERNOON);
        assert!(text.contains("350 ml total · 5 feeds"), "{text}");
    }

    #[test]
    fn the_recent_sleep_total_says_total_too() {
        let mut data = dataset();
        data.sleep = vec![
            sleep(AFTERNOON - 4_320.0, 1_800.0),
            sleep(AFTERNOON - 10_020.0, 1_800.0),
        ];
        let text = joined(&data, AFTERNOON);
        assert!(text.contains("1h 0m total · 2 sleeps"), "{text}");
    }

    /// A lone event still belongs on the second line within its cell.
    #[test]
    fn single_recent_events_use_the_second_line_of_the_data_row() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 2_520.0, 30.0)];
        data.sleep = vec![sleep(AFTERNOON - 4_320.0, 1_800.0)];
        let text = joined(&data, AFTERNOON);
        let cells = table_cells(&text, "In last 4h");
        assert_eq!(cells[1], ["30 ml total · 1 feed", "30m total · 1 sleep"]);
        assert_eq!(cells[2], ["42m ago", "42m ago"]);
    }

    /// An empty metric has no total to qualify, even beside a populated one.
    #[test]
    fn the_word_total_goes_nowhere_it_is_not_needed() {
        let text = joined(&a_newborns_day(), AFTERNOON);
        for cell in text.lines().flat_map(|line| line.split('│')) {
            if cell.contains("nothing logged") {
                assert!(!cell.contains(" total"), "{cell}");
            }
        }
    }

    /// The day figures are as easily misread as the recent ones.
    #[test]
    fn the_day_totals_say_total_too() {
        let mut data = across_a_day_start();
        data.sleep = vec![sleep(AFTERNOON - 10_800.0, 7_200.0)];
        let text = with_rule(
            &data,
            crate::domain::today::DayRule::continuous(6.0, 19.5),
            AFTERNOON,
        );
        assert_eq!(
            table_cells(&text, "In last 24h"),
            [
                vec!["Feed", "Sleep"],
                vec!["350 ml total · 4 feeds", "2h 0m total"]
            ]
        );
    }

    /// The label names the window and the value says it is a total, so a
    /// discrete day stops saying "Total" twice over.
    #[test]
    fn a_discrete_day_is_labelled_by_the_day_not_by_the_word_total() {
        let text = joined(&a_newborns_day(), AFTERNOON);
        assert!(text.contains("Today"), "{text}");
        assert!(!text.contains("Total fed today"), "{text}");
        assert!(!text.contains("Total sleep today"), "{text}");
    }

    /// "nothing logged total" is not a sentence, and in a discrete day the
    /// phrase carries a `· since` after it that a naive split would break.
    #[test]
    fn a_window_with_nothing_in_it_never_gains_a_total() {
        let text = joined(&dataset(), AFTERNOON);
        assert!(text.contains("nothing logged"), "{text}");
        assert!(!text.contains("nothing logged total"), "{text}");
    }

    #[test]
    fn a_blank_line_separates_the_age_from_the_first_range() {
        let text = joined(&a_newborns_day(), AFTERNOON);
        let lines: Vec<&str> = text.lines().collect();
        let age = lines
            .iter()
            .position(|line| line.contains("days old"))
            .expect("an age line");
        assert!(lines[age + 1].trim().is_empty(), "{text}");
        assert!(lines[age + 2].contains("typical"), "{text}");
    }

    /// "typical at this age" leaves the reader to work out what of.
    #[test]
    fn every_range_names_what_it_is_about() {
        let text = wide(&a_newborns_day(), AFTERNOON);
        assert!(text.contains("typical feed in the first weeks"), "{text}");
        assert!(text.contains("typical sleep at this age"), "{text}");
        assert!(text.contains("typical diapers from day 3"), "{text}");
    }

    /// Last events precede the recent and daily windows.
    #[test]
    fn the_totals_follow_the_last_events_in_window_order() {
        let text = joined(&a_newborns_day(), AFTERNOON);
        let labels = [
            "Last fed",
            "Last diaper",
            "Last sleep",
            "Last night's sleep",
            "In last 4h",
            "Today",
        ];
        let positions: Vec<_> = labels
            .iter()
            .map(|label| text.find(label).expect("a labelled fact or window"))
            .collect();
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]), "{text}");
    }

    #[test]
    fn the_sleep_totals_say_sleep_rather_than_slept() {
        let text = joined(&a_newborns_day(), AFTERNOON);
        assert!(!text.contains("Slept"), "{text}");
    }

    /// The columns are not rows: nothing on the right belongs to the line it
    /// happens to sit beside, and a rule is what says so.
    #[test]
    fn rules_separate_the_three_columns() {
        let text = wide(&a_newborns_day(), AFTERNOON);
        let columns: Vec<usize> = text
            .lines()
            .filter_map(|line| {
                line.chars()
                    .collect::<Vec<_>>()
                    .iter()
                    .rposition(|glyph| *glyph == '\u{2502}')
            })
            .collect();
        assert!(
            columns.len() >= 8,
            "the rule runs the height of the panel: {text}"
        );
        assert!(
            columns.windows(2).all(|pair| pair[0] == pair[1]),
            "and stays in one column: {text}"
        );
    }

    #[test]
    fn the_night_line_says_last_night_rather_than_naming_the_day() {
        let text = joined(&dataset(), AFTERNOON);
        assert!(text.contains("Last night's sleep"), "{text}");
        assert!(!text.contains("Night of"), "{text}");
    }

    #[test]
    fn only_the_recent_table_has_an_event_line() {
        let text = joined(&dataset(), AFTERNOON);
        assert_eq!(table_cells(&text, "In last 4h").len(), 3);
        assert_eq!(table_cells(&text, "Today").len(), 2);
    }

    /// Three feeds spread over four hours, at the times the design asked for.
    fn three_recent_feeds() -> Dataset {
        let mut data = dataset();
        data.feeds = vec![
            bottle(AFTERNOON - 2_520.0, 30.0),
            bottle(AFTERNOON - 8_220.0, 30.0),
            bottle(AFTERNOON - 11_700.0, 30.0),
        ];
        data
    }

    #[test]
    fn three_recent_feeds_are_listed_on_a_line_of_their_own() {
        let text = joined(&three_recent_feeds(), AFTERNOON);
        assert!(text.contains("3 feeds"), "{text}");
        assert!(text.contains("42m ago · 2h 17m ago · 3h 15m ago"), "{text}");
    }

    /// The list lines up under the value, not under the label it belongs to.
    #[test]
    fn the_list_is_indented_to_the_value_above_it() {
        let text = joined(&three_recent_feeds(), AFTERNOON);
        let lines: Vec<_> = text.lines().collect();
        let total = lines
            .iter()
            .position(|line| line.contains("90 ml total"))
            .expect("a total");
        assert_eq!(
            lines[total].find("90 ml"),
            lines[total + 1].find("42m ago"),
            "{text}"
        );
    }

    /// Two events use the same separator as longer lists.
    #[test]
    fn two_recent_feeds_are_joined_with_a_bullet() {
        let mut data = dataset();
        data.feeds = vec![
            bottle(AFTERNOON - 2_520.0, 30.0),
            bottle(AFTERNOON - 8_220.0, 30.0),
        ];
        let text = joined(&data, AFTERNOON);
        assert!(text.contains("2 feeds"), "{text}");
        assert!(text.contains("42m ago · 2h 17m ago"), "{text}");
        assert!(!text.contains("fed 42m ago ·"), "{text}");
    }

    /// A time stays under the metric it belongs to.
    #[test]
    fn a_single_feed_time_leaves_the_sleep_time_cell_empty() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 2_520.0, 30.0)];
        let text = joined(&data, AFTERNOON);
        let cells = table_cells(&text, "In last 4h");
        assert_eq!(cells[2], ["42m ago", ""]);
    }

    /// Sleeps are listed the same way, measured from when each one ended.
    fn three_recent_sleeps() -> Dataset {
        let mut data = dataset();
        data.sleep = vec![
            sleep(AFTERNOON - 4_320.0, 1_800.0),
            sleep(AFTERNOON - 10_020.0, 1_800.0),
            sleep(AFTERNOON - 13_500.0, 1_800.0),
        ];
        data
    }

    #[test]
    fn three_recent_sleeps_list_times_without_a_verb() {
        let text = joined(&three_recent_sleeps(), AFTERNOON);
        assert!(text.contains("3 sleeps"), "{text}");
        assert!(text.contains("42m ago · 2h 17m ago · 3h 15m ago"), "{text}");
    }

    #[test]
    fn two_recent_sleeps_are_joined_with_a_bullet() {
        let mut data = dataset();
        data.sleep = vec![
            sleep(AFTERNOON - 4_320.0, 1_800.0),
            sleep(AFTERNOON - 10_020.0, 1_800.0),
        ];
        let text = joined(&data, AFTERNOON);
        assert!(text.contains("42m ago · 2h 17m ago"), "{text}");
    }

    #[test]
    fn a_single_sleep_time_leaves_the_feed_time_cell_empty() {
        let mut data = dataset();
        data.sleep = vec![sleep(AFTERNOON - 4_320.0, 1_800.0)];
        let text = joined(&data, AFTERNOON);
        assert_eq!(table_cells(&text, "In last 4h")[2], ["", "42m ago"]);
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
    fn in_the_afternoon_the_stretch_line_is_last_nights() {
        let mut data = dataset();
        data.sleep = vec![sleep(AFTERNOON - 50_400.0, 10_800.0)];
        let text = joined(&data, AFTERNOON);
        assert!(text.contains("Last night's sleep"), "{text}");
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
            .find(|line| line.contains("100 ml total"))
            .unwrap_or_else(|| panic!("no recent feed total in {text}"));
        assert!(recent.contains("100 ml total · 2 feeds"), "{recent}");
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
        assert!(text.contains("Today"), "{text}");
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
        assert!(text.contains("150 ml total · 2 feeds"), "{text}");
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
        assert!(text.contains("In last 24h"), "{text}");
        assert!(text.contains("350 ml total · 4 feeds"), "{text}");
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
            text.contains("Today") && text.contains("80 ml"),
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
    fn each_table_aligns_its_feed_and_sleep_columns() {
        let text = joined(&three_recent_feeds(), AFTERNOON);
        for (heading, count) in [("In last 4h", 3), ("Today", 2)] {
            let borders: Vec<_> = text
                .lines()
                .skip_while(|line| *line != heading)
                .skip(1)
                .take_while(|line| !line.starts_with('└'))
                .filter(|line| line.starts_with('│'))
                .map(|line| {
                    line.chars()
                        .enumerate()
                        .filter_map(|(index, glyph)| (glyph == '│').then_some(index))
                        .collect::<Vec<_>>()
                })
                .collect();
            assert_eq!(borders.len(), count);
            assert!(borders.windows(2).all(|pair| pair[0] == pair[1]), "{text}");
        }
    }

    #[test]
    fn a_window_with_nothing_in_it_says_so_rather_than_printing_a_zero() {
        let text = joined(&dataset(), AFTERNOON);
        for heading in ["In last 4h", "Today"] {
            let cells = table_cells(&text, heading);
            assert!(
                cells[1]
                    .iter()
                    .all(|cell| cell.starts_with("nothing logged")),
                "{text}"
            );
        }
        assert_eq!(table_cells(&text, "In last 4h")[2], ["", ""]);
        assert!(!text.contains("0 ml"), "{text}");
        assert!(!text.contains("0m"), "{text}");
    }

    #[test]
    fn a_sleep_in_progress_counts_towards_what_was_slept_today() {
        let mut data = dataset();
        data.live.sleep_active = true;
        data.live.sleep_start = Some(AFTERNOON - 3_600.0);
        let text = joined(&data, AFTERNOON);
        assert_eq!(table_cells(&text, "Today")[1][1], "1h 0m total");
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
            Some(170),
        )
        .join("\n")
    }

    #[test]
    fn a_wide_screen_carries_the_typical_ranges_beside_the_facts() {
        let text = wide(&a_newborns_day(), AFTERNOON);
        assert!(
            text.contains("typical feed in the first weeks: 8 or more feeds a day"),
            "{text}"
        );
        assert!(
            text.contains("typical diapers from day 3: 5 or more wet diapers a day"),
            "{text}"
        );
        assert!(
            text.contains("typical sleep at this age: 8 to 20 hours a day"),
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
        let text = joined(&a_newborns_day(), AFTERNOON);
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
        let text = joined(&data, AFTERNOON);
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
            None,
        )
        .join("\n");
        assert!(text.contains("(today is over that)"), "{text}");
    }

    /// The pediatrician outranks this table, and the screen says so where the
    /// ranges are rather than only in the manual.
    #[test]
    fn the_ranges_carry_the_note_that_a_pediatrician_outranks_them() {
        let text = joined(&a_newborns_day(), AFTERNOON);
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
            .rposition(|line| line.contains("typical ") && !line.contains("pediatrician"))
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
            text.contains("typical feed in the first weeks: 8 or more feeds a day"),
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
