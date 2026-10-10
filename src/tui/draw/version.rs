//! Centered Version modal, drawn over the shell without owning I/O.
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Wrap};

use super::{block, style};
use crate::theme::Tone;
use crate::tui::version::{CheckState, State};
use crate::version::UpdateStatus;

/// Modal controls replace ordinary menu hints while the overlay is open.
pub const HINTS: &str = "Enter/→ l/d close · Esc/← h/a back · r retry · q quit";

/// Keep the modal centered and entirely inside the current terminal bounds.
#[must_use]
pub const fn area(terminal: Rect) -> Rect {
    let width = if terminal.width < 64 {
        terminal.width
    } else {
        64
    };
    let height = if terminal.height < 12 {
        terminal.height
    } else {
        12
    };
    Rect::new(
        terminal.x + (terminal.width - width) / 2,
        terminal.y + (terminal.height - height) / 2,
        width,
        height,
    )
}

/// Paint an opaque overlay with a default Close action and concise check status.
pub fn draw(frame: &mut Frame, state: &State) {
    let modal = area(frame.area());
    let outline = block("Version");
    let inner = outline.inner(modal);
    frame.render_widget(Clear, modal);
    frame.render_widget(outline, modal);
    frame.render_widget(
        Paragraph::new(lines(state))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
        inner,
    );
}

fn lines(state: &State) -> Vec<Line<'static>> {
    let (installed, latest, message, reason, tone) = match &state.check {
        CheckState::Checking => (
            env!("CARGO_PKG_VERSION"),
            None,
            "Checking…",
            None,
            Tone::Info,
        ),
        CheckState::Complete(report) => (
            report.installed.as_str(),
            report.latest.as_deref(),
            report.status.message(),
            report.reason.as_deref(),
            status_tone(report.status),
        ),
    };
    vec![
        Line::default(),
        Line::styled(format!("Installed  {installed}"), style(Tone::Value)),
        Line::styled(
            format!("Latest     {}", latest.unwrap_or("Unknown")),
            style(Tone::Value),
        ),
        Line::default(),
        Line::styled(message, style(tone)),
        Line::styled(reason.unwrap_or("").to_owned(), style(Tone::Muted)),
        Line::default(),
        Line::from(Span::styled(" Close ", style(Tone::Selected))),
    ]
}

const fn status_tone(status: UpdateStatus) -> Tone {
    match status {
        UpdateStatus::UpToDate => Tone::Success,
        UpdateStatus::UpdateAvailable | UpdateStatus::Unavailable => Tone::Warning,
        UpdateStatus::Ahead | UpdateStatus::Offline | UpdateStatus::NoRelease => Tone::Info,
    }
}
