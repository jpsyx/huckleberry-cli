//! Owned list state uses the existing filter, layout and renderer.
use super::{ControlEvent, FormValue, view::navigation};
use crate::{
    listing::{self, Column, Flow, Row, State},
    prompt::host::Input,
    theme::Theme,
};
use anyhow::{Result, anyhow, bail};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
/// A searchable picker retaining its query, scroll and semantic selection.
pub struct ListingControl {
    title: String,
    columns: &'static [Column],
    rows: Vec<Row>,
    state: State,
    missing: Option<String>,
}
impl ListingControl {
    /// Creates a picker on a known token when one is supplied.
    #[must_use]
    pub fn create_listing(
        title: impl Into<String>,
        columns: &'static [Column],
        rows: Vec<Row>,
        selected: Option<&str>,
    ) -> Self {
        let cursor = selected
            .and_then(|token| rows.iter().position(|row| row.key == token))
            .unwrap_or_default();
        Self {
            title: title.into(),
            columns,
            rows,
            state: State {
                cursor,
                ..State::default()
            },
            missing: None,
        }
    }
    /// Refreshes rows by token; a disappeared row requires another deliberate selection.
    pub fn refresh_rows(&mut self, rows: Vec<Row>) {
        let token = self.value().ok().and_then(|value| match value {
            FormValue::Picked(token) => Some(token),
            _ => None,
        });
        self.rows = rows;
        if let Some(token) = token {
            match listing::model::filter(&self.rows, &self.state.query)
                .iter()
                .position(|row| row.key == token)
            {
                Some(cursor) => self.state.cursor = cursor,
                None => self.missing = Some(token),
            }
        }
    }
    pub(super) fn refresh(&mut self, fresh: Self) {
        self.title = fresh.title;
        self.refresh_rows(fresh.rows);
    }
    /// Reads the exact selected row identity without silently substituting its neighbor.
    pub fn value(&self) -> Result<FormValue> {
        if self.missing.is_some() {
            bail!("That entry is no longer available. Choose another entry.");
        }
        let filtered = listing::model::filter(&self.rows, &self.state.query);
        let row = filtered
            .get(self.state.cursor)
            .ok_or_else(|| anyhow!("choose an entry"))?;
        if !row.selectable {
            bail!(
                row.refusal
                    .clone()
                    .unwrap_or_else(|| "that entry cannot be selected".into())
            );
        }
        Ok(FormValue::Picked(row.key.clone()))
    }
    /// Applies the existing list motions with distinct form navigation.
    pub fn apply(&mut self, input: Input, size: (u16, u16)) -> ControlEvent {
        let key = match input {
            Input::Key(key) => key,
            Input::Pasted(text) => {
                for character in text.chars().filter(|character| !character.is_control()) {
                    self.apply(
                        Input::Key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE)),
                        size,
                    );
                }
                return ControlEvent::Stay;
            }
        };
        if let Some(event) = navigation(key, !self.state.searching, !self.state.searching) {
            return event;
        }
        let count = listing::model::filter(&self.rows, &self.state.query).len();
        let mapped = listing::state::key_for(key);
        if !matches!(mapped, listing::Key::Enter | listing::Key::Ignore) {
            self.missing = None;
        }
        let flow = listing::state::apply(&mut self.state, mapped, count, usize::from(size.1) / 2);
        let mut state = self.state.clone();
        self.frame(size, &mut state);
        self.state = state;
        match flow {
            Flow::Stay => ControlEvent::Stay,
            Flow::Quit => ControlEvent::Back,
            Flow::Choose => ControlEvent::Accept,
        }
    }
    /// Draws the shared listing surface using the retained state.
    #[must_use]
    pub fn render(&self, size: (u16, u16), theme: Theme) -> Vec<String> {
        let mut state = self.state.clone();
        let (head, body, height) = self.frame(size, &mut state);
        if self.missing.is_some() {
            state.trouble = Some("That entry is no longer available. Choose another entry.".into());
        }
        listing::rendered(
            &head,
            &body,
            &state,
            "↑/↓ j/k w/s move · / searches · Enter chooses · ← h/a back · Esc home",
            height,
            theme,
        )
        .into_iter()
        .map(|line| line.replace("esc clears", "← leaves filter · Esc home"))
        .collect()
    }
    fn frame(
        &self,
        size: (u16, u16),
        state: &mut State,
    ) -> (Vec<listing::Line>, Vec<listing::Line>, usize) {
        let listing =
            listing::Listing::new(&self.title, "entries", self.columns).rows(self.rows.clone());
        let width = usize::from(size.0).saturating_sub(self.rows.len().to_string().len() + 2);
        let (head, mut body) = listing.frame(&state.query, state.searching, Some(width));
        listing::layout::number_rows(&mut body);
        let height = usize::from(size.1).saturating_sub(head.len() + 1).max(1);
        let line = body
            .iter()
            .position(|line| line.row == Some(state.cursor))
            .unwrap_or(0);
        state.top = listing::state::scrolled(state.top, line, height, body.len());
        (head, body, height)
    }
}
