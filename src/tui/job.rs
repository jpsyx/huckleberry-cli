//! A command running inside the shell.
//!
//! The command runs on its own task and talks to the loop through
//! [`crate::prompt::host`]: it hands over lines to draw and waits for the key
//! that answers them. The loop keeps drawing the rest of the screen the whole
//! time, which is what makes a menu selection something that happens *in* the
//! shell rather than instead of it.

use std::sync::mpsc::{SyncSender, TryRecvError};

use anyhow::Result;
use crossterm::event::KeyEvent;
use tokio::task::JoinHandle;

use crate::interactive::catalog::CommandPath;
use crate::interactive::session::SessionOptions;
use crate::prompt::host::{self, Channel, Reply, Request};
use crate::theme::Theme;

/// What became of a command, once its output has been read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Landing {
    /// A recording finished: start again from the top.
    Home,
    /// A view or a utility finished: stay where it was opened from.
    Stay,
    /// Escape, or a question abandoned.
    Cancelled,
    /// It failed, and the failure has already been read.
    Failed,
}

/// Whether this command draws a full screen of its own.
///
/// `dash` is the one. It is a second full-screen program rather than a command
/// with questions, and a terminal has one alternate screen to give, so the
/// shell steps aside for it and takes the screen back when it ends. Everything
/// else runs in the panel where the menu was.
///
/// This is the last thing in the tool that suspends the shell, and it is on
/// its way out: the shell already shows what the dashboard's first tab does,
/// and the rest of its tabs are commands.
#[must_use]
pub fn takes_the_screen(path: &CommandPath) -> bool {
    path.0.first().is_some_and(|word| word == "dash")
}

/// What to draw, given what a command has said and how tall the panel is.
///
/// The foot of a long flow is where the question is, so a panel too short to
/// hold everything keeps the end rather than the beginning: what is being
/// asked matters more than what was answered three questions ago.
#[must_use]
pub fn window(output: &[String], question: Option<&[String]>, height: usize) -> Vec<String> {
    let mut lines = output.to_vec();
    match question {
        Some(question) => {
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.extend_from_slice(question);
        }
        None if lines.is_empty() => lines.push(String::new()),
        None => {}
    }
    let from = lines.len().saturating_sub(height.max(1));
    lines.split_off(from)
}

/// A command, and everything it has said so far.
pub struct Job {
    title: String,
    handle: JoinHandle<Landing>,
    channel: Channel,
    /// The lines a question is waiting on, and who to send the key back to.
    waiting: Option<(Vec<String>, SyncSender<Reply>)>,
    /// Everything the command has printed.
    output: Vec<String>,
}

impl Job {
    /// Starts a command, with the shell hosting its questions.
    #[must_use]
    pub fn start(path: &CommandPath, globals: &SessionOptions, theme: Theme) -> Self {
        let channel = host::install();
        let title = crate::render::output::words(&path.0.join(" "));
        let (path, globals) = (path.clone(), globals.clone());
        let handle = tokio::spawn(async move {
            settle(
                Box::pin(crate::interactive::operation::run(&path, &globals, theme)).await,
                theme,
            )
        });
        Self {
            title,
            handle,
            channel,
            waiting: None,
            output: Vec::new(),
        }
    }

    /// A job that is not running anything, for drawing a panel in a test.
    #[cfg(test)]
    #[must_use]
    pub fn idle(title: &str, output: &[String]) -> Self {
        let channel = host::install();
        host::remove();
        Self {
            title: title.to_owned(),
            // A handle over a task that has nothing to do: this exists to
            // draw a panel, not to run anything.
            handle: tokio::runtime::Builder::new_current_thread()
                .build()
                .expect("a runtime")
                .spawn(std::future::ready(Landing::Stay)),
            channel,
            waiting: None,
            output: output.to_vec(),
        }
    }

    /// Command help, which is a flow like any other.
    #[must_use]
    pub fn help(theme: Theme) -> Self {
        let channel = host::install();
        let handle =
            tokio::spawn(
                async move { settle(crate::interactive::help(theme).map(|()| false), theme) },
            );
        Self {
            title: "Help".to_owned(),
            handle,
            channel,
            waiting: None,
            output: Vec::new(),
        }
    }

    /// What the panel calls itself.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Takes whatever the command has said since the last look.
    ///
    /// Output is answered at once so the command carries on; a question stops
    /// the collecting, because it is waiting on a key that has not been
    /// pressed yet.
    pub fn collect(&mut self) {
        while self.waiting.is_none() {
            match self.channel.requests.try_recv() {
                Ok((Request::Show(lines), reply)) => {
                    self.output.extend(lines);
                    let _ = reply.send(Reply::Shown);
                }
                Ok((Request::Frame(lines), reply)) => self.waiting = Some((lines, reply)),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return,
            }
        }
    }

    /// Whether a question is on the screen waiting to be answered.
    #[must_use]
    pub const fn asking(&self) -> bool {
        self.waiting.is_some()
    }

    /// Answers the question on the screen.
    pub fn answer(&mut self, key: KeyEvent) {
        if let Some((_, reply)) = self.waiting.take() {
            let _ = reply.send(Reply::Key(key));
        }
    }

    /// The lines to draw, newest output last and the question at the foot.
    #[must_use]
    pub fn visible(&self, height: usize) -> Vec<String> {
        window(
            &self.output,
            self.waiting.as_ref().map(|(lines, _)| lines.as_slice()),
            height,
        )
    }

    /// Whether the command has run to the end.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.handle.is_finished()
    }

    /// Takes the outcome, and gives the prompts back to the terminal.
    pub async fn finish(self) -> Landing {
        host::remove();
        self.handle.await.unwrap_or(Landing::Failed)
    }
}

/// Reports how a command ended, in the panel it ran in.
///
/// The host is still installed, so the failure and the pause that holds it
/// draw where everything else did.
#[must_use]
pub fn settle(result: Result<bool>, theme: Theme) -> Landing {
    match result {
        Ok(true) => Landing::Home,
        Ok(false) => Landing::Stay,
        Err(error) if crate::prompt::is_cancelled(&error) => Landing::Cancelled,
        Err(error) => {
            crate::render::note(&theme.error_line("error:", &format!("{error:#}")));
            // A failed write is never replayed on its own; this only holds the
            // message on screen until it has been read.
            let _ = crate::interactive::pause(theme);
            Landing::Failed
        }
    }
}

#[cfg(test)]
mod flows {
    use super::*;

    fn lines(texts: &[&str]) -> Vec<String> {
        texts.iter().map(|text| (*text).to_owned()).collect()
    }

    #[test]
    fn the_question_sits_under_whatever_the_command_has_printed() {
        let shown = window(
            &lines(&["a receipt"]),
            Some(&lines(&["Ready?", "> 1. Continue"])),
            20,
        );
        assert_eq!(shown, ["a receipt", "", "Ready?", "> 1. Continue"]);
    }

    #[test]
    fn a_panel_too_short_keeps_the_question_and_drops_the_oldest_output() {
        let output = lines(&["one", "two", "three", "four"]);
        let shown = window(&output, Some(&lines(&["Ready?"])), 3);
        assert_eq!(
            shown,
            ["four", "", "Ready?"],
            "what is being asked matters more than what was answered three \
             questions ago"
        );
    }

    #[test]
    fn a_command_with_nothing_to_say_yet_still_draws_a_panel() {
        assert_eq!(window(&[], None, 10), [""]);
    }

    #[test]
    fn output_with_no_question_needs_no_gap_after_it() {
        assert_eq!(window(&lines(&["done"]), None, 10), ["done"]);
    }

    #[test]
    fn the_dashboard_is_the_only_command_that_takes_the_screen() {
        assert!(takes_the_screen(&CommandPath(vec!["dash".into()])));
        for words in [
            vec!["now".to_owned()],
            vec!["diaper".to_owned()],
            vec!["log".to_owned()],
            vec!["feed".to_owned(), "bottle".to_owned()],
            vec!["stripes".to_owned()],
        ] {
            assert!(
                !takes_the_screen(&CommandPath(words.clone())),
                "{words:?} runs in the panel where the menu was"
            );
        }
    }
}
