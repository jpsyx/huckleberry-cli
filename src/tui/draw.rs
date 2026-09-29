//! Drawing the shell.
//!
//! One frame, always the same three parts: who and where at the top, the rows
//! in the middle, the keys along the bottom. The parts never move, because a
//! screen somebody glances at with a baby on one arm is read by position
//! before it is read by word.
//!
//! Colours come from [`crate::theme`] through [`crate::dashboard::draw::tone`],
//! so this screen and every other one in the tool agree on what a heading
//! looks like.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use super::state::{App, Row};
use crate::dashboard::draw::tone;
use crate::theme::Tone;

/// The keys, spelled out on every screen rather than hidden behind `?`.
pub const HINTS: &str = "↑/↓ j/k move · ← h back · → l open · Enter select · q quit";

/// The cursor, drawn as a bar down the left of the highlighted row.
const CURSOR: &str = "▌ ";

/// A row that opens another menu says so.
const CHEVRON: char = '›';

/// How many rows fit in the list, given the whole terminal's height.
///
/// The header, the footer and the block's two borders are the four lines this
/// takes off, and the loop passes the same number to
/// [`App::apply`](super::state::App::apply) so scrolling and drawing agree.
#[must_use]
pub const fn viewport(height: u16) -> usize {
    match (height as usize).saturating_sub(4) {
        0 => 1,
        rows => rows,
    }
}

/// Draws the whole frame.
pub fn draw(frame: &mut Frame, app: &App) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(frame.area());

    frame.render_widget(header(app, areas[0].width), areas[0]);
    menu(frame, areas[1], app);
    frame.render_widget(footer(app), areas[2]);
}

/// The name of the tool on the left, and where you are on the right.
fn header(app: &App, width: u16) -> Paragraph<'static> {
    let breadcrumb = app.breadcrumb();
    let used = crate::APP_TITLE.chars().count() + breadcrumb.chars().count() + 2;
    let gap = usize::from(width).saturating_sub(used).max(1);
    Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {}", crate::APP_TITLE),
            Style::default()
                .fg(tone(Tone::Heading))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(gap)),
        Span::styled(breadcrumb, Style::default().fg(tone(Tone::Accent))),
    ]))
}

/// The rows, inside a block that names the question they answer.
fn menu(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(tone(Tone::Muted)))
        .title(Span::styled(
            format!(" {} ", app.title()),
            Style::default().fg(tone(Tone::Heading)),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = app.rows();
    // One number width for the whole menu, so every label starts in the same
    // column whether its row is 9 or 10.
    let digits = rows.len().to_string().len();
    let items = rows
        .iter()
        .enumerate()
        .map(|(index, row)| item(index, row, inner.width, digits))
        .collect::<Vec<_>>();
    let list = List::new(items).highlight_symbol(CURSOR).highlight_style(
        Style::default()
            .fg(tone(Tone::Selected))
            .add_modifier(Modifier::BOLD),
    );
    let mut state = ListState::default()
        .with_selected(Some(app.cursor()))
        .with_offset(app.top());
    frame.render_stateful_widget(list, inner, &mut state);
}

/// One row: its number, its label, and a chevron when it opens a menu.
fn item(index: usize, row: &Row, width: u16, digits: usize) -> ListItem<'static> {
    let number = format!("{:>digits$}. ", index + 1);
    // The cursor symbol is drawn by the list, in the same width on every row.
    let room = usize::from(width).saturating_sub(CURSOR.chars().count() + number.chars().count());
    // Two columns held back: the chevron, and a space between it and the edge.
    let label = crate::prompt::select::clip(&row.label, room.saturating_sub(2));
    let mut spans = vec![
        Span::styled(number, Style::default().fg(tone(Tone::Muted))),
        Span::styled(label.clone(), Style::default().fg(tone(Tone::Value))),
    ];
    if row.opens_menu {
        let used = Line::raw(&label).width();
        spans.push(Span::raw(" ".repeat(room.saturating_sub(used + 2))));
        spans.push(Span::styled(
            CHEVRON.to_string(),
            Style::default().fg(tone(Tone::Muted)),
        ));
    }
    ListItem::new(Line::from(spans))
}

/// The keys, and whatever the last action had to say.
fn footer(app: &App) -> Paragraph<'static> {
    let mut spans = Vec::new();
    if let Some(status) = app.status() {
        spans.push(Span::styled(
            format!(" {status} · "),
            Style::default().fg(tone(Tone::Info)),
        ));
    } else {
        spans.push(Span::raw(" "));
    }
    spans.push(Span::styled(HINTS, Style::default().fg(tone(Tone::Muted))));
    Paragraph::new(Line::from(spans))
}
