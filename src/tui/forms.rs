//! A single draft lives across backward and forward navigation.
use crate::{
    commands::prepared::{CommandProgress, Execution, PreparedCommand},
    interactive::catalog::CommandPath,
    prompt::{flow::FormEvent, host::Input},
    theme::Theme,
};
use crossterm::event::{KeyCode, KeyEventKind};

/// Collection is pure; execution owns a consumed submission until its result arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Editing an owned draft.
    Collecting,
    /// At the launching menu, retaining the draft without a worker.
    Suspended,
    /// Validated input is ready to consume.
    Ready,
    /// The submission has been consumed by a worker.
    Executing,
    /// The command ended or was abandoned.
    Result,
}

/// Owns the active or suspended draft and its command identity.
pub struct Forms {
    path: CommandPath,
    prepared: Option<PreparedCommand>,
    phase: Phase,
    home: bool,
}

impl Forms {
    /// Begins collecting a prepared command without starting its executor.
    #[must_use]
    pub fn create_forms(path: CommandPath, prepared: PreparedCommand) -> Self {
        let phase = if prepared.is_ready() {
            Phase::Ready
        } else {
            Phase::Collecting
        };
        Self {
            path,
            prepared: Some(prepared),
            phase,
            home: false,
        }
    }
    /// The current lifecycle boundary.
    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.phase
    }
    /// Whether the final landing belongs at Home.
    #[must_use]
    pub const fn go_home(&self) -> bool {
        self.home
    }
    /// The command whose preparation produced this draft.
    #[must_use]
    pub const fn path(&self) -> &CommandPath {
        &self.path
    }

    /// Reopens only the same command; choosing another discards the old draft.
    pub fn resume(&mut self, path: &CommandPath) -> bool {
        if self.phase == Phase::Suspended && &self.path == path {
            self.phase = Phase::Collecting;
            true
        } else {
            self.discard(false);
            false
        }
    }

    /// Handles input while keeping executing writes alive through Escape.
    pub fn apply(&mut self, input: Input, now: f64, size: (u16, u16)) -> FormEvent {
        if self.phase == Phase::Executing {
            self.home |= matches!(input, Input::Key(key) if key.kind == KeyEventKind::Press && key.code == KeyCode::Esc);
            return FormEvent::Stay;
        }
        if self.phase != Phase::Collecting {
            return FormEvent::Stay;
        }
        let event = self
            .prepared
            .as_mut()
            .expect("collecting draft")
            .apply(input, now, size);
        match event {
            FormEvent::Stay => {}
            FormEvent::Suspend => self.phase = Phase::Suspended,
            FormEvent::Home => self.discard(true),
            FormEvent::Cancel => self.discard(false),
            FormEvent::Ready => self.phase = Phase::Ready,
        }
        event
    }

    /// Consumes a ready draft once; repeated input cannot produce another execution.
    pub fn take_execution(&mut self) -> Option<Execution> {
        if self.phase != Phase::Ready {
            return None;
        }
        self.phase = Phase::Executing;
        self.prepared.take().map(PreparedCommand::execute)
    }

    /// Records the actual result, even when Home was requested during execution.
    pub fn complete(&mut self, progress: CommandProgress) {
        match progress {
            CommandProgress::Continue(prepared) if !self.home => {
                self.phase = if prepared.is_ready() {
                    Phase::Ready
                } else {
                    Phase::Collecting
                };
                self.prepared = Some(*prepared);
            }
            CommandProgress::Continue(_) => self.discard(true),
            CommandProgress::Finished { home } => self.discard(self.home || home),
        }
    }

    /// Draws the retained form; suspended and completed drafts own no reply channel.
    #[must_use]
    pub fn render(&self, now: f64, size: (u16, u16), theme: Theme) -> Vec<String> {
        self.prepared
            .as_ref()
            .map_or_else(Vec::new, |form| form.render(now, size, theme))
    }

    fn discard(&mut self, home: bool) {
        self.prepared = None;
        self.phase = Phase::Result;
        self.home = home;
    }
}
