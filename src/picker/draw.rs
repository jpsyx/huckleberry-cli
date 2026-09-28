//! Drawing the picker.
//!
//! One line per entry, in the order `log` shows them: the day, the clock, what
//! it was, and what it said. The entry under the cursor is the brightest thing
//! on the screen, and an entry this tool cannot change is drawn muted and says
//! so at the foot rather than silently refusing a keystroke.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph};

use crate::dashboard::draw::tone;
use crate::domain::Calendar;
use crate::domain::log::Entry;
use crate::render::format;
use crate::render::log as log_render;
use crate::theme::Tone;

use super::state::Picker;

/// How wide the title column is, as in `render::log`.
const TITLE_WIDTH: usize = 9;

/// Draws the whole frame.
pub fn draw(frame: &mut Frame, picker: &Picker, calendar: &Calendar) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    frame.render_widget(heading(picker), areas[0]);

    let items: Vec<ListItem> = picker
        .entries
        .iter()
        .map(|entry| ListItem::new(row(entry, calendar, picker.purpose.allows(entry))))
        .collect();
    let list = List::new(items).highlight_symbol("› ").highlight_style(
        Style::default()
            .fg(tone(Tone::Today))
            .add_modifier(Modifier::BOLD),
    );
    // The offset is worked out from the selection on every draw, so the
    // cursor is the only thing this screen has to remember.
    let mut scroll = ListState::default().with_selected(Some(picker.cursor));
    frame.render_stateful_widget(list, areas[1], &mut scroll);

    frame.render_widget(footer(picker), areas[2]);
}

/// The line at the top: what is being asked, and how many there are.
fn heading(picker: &Picker) -> Paragraph<'static> {
    let count = picker.entries.len();
    Paragraph::new(Line::from(vec![
        Span::styled(
            picker.purpose.question(),
            Style::default()
                .fg(tone(Tone::Prompt))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{count} in this window"),
            Style::default().fg(tone(Tone::Muted)),
        ),
    ]))
}

/// One entry's line.
fn row(entry: &Entry, calendar: &Calendar, allowed: bool) -> Line<'static> {
    let day = format::day_short(calendar.day_of(entry.start));
    let when = format!("{day}  {:>8}", format::clock(entry.start, calendar));
    let muted = Style::default().fg(tone(Tone::Muted));
    let mut spans = vec![
        Span::styled(format!("{when}  "), muted),
        Span::styled(
            format::pad(&entry.title, TITLE_WIDTH),
            Style::default().fg(if allowed {
                tone(log_render::tone_for(entry.kind))
            } else {
                tone(Tone::Muted)
            }),
        ),
        Span::styled(
            format!("  {}", entry.description),
            Style::default().fg(if allowed {
                tone(Tone::Value)
            } else {
                tone(Tone::Muted)
            }),
        ),
    ];
    if let Some(notes) = &entry.notes {
        spans.push(Span::styled(format!(" · {notes}"), muted));
    }
    Line::from(spans)
}

/// The line at the foot: the keys, or what the last keystroke could not do.
fn footer(picker: &Picker) -> Paragraph<'static> {
    picker.trouble.as_ref().map_or_else(
        || {
            Paragraph::new(Span::styled(
                picker.purpose.keys(),
                Style::default().fg(tone(Tone::Muted)),
            ))
        },
        |trouble| {
            Paragraph::new(Span::styled(
                trouble.clone(),
                Style::default().fg(tone(Tone::Warning)),
            ))
        },
    )
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;
    use crate::domain::fixtures::{AFTERNOON, bottle, dataset, diaper};
    use crate::domain::log;
    use crate::domain::types::{MilestoneEvent, PumpEvent};

    fn screen(picker: &Picker) -> String {
        let calendar = Calendar::new("America/New_York").expect("a real timezone");
        let mut terminal = Terminal::new(TestBackend::new(90, 12)).expect("a test terminal");
        terminal
            .draw(|frame| draw(frame, picker, &calendar))
            .expect("a frame");
        let buffer = terminal.backend().buffer().clone();
        (0..buffer.area.height)
            .map(|row| {
                (0..buffer.area.width)
                    .map(|column| buffer[(column, row)].symbol().to_owned())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn entries() -> Vec<log::Entry> {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 3_600.0, 90.0)];
        data.diapers = vec![diaper(AFTERNOON - 1_800.0, true, false)];
        data.pumps = vec![PumpEvent {
            at: None,
            id: "p1".to_owned(),
            start: AFTERNOON - 900.0,
            left_ml: Some(60.0),
            right_ml: Some(40.0),
            total_ml: Some(100.0),
            duration_seconds: Some(900.0),
            notes: None,
        }];
        data.milestones = vec![MilestoneEvent {
            at: None,
            id: "m1".to_owned(),
            start: AFTERNOON - 300.0,
            name: "First smile".to_owned(),
            category: None,
            notes: None,
            has_photo: false,
        }];
        log::build(&data)
    }

    #[test]
    fn every_entry_is_on_the_screen_with_the_day_it_happened() {
        let text = screen(&Picker::new(entries()));
        assert!(text.contains("Bottle"), "{text}");
        assert!(text.contains("Diaper"), "{text}");
        assert!(text.contains("Pumping"), "{text}");
        assert!(text.contains("Mon 22 Sep"), "{text}");
    }

    #[test]
    fn every_kind_of_entry_is_drawn_in_its_own_colour() {
        let calendar = Calendar::new("America/New_York").expect("a real timezone");
        let entries = entries();
        let colours: Vec<_> = entries
            .iter()
            .map(|entry| {
                row(entry, &calendar, true).spans[1]
                    .style
                    .fg
                    .expect("a colour")
            })
            .collect();
        let kinds: Vec<_> = entries.iter().map(|entry| entry.kind).collect();
        for (position, kind) in kinds.iter().enumerate() {
            for (other_position, other) in kinds.iter().enumerate() {
                if kind != other {
                    assert_ne!(
                        colours[position], colours[other_position],
                        "{kind:?} and {other:?} share a colour"
                    );
                }
            }
        }
    }

    #[test]
    fn the_cursor_marks_the_entry_it_is_on() {
        let mut picker = Picker::new(entries());
        picker.move_by(1);
        let text = screen(&picker);
        assert!(text.contains('›'), "{text}");
    }

    #[test]
    fn the_keys_are_on_the_screen_until_something_goes_wrong() {
        let mut picker = Picker::new(entries());
        assert!(screen(&picker).contains("enter edits"));
        picker.trouble = Some("nothing doing".to_owned());
        let text = screen(&picker);
        assert!(text.contains("nothing doing"), "{text}");
        assert!(!text.contains("enter edits"), "{text}");
    }

    #[test]
    fn an_empty_window_draws_without_panicking() {
        let text = screen(&Picker::new(Vec::new()));
        assert!(text.contains("0 in this window"), "{text}");
    }
}
