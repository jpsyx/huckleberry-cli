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
/// WASD aliases preserve existing control chords.
#[must_use]
pub fn action_for(key: KeyEvent) -> Action {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('c') => return Action::Quit,
            KeyCode::Char('w' | 'W' | 'a' | 'A' | 's' | 'S' | 'd' | 'D') => {
                return Action::Ignore;
            }
            _ => {}
        }
    }
    match key.code {
        KeyCode::Char('q' | 'Q') | KeyCode::Esc => Action::Quit,
        KeyCode::Char('r' | 'R') => Action::Refresh,
        KeyCode::Tab | KeyCode::Right | KeyCode::Char('l' | 'L' | 'd' | 'D') => {
            Action::Show(Tab::Now.next())
        }
        KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h' | 'H' | 'a' | 'A') => {
            Action::Show(Tab::Now.previous())
        }
        KeyCode::Down | KeyCode::Char('j' | 'J' | 's' | 'S') => Action::ScrollDown,
        KeyCode::Up | KeyCode::Char('k' | 'K' | 'w' | 'W') => Action::ScrollUp,
        KeyCode::Char(digit @ '1'..='9') => {
            Tab::from_digit(digit).map_or(Action::Ignore, Action::Show)
        }
        _ => Action::Ignore,
    }
}

/// What a keystroke means on a given tab.
///
/// The same thing on every one of them. The Log tab used to read `h` as down
/// so a long list could be scrolled with it, but the dashboard is the one
/// screen here with a left and a right, and a letter that means left on four
/// tabs and down on the fifth is a letter nobody can press without looking.
/// `j`/`s` and `k`/`w` scroll, everywhere; `a`/`d` match `h`/`l`.
#[must_use]
pub fn action_for_tab(key: KeyEvent, _tab: Tab) -> Action {
    action_for(key)
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
    /// How this family counts a day.
    pub rule: crate::domain::today::DayRule,
    /// Whether it is being drawn inside the shell rather than on its own.
    ///
    /// The shell's footer already names the keys, and they are not the same
    /// keys: `q` does not leave in there. So the status bar keeps the child
    /// and the staleness and drops the hints.
    pub in_panel: bool,
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
            rule: crate::domain::today::DayRule::assumed(),
            in_panel: false,
        }
    }

    /// Drawn inside the shell, where the keys belong to the shell.
    ///
    /// The Now tab goes with it: the shell's own Now drawer is already on the
    /// screen above, and the same facts twice is one of them wasted.
    #[must_use]
    pub const fn in_panel(mut self) -> Self {
        self.in_panel = true;
        self.tab = Tab::Sleep;
        self
    }

    /// The tabs this dashboard has, in the order they appear.
    #[must_use]
    pub const fn tabs(&self) -> &'static [Tab] {
        if self.in_panel {
            // Everything but Now, which is the first.
            Tab::ALL.split_first().expect("five tabs").1
        } else {
            &Tab::ALL
        }
    }

    /// Where the tab showing sits on the bar.
    #[must_use]
    pub fn tab_index(&self) -> usize {
        self.tabs()
            .iter()
            .position(|tab| *tab == self.tab)
            .unwrap_or(0)
    }

    /// The tab a number key selects, counting from one.
    fn tab_from_digit(&self, digit: char) -> Option<Tab> {
        let index = digit.to_digit(10)?.checked_sub(1)? as usize;
        self.tabs().get(index).copied()
    }

    /// The next tab, wrapping at the end of the ones there are.
    fn next_tab(&self) -> Tab {
        let tabs = self.tabs();
        tabs[(self.tab_index() + 1) % tabs.len()]
    }

    /// The previous tab, wrapping at the start.
    fn previous_tab(&self) -> Tab {
        let tabs = self.tabs();
        tabs[(self.tab_index() + tabs.len() - 1) % tabs.len()]
    }

    /// Counts days the way the family configured.
    #[must_use]
    pub const fn with_rule(mut self, rule: crate::domain::today::DayRule) -> Self {
        self.rule = rule;
        self
    }

    /// Applies a keystroke. Returns `false` when it is time to leave.
    ///
    /// Relative movement is resolved here rather than in [`action_for`],
    /// which cannot know which tab is showing.
    pub fn apply(&mut self, key: KeyEvent) -> bool {
        match action_for_tab(key, self.tab) {
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
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l' | 'L' | 'd' | 'D') => self.next_tab(),
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h' | 'H' | 'a' | 'A') => {
                self.previous_tab()
            }
            KeyCode::Char(digit) => self.tab_from_digit(digit).unwrap_or(self.tab),
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

#[cfg(test)]
mod panels {
    //! Inside the shell the Now drawer is already on the screen, so the
    //! dashboard's own Now tab would be the same facts twice.

    use super::*;
    use crate::domain::Calendar;
    use crate::domain::fixtures::dataset;

    fn state() -> State {
        State::new(
            dataset(),
            Calendar::new("America/New_York").expect("a real timezone"),
            7,
        )
    }

    fn press(character: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE)
    }

    #[test]
    fn on_its_own_the_dashboard_shows_every_tab() {
        assert_eq!(state().tabs(), Tab::ALL);
        assert_eq!(state().tab, Tab::Now);
    }

    #[test]
    fn inside_the_shell_the_now_tab_is_left_out() {
        let state = state().in_panel();
        assert_eq!(
            state.tabs(),
            [Tab::Sleep, Tab::Feeding, Tab::Diapers, Tab::Log],
            "the Now drawer is already showing those facts"
        );
        assert_eq!(state.tab, Tab::Sleep, "so it opens on the first one left");
    }

    #[test]
    fn the_numbers_count_the_tabs_that_are_there() {
        let mut state = state().in_panel();
        for (digit, expected) in [
            ('1', Tab::Sleep),
            ('2', Tab::Feeding),
            ('3', Tab::Diapers),
            ('4', Tab::Log),
        ] {
            state.apply(press(digit));
            assert_eq!(state.tab, expected, "{digit}");
        }
        state.apply(press('5'));
        assert_eq!(state.tab, Tab::Log, "there is no fifth tab to go to");
    }

    #[test]
    fn moving_wraps_around_the_tabs_that_are_there() {
        let mut state = state().in_panel();
        assert_eq!(state.tab, Tab::Sleep);
        state.apply(press('h'));
        assert_eq!(state.tab, Tab::Log, "back from the first is the last");
        state.apply(press('l'));
        assert_eq!(state.tab, Tab::Sleep, "and on from the last is the first");
    }

    #[test]
    fn w_and_s_scroll_on_every_dashboard_tab() {
        for in_panel in [false, true] {
            for tab in Tab::ALL {
                for (letter, expected) in [('s', 3), ('S', 3), ('w', 1), ('W', 1)] {
                    let mut state = state();
                    state.in_panel = in_panel;
                    state.tab = tab;
                    state.scroll = 2;
                    assert!(state.apply(press(letter)));
                    assert_eq!(state.tab, tab);
                    assert_eq!(state.scroll, expected, "{letter} on {tab:?}");
                }
            }
        }
    }

    #[test]
    fn control_wasd_does_not_navigate_the_dashboard() {
        for in_panel in [false, true] {
            for letter in "wasdWASD".chars() {
                let mut state = state();
                state.in_panel = in_panel;
                state.tab = Tab::Feeding;
                state.scroll = 2;
                assert!(state.apply(KeyEvent::new(KeyCode::Char(letter), KeyModifiers::CONTROL)));
                assert_eq!(state.tab, Tab::Feeding, "ctrl-{letter}");
                assert_eq!(state.scroll, 2, "ctrl-{letter}");
            }
        }
    }

    #[test]
    fn a_and_d_move_relative_to_the_current_dashboard_tab() {
        for (in_panel, tab, previous, following) in [
            (false, Tab::Now, Tab::Log, Tab::Sleep),
            (false, Tab::Sleep, Tab::Now, Tab::Feeding),
            (false, Tab::Feeding, Tab::Sleep, Tab::Diapers),
            (false, Tab::Diapers, Tab::Feeding, Tab::Log),
            (false, Tab::Log, Tab::Diapers, Tab::Now),
            (true, Tab::Sleep, Tab::Log, Tab::Feeding),
            (true, Tab::Feeding, Tab::Sleep, Tab::Diapers),
            (true, Tab::Diapers, Tab::Feeding, Tab::Log),
            (true, Tab::Log, Tab::Diapers, Tab::Sleep),
        ] {
            for (letter, expected) in [
                ('a', previous),
                ('A', previous),
                ('d', following),
                ('D', following),
            ] {
                let mut state = state();
                state.in_panel = in_panel;
                state.tab = tab;
                state.scroll = 2;
                assert!(state.apply(press(letter)));
                assert_eq!(state.tab, expected, "{letter} on {tab:?}");
                assert_eq!(state.scroll, 0, "changing tabs resets scrolling");
            }
        }
    }

    #[test]
    fn the_tab_bar_names_only_the_tabs_that_are_there() {
        let drawn = crate::dashboard::draw::tab_titles(&state().in_panel());
        assert!(!drawn.contains(&"Now"), "{drawn:?}");
        assert_eq!(drawn, ["Sleep", "Feeding", "Diapers", "Log"]);
    }
}
