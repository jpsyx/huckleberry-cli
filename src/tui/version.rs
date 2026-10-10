//! Pure Version modal state and one-handed input decisions.
use super::Motion;
use crate::version::{UpdateStatus, VersionReport};

/// Progress shown by the modal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckState {
    /// A background request is in flight.
    Checking,
    /// The check has an answer, including offline and unavailable outcomes.
    Complete(VersionReport),
}

/// What a modal key asks the shell to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Keep displaying this modal.
    Stay,
    /// Close without changing the menu underneath.
    Close,
    /// Start a fresh check.
    Check,
    /// Leave the whole shell.
    Quit,
}

/// The modal's content, without task or terminal ownership.
pub struct State {
    /// Current request state or completed answer.
    pub check: CheckState,
}

impl State {
    /// Open immediately, without needing a network result to draw.
    #[must_use]
    pub fn new(offline: bool) -> Self {
        Self {
            check: if offline {
                CheckState::Complete(VersionReport::offline(env!("CARGO_PKG_VERSION")))
            } else {
                CheckState::Checking
            },
        }
    }

    /// Consume every modal motion so none reaches the underlying menu.
    pub fn apply(&mut self, motion: Motion) -> Action {
        match motion {
            Motion::Quit => Action::Quit,
            Motion::Open | Motion::Back | Motion::Cancel => Action::Close,
            Motion::Refresh => {
                if matches!(&self.check, CheckState::Complete(report) if report.status != UpdateStatus::Offline)
                {
                    self.check = CheckState::Checking;
                    Action::Check
                } else {
                    Action::Stay
                }
            }
            _ => Action::Stay,
        }
    }
}
