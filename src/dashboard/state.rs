//! What the dashboard is showing, and what a keystroke does to it.
//!
//! Everything in this module is pure. The event loop in
//! [`crate::commands::dash`] owns the terminal and the network; this owns the
//! decisions, which is what makes "does `tab` wrap around at the last tab"
//! a test rather than something you find out by pressing it.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::domain::Calendar;
use crate::domain::types::Dataset;

/// Which screen the dashboard is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// The 3am facts and the live timers.
    Now,
    /// The stripe chart.
    Sleep,
    /// Milk, by day.
    Feeding,
    /// Diapers, by day.
    Diapers,
    /// Everything, newest first.
    Log,
}

impl Tab {
    /// Every tab, in the order they appear.
    pub const ALL: [Self; 5] = [
        Self::Now,
        Self::Sleep,
        Self::Feeding,
        Self::Diapers,
        Self::Log,
    ];

    /// The label on the tab bar.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Now => "Now",
            Self::Sleep => "Sleep",
            Self::Feeding => "Feeding",
            Self::Diapers => "Diapers",
            Self::Log => "Log",
        }
    }

    /// Where this tab sits on the bar.
    #[must_use]
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|tab| *tab == self).unwrap_or(0)
    }

    /// The next tab, wrapping at the end. Wrapping rather than stopping,
    /// because a person holding tab expects to cycle, not to get stuck.
    #[must_use]
    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    /// The previous tab, wrapping at the start.
    #[must_use]
    pub fn previous(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    /// The tab a number key selects, counting from one.
    #[must_use]
    pub fn from_digit(digit: char) -> Option<Self> {
        digit
            .to_digit(10)
            .and_then(|number| number.checked_sub(1))
            .and_then(|index| Self::ALL.get(index as usize).copied())
    }
}

/// What a keystroke asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Nothing this loop understands.
    Ignore,
    /// Leave.
    Quit,
    /// Re-read from Huckleberry now.
    Refresh,
    /// Go to a tab.
    Show(Tab),
    /// Scroll the log.
    ScrollDown,
    /// Scroll the log back.
    ScrollUp,
}

/// What a keystroke means.
///
/// `q`, `Esc` and `Ctrl-C` all leave: a full-screen program that traps a
/// person's terminal because they guessed the wrong key is a bad program.
#[must_use]
pub fn action_for(key: KeyEvent) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Action::Quit;
    }
    match key.code {
        KeyCode::Char('q' | 'Q') | KeyCode::Esc => Action::Quit,
        KeyCode::Char('r' | 'R') => Action::Refresh,
        KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => Action::Show(Tab::Now.next()),
        KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => Action::Show(Tab::Now.previous()),
        KeyCode::Down | KeyCode::Char('j') => Action::ScrollDown,
        KeyCode::Up | KeyCode::Char('k') => Action::ScrollUp,
        KeyCode::Char(digit @ '1'..='9') => {
            Tab::from_digit(digit).map_or(Action::Ignore, Action::Show)
        }
        _ => Action::Ignore,
    }
}

/// Everything the dashboard is holding.
pub struct State {
    /// What was read.
    pub dataset: Dataset,
    /// The calendar its days are counted in.
    pub calendar: Calendar,
    /// Which screen is showing.
    pub tab: Tab,
    /// How far down the log is scrolled.
    pub scroll: usize,
    /// How many days the charts cover.
    pub days: usize,
    /// The last failure, kept so a dropped connection is visible without
    /// taking the screen away.
    pub trouble: Option<String>,
    /// Whether a read is in flight.
    pub refreshing: bool,
}

impl State {
    /// A dashboard showing this dataset.
    #[must_use]
    pub const fn new(dataset: Dataset, calendar: Calendar, days: usize) -> Self {
        Self {
            dataset,
            calendar,
            tab: Tab::Now,
            scroll: 0,
            days,
            trouble: None,
            refreshing: false,
        }
    }

    /// Applies a keystroke. Returns `false` when it is time to leave.
    ///
    /// Relative movement is resolved here rather than in [`action_for`],
    /// which cannot know which tab is showing.
    pub fn apply(&mut self, key: KeyEvent) -> bool {
        match action_for(key) {
            Action::Quit => return false,
            Action::Refresh => self.refreshing = true,
            Action::ScrollDown => self.scroll = self.scroll.saturating_add(1),
            Action::ScrollUp => self.scroll = self.scroll.saturating_sub(1),
            Action::Show(_) => self.move_to(key),
            Action::Ignore => {}
        }
        true
    }

    /// Works out which tab a movement key meant.
    fn move_to(&mut self, key: KeyEvent) {
        self.tab = match key.code {
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => self.tab.next(),
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => self.tab.previous(),
            KeyCode::Char(digit) => Tab::from_digit(digit).unwrap_or(self.tab),
            _ => self.tab,
        };
        // A tab change starts at the top: carrying a scroll offset from the
        // log onto another screen looks like a bug.
        self.scroll = 0;
    }

    /// Takes on a fresh read.
    pub fn replace(&mut self, dataset: Dataset) {
        self.dataset = dataset;
        self.trouble = None;
        self.refreshing = false;
    }

    /// Records that a read failed, keeping the data that is already on screen.
    pub fn record_trouble(&mut self, problem: &str) {
        self.trouble = Some(problem.to_owned());
        self.refreshing = false;
    }
}

#[cfg(test)]
mod tabs {
    use super::*;

    #[test]
    fn tabbing_past_the_last_tab_wraps_to_the_first() {
        assert_eq!(Tab::Log.next(), Tab::Now);
        assert_eq!(Tab::Now.previous(), Tab::Log);
    }

    #[test]
    fn a_number_key_selects_the_tab_at_that_position() {
        assert_eq!(Tab::from_digit('1'), Some(Tab::Now));
        assert_eq!(Tab::from_digit('5'), Some(Tab::Log));
        assert_eq!(Tab::from_digit('6'), None);
        assert_eq!(Tab::from_digit('0'), None);
    }

    #[test]
    fn every_tab_has_a_title_and_knows_where_it_sits() {
        for (position, tab) in Tab::ALL.into_iter().enumerate() {
            assert!(!tab.title().is_empty());
            assert_eq!(tab.index(), position);
        }
    }
}

#[cfg(test)]
mod keys {
    use super::*;
    use crate::domain::fixtures::dataset;

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn state() -> State {
        State::new(
            dataset(),
            Calendar::new("America/New_York").expect("a real timezone"),
            7,
        )
    }

    #[test]
    fn three_different_keys_all_leave_so_nobody_is_trapped() {
        for code in [KeyCode::Char('q'), KeyCode::Esc] {
            assert_eq!(action_for(press(code)), Action::Quit, "{code:?}");
        }
        assert_eq!(
            action_for(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Action::Quit
        );
    }

    #[test]
    fn a_quit_key_ends_the_loop() {
        assert!(!state().apply(press(KeyCode::Char('q'))));
        assert!(state().apply(press(KeyCode::Char('r'))));
    }

    #[test]
    fn tab_moves_forward_and_shift_tab_moves_back() {
        let mut state = state();
        state.apply(press(KeyCode::Tab));
        assert_eq!(state.tab, Tab::Sleep);
        state.apply(press(KeyCode::BackTab));
        assert_eq!(state.tab, Tab::Now);
    }

    #[test]
    fn the_vim_keys_move_the_same_way_as_the_arrows() {
        let mut state = state();
        state.apply(press(KeyCode::Char('l')));
        assert_eq!(state.tab, Tab::Sleep);
        state.apply(press(KeyCode::Char('h')));
        assert_eq!(state.tab, Tab::Now);
    }

    #[test]
    fn a_number_key_jumps_straight_to_its_tab() {
        let mut state = state();
        state.apply(press(KeyCode::Char('5')));
        assert_eq!(state.tab, Tab::Log);
    }

    #[test]
    fn changing_tab_scrolls_back_to_the_top() {
        let mut state = state();
        state.apply(press(KeyCode::Char('5')));
        state.apply(press(KeyCode::Down));
        state.apply(press(KeyCode::Down));
        assert_eq!(state.scroll, 2);
        state.apply(press(KeyCode::Char('1')));
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn scrolling_up_at_the_top_stays_at_the_top() {
        let mut state = state();
        state.apply(press(KeyCode::Up));
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn a_key_the_dashboard_does_not_use_changes_nothing() {
        let mut state = state();
        state.apply(press(KeyCode::Char('z')));
        assert_eq!(state.tab, Tab::Now);
        assert_eq!(state.scroll, 0);
        assert!(!state.refreshing);
    }
}

#[cfg(test)]
mod reads {
    use super::*;
    use crate::domain::fixtures::{AFTERNOON, bottle, dataset};

    fn state() -> State {
        State::new(
            dataset(),
            Calendar::new("America/New_York").expect("a real timezone"),
            7,
        )
    }

    #[test]
    fn a_failed_read_leaves_what_is_on_screen_alone() {
        let mut state = state();
        state.dataset.feeds = vec![bottle(AFTERNOON, 90.0)];
        state.record_trouble("could not reach Huckleberry");
        assert_eq!(state.dataset.feeds.len(), 1, "the data is still there");
        assert!(state.trouble.is_some());
        assert!(!state.refreshing);
    }

    #[test]
    fn a_successful_read_clears_the_last_failure() {
        let mut state = state();
        state.record_trouble("could not reach Huckleberry");
        state.replace(dataset());
        assert!(state.trouble.is_none());
    }
}
