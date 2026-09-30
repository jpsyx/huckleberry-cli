//! The flow panel: a command, running where the menu was.
//!
//! Choosing a row does not take the screen away. The Menu view becomes the
//! command's questions, its receipt and its failures, and everything else on
//! the screen stays where it was: the header, the Now drawer, the keys. The
//! shell is the container and a command is something that happens inside it.
//!
//! What it draws comes from the prompts themselves, painted rather than
//! rewritten, so there is still one implementation of every question in this
//! tool. See [`crate::prompt::host`].

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Paragraph;

use crate::tui::job::Job;

use super::{block, painted};

/// Draws whatever the command has put on the screen.
pub fn draw(frame: &mut Frame, area: Rect, job: &Job) {
    let outline = block(job.title());
    let inner = outline.inner(area);
    frame.render_widget(outline, area);
    // The prompts lay themselves out to whatever they are told they have.
    crate::prompt::host::set_size(inner.width, inner.height);

    let lines = job
        .visible(usize::from(inner.height))
        .iter()
        .map(|line| painted(line))
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), inner);
}
