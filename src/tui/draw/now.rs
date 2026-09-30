//! The Now drawer: `h now`, left on the screen.
//!
//! **This widget invents nothing.** Every row it draws comes from
//! [`crate::render::now::screen`], which is also what `h now` prints, so the
//! drawer and the command can never word the same fact differently. Changing
//! what the drawer says means changing that function, which changes both.
//!
//! It is the widget the shell exists to show. A parent opening a terminal at
//! 3am is asking one question before any other, "when did she last eat", and
//! the answer should already be on the screen rather than one menu row away.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::domain::now;
use crate::theme::Tone;
use crate::tui::facts::Facts;

use super::{style, tone};

/// The drawer's rows, ready to render inside a block.
#[must_use]
pub fn widget(facts: &Facts, at: f64) -> Paragraph<'static> {
    Paragraph::new(lines(facts, at))
}

/// How many rows the drawer wants, borders included.
///
/// Asked before the layout is split, so the menu can be given everything left
/// over rather than a guess.
#[must_use]
pub fn height(facts: &Facts) -> u16 {
    u16::try_from(lines(facts, measured_at(facts)).len().saturating_add(2)).unwrap_or(u16::MAX)
}

/// The instant used for measuring alone: how many rows there are does not
/// depend on the clock, and measuring must not read one.
fn measured_at(facts: &Facts) -> f64 {
    facts
        .reading()
        .map_or(0.0, |reading| reading.dataset.fetched_at)
}

fn lines(facts: &Facts, at: f64) -> Vec<Line<'static>> {
    let Some(reading) = facts.reading() else {
        return vec![waiting(facts)];
    };
    let view = now::build(&reading.dataset, &reading.calendar, reading.rule, at);
    crate::render::now::screen(
        &view,
        &reading.dataset,
        &reading.calendar,
        reading.units,
        at,
    )
    .into_iter()
    .map(|row| {
        Line::from(
            core::iter::once(Span::raw(" "))
                .chain(
                    row.into_iter()
                        .map(|piece| Span::styled(piece.text, style(piece.tone))),
                )
                .collect::<Vec<_>>(),
        )
    })
    .collect()
}

/// What the drawer says before it has anything to say.
///
/// The one thing here `h now` has no equivalent for: a command that cannot
/// read simply fails, where a drawer has to keep drawing something.
fn waiting(facts: &Facts) -> Line<'static> {
    if facts.refreshing {
        return Line::from(Span::styled(
            " reading…",
            Style::default().fg(tone(Tone::Info)),
        ));
    }
    facts.trouble.as_ref().map_or_else(
        || {
            Line::from(Span::styled(
                " nothing read yet · press r",
                Style::default().fg(tone(Tone::Muted)),
            ))
        },
        |trouble| {
            Line::from(Span::styled(
                format!(" {trouble}"),
                Style::default().fg(tone(Tone::Warning)),
            ))
        },
    )
}
