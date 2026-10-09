//! The one list view every listing in this tool renders through.
//!
//! `log`, `edit`, `delete`, `child list` and `foods list` all show the same
//! shape of thing: titled, column-aligned rows, sometimes under headings,
//! always worth searching. Rather than each command working out widths,
//! colours and paging for itself, they hand this module [`Row`]s and it owns
//! the whole surface.
//!
//! - **Searching.** `/` opens a live filter; every whitespace-separated term
//!   has to match somewhere in the row, so typing narrows. A group keeps its
//!   heading while one of its rows survives.
//! - **Browsing.** `j`/`k`, the arrows and Ctrl-J/Ctrl-K move; `u`/`d` and the
//!   page keys move a half screen; `g`/`G` reach the ends. Three keys leave.
//! - **Choosing.** Enter hands the row's key back to the command, which is how
//!   `edit` and `delete` pick one; where there is nothing to do with a row,
//!   Enter shows what is known about it instead.
//!
//! Which of the two shapes a listing takes is not a question of how tall the
//! window is. Piped output and anything an agent runs are always plain, so a
//! listing stays parseable. On a terminal, a listing asked for with no filter
//! is a place to look around, so it opens the searchable view; naming a filter
//! says "just show me", so that prints and gets out of the way.
//!
//! The split is the one the dashboard uses: [`model`], [`layout`] and
//! [`state`] are pure and carry the whole design, and this file owns the
//! terminal. Ported from the listing view in this author's `jpsyx` CLI.

pub mod layout;
pub mod model;
pub mod state;

use anyhow::{Context as _, Result};
use crossterm::event::{Event, KeyEventKind};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line as TextLine, Span};
use ratatui::widgets::Paragraph;

use crate::dashboard::draw::tone;
use crate::theme::{Theme, Tone};

pub use layout::{GUTTER, Line, MARKER, Piece};
pub use model::{Column, Role, Row};
pub use state::{Flow, Key, State};

/// Whether a listing printed new content or was explicitly left by the reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShowOutcome {
    /// Static output or entry details still need time to be read.
    Displayed,
    /// The reader pressed Back while browsing the listing.
    Back,
}

/// A listing, ready to show. Built with the setters, then [`Listing::show`] or
/// [`Listing::choose`].
pub struct Listing<'a> {
    title: String,
    noun: &'a str,
    columns: &'a [Column],
    rows: Vec<Row>,
    heading: Box<dyn Fn(&str) -> String + 'a>,
    today: Box<dyn Fn(&str) -> bool + 'a>,
    empty: String,
    query: String,
    verb: &'static str,
}

impl<'a> Listing<'a> {
    /// A listing with a title, the plural word for what is in it, and its
    /// columns.
    #[must_use]
    pub fn new(title: impl Into<String>, noun: &'a str, columns: &'a [Column]) -> Self {
        Self {
            title: title.into(),
            noun,
            columns,
            rows: Vec::new(),
            heading: Box::new(str::to_owned),
            today: Box::new(|_| false),
            empty: format!("no {noun}"),
            query: String::new(),
            verb: "↑/↓ j/k w/s move · / searches · enter opens · h/a or q leaves",
        }
    }

    /// What to list.
    #[must_use]
    pub fn rows(mut self, rows: Vec<Row>) -> Self {
        self.rows = rows;
        self
    }

    /// Turns a group's key into the heading above it.
    #[must_use]
    pub fn group_heading(mut self, heading: impl Fn(&str) -> String + 'a) -> Self {
        self.heading = Box::new(heading);
        self
    }

    /// Says which heading is today's, which is the one somebody is looking
    /// for and so the brightest on the screen.
    #[must_use]
    pub fn today(mut self, today: impl Fn(&str) -> bool + 'a) -> Self {
        self.today = Box::new(today);
        self
    }

    /// What to say when there is nothing at all to list.
    #[must_use]
    pub fn empty(mut self, message: impl Into<String>) -> Self {
        self.empty = message.into();
        self
    }

    /// Starts with this filter applied.
    #[must_use]
    pub fn query(mut self, query: Option<&str>) -> Self {
        query.unwrap_or_default().clone_into(&mut self.query);
        self
    }

    /// What the keys do, for the line at the foot of the screen.
    #[must_use]
    pub const fn verb(mut self, verb: &'static str) -> Self {
        self.verb = verb;
        self
    }

    /// The whole listing as lines of text, for a pipe or a screen that is not
    /// being browsed.
    #[must_use]
    pub fn lines(&self, theme: Theme, width: Option<usize>) -> Vec<String> {
        if self.rows.is_empty() {
            return vec![theme.muted(&self.empty)];
        }
        let (head, body) = self.frame(&self.query, false, width);
        head.iter()
            .chain(&body)
            .map(|line| line.painted(theme))
            .collect()
    }

    /// Shows the listing: browsable on a terminal, plain text otherwise.
    ///
    /// Enter opens what is under the cursor, which for a listing nobody is
    /// choosing from means printing what is known about it. In the shell,
    /// explicit Back cancels the command so it cannot add a completion prompt.
    pub fn show(&self, theme: Theme) -> Result<()> {
        if self.show_with_outcome(theme)? == ShowOutcome::Back && crate::prompt::host::hosted() {
            return Err(crate::prompt::Cancelled.into());
        }
        Ok(())
    }

    /// Shows a listing while preserving whether the reader explicitly went back.
    /// Static output and entry details remain distinguishable from navigation.
    pub fn show_with_outcome(&self, theme: Theme) -> Result<ShowOutcome> {
        let browsed = !self.rows.is_empty() && (crate::prompt::host::hosted() || browsable());
        let Some(chosen) = self.open(theme, false)? else {
            return Ok(if browsed {
                ShowOutcome::Back
            } else {
                ShowOutcome::Displayed
            });
        };
        let Some(row) = self.rows.iter().find(|row| row.key == chosen) else {
            return Ok(ShowOutcome::Displayed);
        };
        let width = row
            .detail
            .iter()
            .map(|(label, _)| label.chars().count())
            .max()
            .unwrap_or(0);
        crate::render::print(
            &row.detail
                .iter()
                .map(|(label, value)| {
                    format!(
                        "{}  {}",
                        theme.accent(&format!("{label:<width$}")),
                        theme.value(value)
                    )
                })
                .collect::<Vec<_>>(),
        );
        Ok(ShowOutcome::Displayed)
    }

    /// Opens the listing for somebody to pick one row, and hands back its key.
    ///
    /// In the shell, explicit Back cancels the command before its completion
    /// receipt. `None` means an empty result, or leaving a standalone listing.
    pub fn choose(&self, theme: Theme) -> Result<Option<String>> {
        let chosen = self.open(theme, true)?;
        if chosen.is_none() && !self.rows.is_empty() && crate::prompt::host::hosted() {
            return Err(crate::prompt::Cancelled.into());
        }
        Ok(chosen)
    }

    /// The listing, one way or the other.
    fn open(&self, theme: Theme, choosing: bool) -> Result<Option<String>> {
        if self.rows.is_empty() {
            crate::render::note(&theme.muted(&self.empty));
            return Ok(None);
        }
        if crate::prompt::host::hosted() {
            return self.hosted(theme, choosing);
        }
        if !browsable() {
            crate::render::print(&self.lines(theme, None));
            return Ok(None);
        }
        let mut terminal = ratatui::init();
        let chosen = self.browse(&mut terminal, choosing);
        ratatui::restore();
        chosen
    }

    /// The same listing, drawn by whatever is hosting the screen.
    ///
    /// The same filter, the same state and the same layout: only the drawing
    /// and the keyboard come from somewhere else.
    fn hosted(&self, theme: Theme, choosing: bool) -> Result<Option<String>> {
        let mut state = State::new(self.query.clone());
        loop {
            let (width, panel) = crate::prompt::host::size();
            let number_width = self.rows.len().to_string().len() + 2;
            let (head, mut body) = self.frame(
                &state.query,
                state.searching,
                Some(usize::from(width).saturating_sub(number_width)),
            );
            layout::number_rows(&mut body);
            let filtered = model::filter(&self.rows, &state.query);
            let height = usize::from(panel).saturating_sub(head.len() + 1).max(1);
            state.cursor = state.cursor.min(filtered.len().saturating_sub(1));
            let line = body.iter().position(|line| line.row == Some(state.cursor));
            state.top = state::scrolled(state.top, line.unwrap_or(0), height, body.len());

            let drawn = rendered(&head, &body, &state, self.verb, height, theme);
            let key = match crate::prompt::host::frame(drawn)? {
                crate::prompt::host::Input::Key(key) => key,
                crate::prompt::host::Input::Pasted(text) => {
                    for character in text.chars().filter(|character| !character.is_control()) {
                        let typed = crossterm::event::KeyEvent::new(
                            crossterm::event::KeyCode::Char(character),
                            crossterm::event::KeyModifiers::NONE,
                        );
                        state::apply(
                            &mut state,
                            state::key_for(typed),
                            filtered.len(),
                            height / 2,
                        );
                    }
                    continue;
                }
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match state::apply(&mut state, state::key_for(key), filtered.len(), height / 2) {
                Flow::Stay => {}
                Flow::Quit => return Ok(None),
                Flow::Choose => {
                    let Some(row) = filtered.get(state.cursor) else {
                        continue;
                    };
                    if choosing && !row.selectable {
                        state.trouble.clone_from(&row.refusal);
                        continue;
                    }
                    if !choosing && row.detail.is_empty() {
                        continue;
                    }
                    return Ok(Some(row.key.clone()));
                }
            }
        }
    }

    /// The head lines and the body lines for one pass.
    fn frame(&self, query: &str, searching: bool, width: Option<usize>) -> (Vec<Line>, Vec<Line>) {
        let filtered = model::filter(&self.rows, query);
        let widths = layout::widths(self.columns, &filtered, width);
        let status = model::status(self.noun, filtered.len(), self.rows.len(), query, searching);
        let head = vec![
            Line::text(vec![
                Piece::new(format!("{} ", self.title), Tone::Heading),
                Piece::new(status, Tone::Muted),
            ]),
            Line::text(Vec::new()),
            layout::header_line(self.columns, &widths),
        ];
        let body = if filtered.is_empty() {
            vec![Line::text(vec![Piece::new(
                format!("{GUTTER}nothing matches `{query}`"),
                Tone::Attention,
            )])]
        } else {
            layout::body_lines(
                self.columns,
                &model::grouped(&filtered),
                &widths,
                &*self.heading,
                &*self.today,
            )
        };
        (head, body)
    }

    /// The keyboard loop, with the terminal already set up.
    fn browse(
        &self,
        terminal: &mut ratatui::DefaultTerminal,
        choosing: bool,
    ) -> Result<Option<String>> {
        let mut state = State::new(self.query.clone());
        loop {
            let size = terminal.size().context("asking the terminal its size")?;
            let number_width = self.rows.len().to_string().len() + 2;
            let (head, mut body) = self.frame(
                &state.query,
                state.searching,
                Some(usize::from(size.width).saturating_sub(number_width)),
            );
            layout::number_rows(&mut body);
            let filtered = model::filter(&self.rows, &state.query);
            let height = usize::from(size.height)
                .saturating_sub(head.len() + 2)
                .max(1);
            state.cursor = state.cursor.min(filtered.len().saturating_sub(1));
            let line = body.iter().position(|line| line.row == Some(state.cursor));
            state.top = state::scrolled(state.top, line.unwrap_or(0), height, body.len());

            terminal
                .draw(|frame| draw(frame, &head, &body, &state, self.verb))
                .context("drawing the list")?;

            let Event::Key(key) = crossterm::event::read().context("reading a keystroke")? else {
                continue;
            };
            // Windows reports press and release; acting on both would move two
            // rows for one keystroke.
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match state::apply(&mut state, state::key_for(key), filtered.len(), height / 2) {
                Flow::Stay => {}
                Flow::Quit => return Ok(None),
                Flow::Choose => {
                    let Some(row) = filtered.get(state.cursor) else {
                        continue;
                    };
                    if choosing && !row.selectable {
                        state.trouble.clone_from(&row.refusal);
                        continue;
                    }
                    if !choosing && row.detail.is_empty() {
                        continue;
                    }
                    return Ok(Some(row.key.clone()));
                }
            }
        }
    }
}

/// Whether there is somebody at the terminal to browse.
///
/// Both ends have to be a terminal: a listing whose output is going into a
/// pipe stays plain text even when somebody is typing at the other end.
fn browsable() -> bool {
    use std::io::IsTerminal as _;
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

/// Paints one frame.
fn draw(frame: &mut Frame, head: &[Line], body: &[Line], state: &State, verb: &'static str) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(u16::try_from(head.len()).unwrap_or(3)),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(frame.area());

    frame.render_widget(Paragraph::new(spans(head)), areas[0]);

    let height = areas[1].height as usize;
    let end = (state.top + height).min(body.len());
    let window: Vec<TextLine> = body[state.top.min(body.len())..end]
        .iter()
        .map(|line| {
            if line.row == Some(state.cursor) {
                // The row under the cursor is the brightest thing on the
                // screen, as today is everywhere else in this tool.
                TextLine::from(vec![Span::styled(
                    line.plain()
                        .strip_prefix(GUTTER)
                        .map_or_else(|| line.plain(), |rest| format!("{MARKER}{rest}")),
                    Style::default()
                        .fg(tone(Tone::Today))
                        .add_modifier(Modifier::BOLD),
                )])
            } else {
                paint(line)
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(window), areas[1]);

    let foot = state.trouble.as_ref().map_or_else(
        || {
            Span::styled(
                state::hint(state.searching, verb),
                Style::default().fg(tone(Tone::Muted)),
            )
        },
        |trouble| Span::styled(trouble.clone(), Style::default().fg(tone(Tone::Attention))),
    );
    frame.render_widget(Paragraph::new(TextLine::from(foot)), areas[2]);
}

/// One pass of a hosted listing, as lines of text.
///
/// The same window, the same cursor and the same foot as the browsable
/// version draws, painted rather than turned into widgets.
fn rendered(
    head: &[Line],
    body: &[Line],
    state: &State,
    verb: &'static str,
    height: usize,
    theme: Theme,
) -> Vec<String> {
    let mut lines: Vec<String> = head.iter().map(|line| line.painted(theme)).collect();
    let top = state.top.min(body.len());
    let end = (top + height).min(body.len());
    for line in &body[top..end] {
        lines.push(if line.row == Some(state.cursor) {
            // The row under the cursor is the brightest thing on the screen,
            // as today is everywhere else in this tool.
            theme.paint(
                Tone::Today,
                &line
                    .plain()
                    .strip_prefix(GUTTER)
                    .map_or_else(|| line.plain(), |rest| format!("{MARKER}{rest}")),
            )
        } else {
            line.painted(theme)
        });
    }
    lines.push(state.trouble.as_ref().map_or_else(
        || theme.muted(state::hint(state.searching, verb)),
        |trouble| theme.paint(Tone::Attention, trouble),
    ));
    lines
}

/// A line as ratatui spans.
fn paint(line: &Line) -> TextLine<'static> {
    TextLine::from(
        line.pieces
            .iter()
            .map(|piece| Span::styled(piece.text.clone(), Style::default().fg(tone(piece.tone))))
            .collect::<Vec<_>>(),
    )
}

/// Several lines as ratatui spans.
fn spans(lines: &[Line]) -> Vec<TextLine<'static>> {
    lines.iter().map(paint).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLUMNS: [Column; 3] = [
        Column::new("when", Role::Key),
        Column::new("what", Role::Kind),
        Column::new("detail", Role::Value),
    ];

    fn listing() -> Listing<'static> {
        Listing::new("Entries", "entries", &COLUMNS).rows(vec![
            Row::new("a", ["10:32 pm", "Diaper", "pee · big"].map(str::to_owned))
                .group("Sun 27 Sep")
                .tone(Tone::Diaper),
            Row::new("b", ["7:00 pm", "Bottle", "34 ml"].map(str::to_owned))
                .group("Sun 27 Sep")
                .tone(Tone::Feeding),
        ])
    }

    #[test]
    fn hosted_back_cancels_browsers_and_entry_pickers() {
        use crossterm::event::KeyCode;
        for choosing in [true, false] {
            for key in [
                KeyCode::Esc,
                KeyCode::Left,
                KeyCode::Char('h'),
                KeyCode::Char('a'),
                KeyCode::Char('q'),
            ] {
                let journey = hosted_navigation(choosing, &[key], false);
                assert!(
                    journey
                        .result
                        .as_ref()
                        .is_err_and(crate::prompt::is_cancelled),
                    "Back must bypass completion prompts: {:?}",
                    journey.result
                );
                assert_eq!(journey.frames.len(), 1);
                assert!(journey.output.is_empty());
            }
        }
    }

    #[test]
    fn enter_without_details_keeps_the_browse_selection_in_place() {
        use crossterm::event::KeyCode;
        let journey =
            hosted_navigation(false, &[KeyCode::Down, KeyCode::Enter, KeyCode::Esc], false);
        assert_eq!(
            journey.frames.len(),
            3,
            "Enter must keep the list open: {:?}",
            journey.frames
        );
        assert_eq!(
            journey.frames[1], journey.frames[2],
            "the selected row must not reset"
        );
        assert!(journey.output.is_empty(), "no empty detail receipt");
    }

    #[test]
    fn entry_pickers_can_select_rows_without_display_details() {
        use crossterm::event::KeyCode;
        let journey = hosted_navigation(true, &[KeyCode::Down, KeyCode::Enter], false);
        assert_eq!(journey.result.unwrap(), Some("b".to_owned()));
        assert_eq!(journey.frames.len(), 2);
    }

    #[test]
    fn empty_hosted_results_remain_displayed_instead_of_cancelling() {
        for choosing in [false, true] {
            let journey = hosted_navigation(choosing, &[], true);
            assert_eq!(journey.result.unwrap(), None);
            assert!(journey.frames.is_empty());
            assert!(journey.output.contains("no entries"));
        }
    }

    struct Journey {
        result: Result<Option<String>>,
        frames: Vec<String>,
        output: String,
    }

    fn hosted_navigation(
        choosing: bool,
        keys: &[crossterm::event::KeyCode],
        empty: bool,
    ) -> Journey {
        use crate::prompt::host;
        let _serial = host::one_at_a_time();
        let channel = host::install();
        let worker = std::thread::spawn(move || {
            let mut listing = listing();
            if empty {
                listing.rows.clear();
            }
            if choosing {
                listing.choose(Theme::dark(false))
            } else {
                listing.show(Theme::dark(false)).map(|()| None)
            }
        });
        let (frames, output) = drive_navigation(&channel, &worker, keys);
        let completed = worker.is_finished();
        host::remove();
        let result = worker.join().unwrap();
        assert!(
            completed,
            "the listing must finish before its host is removed"
        );
        Journey {
            result,
            frames,
            output,
        }
    }

    fn drive_navigation(
        channel: &crate::prompt::host::Channel,
        worker: &std::thread::JoinHandle<Result<Option<String>>>,
        keys: &[crossterm::event::KeyCode],
    ) -> (Vec<String>, String) {
        use crate::prompt::host::{Input, Reply, Request};
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut frames = Vec::new();
        let mut output = String::new();
        let started = std::time::Instant::now();
        while !worker.is_finished() && started.elapsed() < std::time::Duration::from_secs(5) {
            let Ok((request, reply)) = channel
                .requests
                .recv_timeout(std::time::Duration::from_millis(20))
            else {
                continue;
            };
            let answer = match request {
                Request::Frame(lines) => {
                    let key = keys.get(frames.len()).copied().unwrap_or(KeyCode::Esc);
                    frames.push(lines.join("\n"));
                    Reply::Input(Input::Key(KeyEvent::new(key, KeyModifiers::NONE)))
                }
                Request::Show(lines) => {
                    output.push_str(&lines.join("\n"));
                    Reply::Shown
                }
                Request::Step(_) => Reply::Stepped { interrupted: false },
            };
            reply.send(answer).unwrap();
        }
        (frames, output)
    }

    #[test]
    fn the_plain_rendering_is_the_title_the_count_the_headings_and_the_rows() {
        let lines = listing().lines(Theme::dark(false), None);
        let text = lines.join("\n");
        assert!(text.contains("Entries 2 entries"), "{text}");
        assert!(text.contains("when"), "the column headings: {text}");
        assert!(text.contains("Sun 27 Sep"), "the group heading: {text}");
        assert!(text.contains("pee · big"), "{text}");
    }

    #[test]
    fn an_empty_listing_says_so_in_its_own_words() {
        let listing = Listing::new("Entries", "entries", &COLUMNS).empty("nothing logged yet");
        assert_eq!(
            listing.lines(Theme::dark(false), None),
            vec!["nothing logged yet".to_owned()]
        );
    }

    #[test]
    fn a_filter_that_matches_nothing_says_so_rather_than_showing_an_empty_table() {
        let listing = listing().query(Some("badger"));
        let text = listing.lines(Theme::dark(false), None).join("\n");
        assert!(text.contains("nothing matches `badger`"), "{text}");
        assert!(text.contains("0 of 2"), "{text}");
    }

    #[test]
    fn a_filter_narrows_the_rows_and_the_count_says_so() {
        let listing = listing().query(Some("diaper"));
        let text = listing.lines(Theme::dark(false), None).join("\n");
        assert!(text.contains("1 of 2"), "{text}");
        assert!(text.contains("pee · big"), "{text}");
        assert!(!text.contains("34 ml"), "{text}");
    }

    #[test]
    fn a_group_heading_is_rendered_from_its_key() {
        let listing = listing().group_heading(|key| format!("— {key} —"));
        let text = listing.lines(Theme::dark(false), None).join("\n");
        assert!(text.contains("— Sun 27 Sep —"), "{text}");
    }
}
