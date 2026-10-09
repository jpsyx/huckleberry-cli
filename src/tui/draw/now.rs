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

/// The drawer's rows, ready to render inside a block `width` columns wide.
#[must_use]
pub fn widget(facts: &Facts, at: f64, width: u16) -> Paragraph<'static> {
    Paragraph::new(lines(facts, at, width))
}

/// How many rows the drawer wants, borders included.
///
/// Asked before the layout is split, so the menu can be given everything left
/// over rather than a guess. Measurement uses the same instant as rendering,
/// because elapsed-time labels can wrap as the clock advances.
#[must_use]
pub fn height(facts: &Facts, width: u16, at: f64) -> u16 {
    u16::try_from(lines(facts, at, width).len().saturating_add(2)).unwrap_or(u16::MAX)
}

fn lines(facts: &Facts, at: f64, width: u16) -> Vec<Line<'static>> {
    let Some(reading) = facts.reading() else {
        return vec![waiting(facts)];
    };
    let view = now::build(&reading.dataset, &reading.calendar, reading.rule, at);
    // Two columns here too when the panel is wide enough: the drawer is
    // `h now`, and it lays itself out the same way.
    crate::render::now::laid_out(
        &view,
        &reading.dataset,
        &reading.calendar,
        reading.units,
        at,
        // Its own width, not the terminal's: the drawer is a panel.
        Some(usize::from(width).saturating_sub(1)),
        // A drawer cannot grow: stacking the ranges under the facts would make
        // the glance strip the biggest thing on a screen it shares.
        crate::render::now::WhenNarrow::Drop,
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
