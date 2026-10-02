//! A command running inside the shell.
//!
//! The command runs on its own task and talks to the loop through
//! [`crate::prompt::host`]: it hands over lines to draw and waits for the key
//! that answers them. The loop keeps drawing the rest of the screen the whole
//! time, which is what makes a menu selection something that happens *in* the
//! shell rather than instead of it.

use std::collections::VecDeque;
use std::sync::mpsc::{SyncSender, TryRecvError};

use anyhow::Result;
use tokio::task::JoinHandle;

use crate::interactive::catalog::CommandPath;
use crate::interactive::session::SessionOptions;
use crate::prompt::host::{self, Channel, Input, Reply, Request};
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

/// What is on the screen, and what has been typed at it.
///
/// Two things it exists to get right, both found by typing into a question:
///
/// - **Nothing typed is ever dropped.** There is a window after every
///   keystroke, while the command works out what to draw next, when nothing is
///   waiting for input. Anything arriving then is held until the question that
///   wants it turns up. Without that, typing faster than the command answers
///   loses characters, and a dictated phrase arrives in pieces.
/// - **The screen never blanks between the two.** Answering leaves nothing
///   waiting until the next frame arrives, so what is drawn is the last frame
///   rather than the pending one. Drawing the pending one meant drawing
///   nothing for that moment, once per character, which reads as a flicker.
#[derive(Default)]
pub struct Exchange {
    /// The last lines a question asked for, kept until the next replace them.
    shown: Vec<String>,
    /// Whether a question is waiting on an answer.
    waiting: bool,
    /// Input that arrived before the question that wants it, oldest first.
    queued: VecDeque<Input>,
}

impl Exchange {
    /// Whether somebody has pressed something that nothing has taken yet.
    ///
    /// An animation asks before drawing its next frame: a held key would
    /// otherwise stack up turns behind an animation that is still running.
    #[must_use]
    pub fn interrupted(&self) -> bool {
        !self.queued.is_empty()
    }

    /// Takes on the lines a question wants drawn.
    pub fn show(&mut self, lines: Vec<String>) {
        self.shown = lines;
        self.waiting = true;
    }

    /// Keeps input until something asks for it.
    pub fn push(&mut self, input: Input) {
        self.queued.push_back(input);
    }

    /// The answer to send back, when there is a question and something to
    /// answer it with.
    pub fn take(&mut self) -> Option<Input> {
        if !self.waiting {
            return None;
        }
        let input = self.queued.pop_front()?;
        self.waiting = false;
        Some(input)
    }

    /// The lines to draw.
    #[must_use]
    pub fn shown(&self) -> &[String] {
        &self.shown
    }
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
    /// What is on the screen and what has been typed at it.
    exchange: Exchange,
    /// Who to send the answer back to, while a question is waiting.
    reply: Option<SyncSender<Reply>>,
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
            exchange: Exchange::default(),
            reply: None,
            output: Vec::new(),
        }
    }

    /// A job that is not running anything, for drawing a panel in a test.
    #[cfg(test)]
    #[must_use]
    pub fn idle(title: &str, output: &[String]) -> Self {
        let channel = host::detached();
        Self {
            title: title.to_owned(),
            // A handle over a task that has nothing to do: this exists to
            // draw a panel, not to run anything.
            handle: tokio::runtime::Builder::new_current_thread()
                .build()
                .expect("a runtime")
                .spawn(std::future::ready(Landing::Stay)),
            channel,
            exchange: Exchange::default(),
            reply: None,
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
            exchange: Exchange::default(),
            reply: None,
            output: Vec::new(),
        }
    }

    /// What the panel calls itself.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Takes whatever the command has said, and answers what it has asked.
    ///
    /// Output is taken at once so the command carries on. A question is
    /// answered from whatever has been typed, which may have been typed before
    /// the question arrived: the loop never turns input away, so a burst from
    /// a paste or a fast hand is spent one frame at a time rather than lost.
    pub fn collect(&mut self) {
        loop {
            if self.reply.is_some() {
                let Some(input) = self.exchange.take() else {
                    // Asking, and nothing typed at it yet.
                    return;
                };
                if let Some(reply) = self.reply.take() {
                    let _ = reply.send(Reply::Input(input));
                }
                continue;
            }
            match self.channel.requests.try_recv() {
                Ok((Request::Show(lines), reply)) => {
                    self.output.extend(lines);
                    let _ = reply.send(Reply::Shown);
                }
                Ok((Request::Frame(lines), reply)) => {
                    self.exchange.show(lines);
                    self.reply = Some(reply);
                }
                Ok((Request::Step(lines), reply)) => {
                    self.exchange.show(lines);
                    let _ = reply.send(Reply::Stepped {
                        interrupted: self.exchange.interrupted(),
                    });
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return,
            }
        }
    }

    /// Offers input to the command, now or when it next asks for some.
    pub fn feed(&mut self, input: Input) {
        self.exchange.push(input);
    }

    /// The lines to draw, newest output last and the question at the foot.
    #[must_use]
    pub fn visible(&self, height: usize) -> Vec<String> {
        let shown = self.exchange.shown();
        window(&self.output, (!shown.is_empty()).then_some(shown), height)
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
mod exchanges {
    //! The two bugs this type exists to prevent, and nothing else.
    //!
    //! Both were found by typing into a hosted question: characters going
    //! missing, and the panel blinking on every keypress.

    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn typed(character: char) -> Input {
        Input::Key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))
    }

    fn letter(input: &Input) -> char {
        match input {
            Input::Key(key) => match key.code {
                KeyCode::Char(character) => character,
                other => panic!("{other:?}"),
            },
            Input::Pasted(text) => panic!("{text}"),
        }
    }

    /// Typing faster than the command answers must not lose characters.
    ///
    /// There is a window after every keystroke, while the command works out
    /// what to draw next, when nothing is waiting for input. Anything typed in
    /// it used to be dropped on the floor, which is why a dictated phrase
    /// arrived with letters missing.
    #[test]
    fn input_that_arrives_before_a_question_wants_it_is_kept_not_dropped() {
        let mut exchange = Exchange::default();
        // Three characters typed while the command is still thinking.
        for character in "30m".chars() {
            exchange.push(typed(character));
        }
        assert!(
            exchange.take().is_none(),
            "nothing is asking yet, so nothing is answered yet"
        );

        exchange.show(vec!["When?".to_owned()]);
        let first = exchange.take().expect("the question takes what was typed");
        assert_eq!(letter(&first), '3');

        // And the rest are still there, in order, for the frames after it.
        exchange.show(vec!["When? 3".to_owned()]);
        assert_eq!(letter(&exchange.take().expect("still queued")), '0');
        exchange.show(vec!["When? 30".to_owned()]);
        assert_eq!(letter(&exchange.take().expect("still queued")), 'm');
        exchange.show(vec!["When? 30m".to_owned()]);
        assert!(exchange.take().is_none(), "and then there are no more");
    }

    /// The panel must not blink empty between a keystroke and the redraw.
    ///
    /// Answering a question leaves nothing waiting until the command sends its
    /// next frame. Drawing from what is waiting meant drawing nothing for that
    /// moment, once per character.
    #[test]
    fn the_last_question_stays_on_screen_while_the_next_one_is_being_drawn() {
        let mut exchange = Exchange::default();
        exchange.show(vec!["When?".to_owned(), "> 30".to_owned()]);
        exchange.push(typed('m'));
        exchange.take().expect("the question takes it");

        assert_eq!(
            exchange.shown(),
            ["When?", "> 30"],
            "the question is still on the screen until the next frame replaces it"
        );
        exchange.show(vec!["When?".to_owned(), "> 30m".to_owned()]);
        assert_eq!(exchange.shown(), ["When?", "> 30m"]);
    }

    #[test]
    fn a_pasted_burst_is_one_answer_rather_than_one_per_character() {
        let mut exchange = Exchange::default();
        exchange.show(vec!["When?".to_owned()]);
        exchange.push(Input::Pasted("30 minutes ago".to_owned()));
        match exchange.take().expect("the paste answers the question") {
            Input::Pasted(text) => assert_eq!(text, "30 minutes ago"),
            Input::Key(key) => panic!("{key:?}"),
        }
        assert!(exchange.take().is_none());
    }

    #[test]
    fn nothing_has_been_asked_yet_so_there_is_nothing_to_draw() {
        assert!(Exchange::default().shown().is_empty());
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
}
