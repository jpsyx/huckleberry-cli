//! The Now widget: what `h now` says, kept on the screen permanently.
//!
//! This is the widget the shell exists to show. A parent opening a terminal at
//! 3am is asking one question before any other, "when did she last eat", and
//! the answer should already be on the screen rather than one menu row away.
//!
//! It carries an honest `as of`, because a stale "last fed two hours ago" is
//! what sends somebody to wake a sleeping baby.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::domain::now::{self, NowView};
use crate::domain::time::{format_ago, format_duration};
use crate::domain::types::FeedEvent;
use crate::render::format;
use crate::theme::Tone;
use crate::tui::facts::{Facts, Reading};

use super::tone;

/// The label column, wide enough for `Nursing now`.
const LABEL: usize = 12;

/// The widget, ready to render inside a block.
#[must_use]
pub fn widget(facts: &Facts, at: f64, stacked: bool) -> Paragraph<'static> {
    Paragraph::new(lines(facts, at, stacked))
}

/// How many rows the widget wants, borders included.
///
/// Asked before the layout is split, so the menu can be given everything left
/// over rather than a guess.
#[must_use]
pub fn height(facts: &Facts, stacked: bool) -> u16 {
    u16::try_from(
        lines(facts, at_nothing(facts), stacked)
            .len()
            .saturating_add(2),
    )
    .unwrap_or(u16::MAX)
}

/// The instant used for measuring alone: how many lines there are does not
/// depend on the clock, and measuring must not read one.
fn at_nothing(facts: &Facts) -> f64 {
    facts
        .reading()
        .map_or(0.0, |reading| reading.dataset.fetched_at)
}

fn lines(facts: &Facts, at: f64, stacked: bool) -> Vec<Line<'static>> {
    let Some(reading) = facts.reading() else {
        return vec![waiting(facts)];
    };
    let view = now::build(&reading.dataset, &reading.calendar, reading.rule, at);
    let mut lines = Vec::new();
    for (label, value, detail) in facts_of(&view, reading, at) {
        push_fact(&mut lines, &label, &value, &detail, stacked);
    }
    if !stacked {
        lines.push(Line::default());
    }
    lines.push(muted(&format!(
        " {}",
        crate::render::now::as_of(&reading.dataset, at)
    )));
    for note in &reading.dataset.notes {
        lines.push(muted(&format!(
            " {} unread: {}",
            note.collection, note.problem
        )));
    }
    lines
}

/// What the widget says before it has anything to say.
fn waiting(facts: &Facts) -> Line<'static> {
    if facts.refreshing {
        return Line::from(Span::styled(
            " reading…",
            Style::default().fg(tone(Tone::Info)),
        ));
    }
    facts.trouble.as_ref().map_or_else(
        || muted(" nothing read yet · press r"),
        |trouble| {
            Line::from(Span::styled(
                format!(" {trouble}"),
                Style::default().fg(tone(Tone::Warning)),
            ))
        },
    )
}

/// The four facts, in the order they get asked, plus nursing when it is live.
fn facts_of(view: &NowView, reading: &Reading, at: f64) -> Vec<(String, String, String)> {
    let mut rows = vec![
        (
            "Last fed".to_owned(),
            feed(view, at),
            feed_detail(view, reading),
        ),
        (
            "Diaper".to_owned(),
            view.last_diaper.as_ref().map_or_else(
                || "nothing logged".to_owned(),
                |last| format_ago(last.start, at),
            ),
            view.last_diaper
                .as_ref()
                .map_or_else(String::new, |last| last.label().to_owned()),
        ),
        sleep(view),
        stretch(view),
    ];
    if let Some(nursing) = &view.nursing_now {
        let paused = if nursing.paused { " (paused)" } else { "" };
        rows.push((
            "Nursing now".to_owned(),
            format_duration(nursing.elapsed_seconds),
            format!("on the {}{paused}", nursing.side),
        ));
    }
    // The running totals fit on one line each, so they carry no second line
    // even in the tall narrow column: a detail under each would push the
    // widget past the height a sidebar has to spare.
    let window = crate::render::now::window_label(view);
    rows.push((
        format!("Last {}h", crate::domain::now::RECENT_HOURS as i64),
        crate::render::now::intake(&view.recent, reading.units),
        String::new(),
    ));
    rows.push((
        format!("Fed {window}"),
        crate::render::now::intake(&view.today, reading.units),
        String::new(),
    ));
    rows.push((
        format!("Slept {window}"),
        crate::render::now::slept(&view.today),
        String::new(),
    ));
    rows
}

fn feed(view: &NowView, at: f64) -> String {
    view.last_feed.as_ref().map_or_else(
        || "nothing logged".to_owned(),
        |last| format_ago(last.start, at),
    )
}

/// What the last feed was, in as few words as the column allows.
fn feed_detail(view: &NowView, reading: &Reading) -> String {
    let Some(last) = &view.last_feed else {
        return String::new();
    };
    match &last.feed {
        FeedEvent::Bottle {
            amount_ml,
            bottle_type,
            ..
        } => format!(
            "{} of {}",
            format::volume(*amount_ml, reading.units),
            bottle_type.as_deref().unwrap_or("milk")
        ),
        FeedEvent::Nursing {
            left_seconds,
            right_seconds,
            ..
        } => format!("nursed {}", format_duration(left_seconds + right_seconds)),
        FeedEvent::Solids { foods, .. } => {
            if foods.is_empty() {
                "solids".to_owned()
            } else {
                foods.join(", ")
            }
        }
    }
}

/// Asleep or awake, as the label rather than as the value, so the state is
/// read before the number is.
fn sleep(view: &NowView) -> (String, String, String) {
    let state = &view.sleep_state;
    let label = if state.asleep { "Asleep" } else { "Awake" };
    let value = state
        .asleep_seconds
        .or(state.awake_seconds)
        .map_or_else(|| "nothing logged".to_owned(), format_duration);
    let detail = if state.paused { "timer paused" } else { "" };
    (label.to_owned(), value, detail.to_owned())
}

/// The longest finished stretch, with the night it belongs to spelled out.
///
/// The night is the detail rather than the label because "Night of Sun 21 Sep"
/// does not fit a label column, and leaving it off is how a figure gets read
/// as last night when it is not.
fn stretch(view: &NowView) -> (String, String, String) {
    let stretch = &view.longest_stretch;
    let detail = if stretch.tonight {
        "tonight so far".to_owned()
    } else {
        format!("night of {}", format::day_short(stretch.night_of))
    };
    let value = stretch.seconds.map_or_else(
        || {
            if stretch.tonight {
                "nothing finished yet".to_owned()
            } else {
                "nothing logged".to_owned()
            }
        },
        format_duration,
    );
    ("Longest".to_owned(), value, detail)
}

/// One fact: two lines in a narrow column, one line in a wide short one.
fn push_fact(
    lines: &mut Vec<Line<'static>>,
    label: &str,
    value: &str,
    detail: &str,
    stacked: bool,
) {
    let name = Span::styled(
        format!(" {}", format::pad(label, LABEL)),
        Style::default().fg(tone(Tone::Accent)),
    );
    let number = Span::styled(
        value.to_owned(),
        Style::default()
            .fg(tone(Tone::Value))
            .add_modifier(Modifier::BOLD),
    );
    if detail.is_empty() {
        lines.push(Line::from(vec![name, number]));
    } else if stacked {
        lines.push(Line::from(vec![
            name,
            number,
            Span::styled(
                format!(" · {detail}"),
                Style::default().fg(tone(Tone::Muted)),
            ),
        ]));
    } else {
        lines.push(Line::from(vec![name, number]));
        lines.push(muted(&format!("{}{detail}", " ".repeat(LABEL + 1))));
    }
}

fn muted(text: &str) -> Line<'static> {
    Line::from(Span::styled(
        text.to_owned(),
        Style::default().fg(tone(Tone::Muted)),
    ))
}
