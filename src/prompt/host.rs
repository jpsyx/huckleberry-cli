//! Letting something else own the screen a prompt draws on.
//!
//! Every interactive loop in this tool is the same shape: draw some lines,
//! wait for a key, decide, repeat. Normally the loop owns the terminal and
//! does both itself. Inside the shell it must not: the shell is already
//! drawing, and a prompt writing over it would scribble across the panels the
//! parent is reading.
//!
//! So a host can be installed for as long as a command runs. While one is
//! installed, [`frame`] hands the lines to the host and gets the next
//! keystroke back, and [`show`] hands over output. The loops are otherwise
//! unchanged, which is what keeps one implementation of every prompt rather
//! than one for the terminal and one for the shell.
//!
//! Exactly one command runs at a time, so the host is a single global slot
//! rather than something threaded through every signature.

use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Mutex, OnceLock};

use anyhow::Result;
use crossterm::event::KeyEvent;

use super::Cancelled;

/// What a hosted loop is asking the screen to do.
pub enum Request {
    /// Draw these lines and send back the next keystroke.
    Frame(Vec<String>),
    /// Draw these lines and carry on without waiting: one frame of an
    /// animation, which is a picture rather than a question.
    Step(Vec<String>),
    /// Show these lines as output, and carry on without waiting.
    Show(Vec<String>),
}

/// What somebody did to answer a frame.
#[derive(Debug, Clone)]
pub enum Input {
    /// One keystroke.
    Key(KeyEvent),
    /// Text pasted in one go.
    Pasted(String),
}

/// What the host sends back.
pub enum Reply {
    /// What a [`Request::Frame`] waited for.
    Input(Input),
    /// A [`Request::Show`] has been taken.
    Shown,
    /// A [`Request::Step`] has been drawn.
    Stepped {
        /// Whether somebody has already pressed something.
        ///
        /// An animation holding a key down would otherwise queue a second
        /// behind the first and run minutes late. Told that input is waiting,
        /// the loop drops the rest of the animation and goes to the answer.
        interrupted: bool,
    },
}

/// One side of the conversation, held by whoever is hosting.
pub struct Channel {
    /// Requests from the command.
    pub requests: Receiver<(Request, SyncSender<Reply>)>,
}

/// The handle a hosted loop talks through.
struct Host {
    requests: SyncSender<(Request, SyncSender<Reply>)>,
}

fn slot() -> &'static Mutex<Option<Host>> {
    static SLOT: OnceLock<Mutex<Option<Host>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

/// Installs a host, and hands back the side it listens on.
///
/// Bounded at one so a command that draws faster than the screen reads waits
/// for it rather than running ahead of what is on the screen.
#[must_use]
pub fn install() -> Channel {
    let (requests, incoming) = sync_channel(1);
    if let Ok(mut slot) = slot().lock() {
        *slot = Some(Host { requests });
    }
    Channel { requests: incoming }
}

/// A channel wired to nothing, for a job that never asks anything.
///
/// Installing a real host and taking it away again would reach into the
/// global slot and disconnect whatever was already using it, which in a test
/// run is some other test's live conversation.
#[cfg(test)]
#[must_use]
pub fn detached() -> Channel {
    let (_unused, incoming) = sync_channel(1);
    Channel { requests: incoming }
}

/// Takes the host away, so prompts own the terminal again.
pub fn remove() {
    if let Ok(mut slot) = slot().lock() {
        *slot = None;
    }
}

/// What a panel is assumed to be before a host has said.
const DEFAULT_SIZE: (u16, u16) = (80, 24);

fn panel_size() -> &'static Mutex<(u16, u16)> {
    static SIZE: OnceLock<Mutex<(u16, u16)>> = OnceLock::new();
    SIZE.get_or_init(|| Mutex::new(DEFAULT_SIZE))
}

/// How big the panel a hosted prompt draws into is.
///
/// Recorded by the host each time it draws, so a loop that lays itself out to
/// a width lays itself out to the panel's rather than the terminal's.
#[must_use]
pub fn size() -> (u16, u16) {
    panel_size().lock().map_or(DEFAULT_SIZE, |size| *size)
}

/// Records the panel size for the prompts drawn into it.
pub fn set_size(width: u16, height: u16) {
    if let Ok(mut size) = panel_size().lock() {
        *size = (width.max(8), height.max(3));
    }
}

/// Whether something is hosting the prompts.
#[must_use]
pub fn hosted() -> bool {
    slot().lock().is_ok_and(|slot| slot.is_some())
}

/// Sends a request, and waits for what comes back.
///
/// A host that has gone away reads as a cancellation: the screen it was
/// drawing on is not there any more, so neither is the answer.
fn send(request: Request) -> Result<Reply> {
    let (reply, answer) = sync_channel(1);
    let sent = slot().lock().ok().and_then(|slot| {
        slot.as_ref()
            .map(|host| host.requests.send((request, reply)))
    });
    match sent {
        Some(Ok(())) => answer.recv().map_err(|_| Cancelled.into()),
        _ => Err(Cancelled.into()),
    }
}

/// Draws one frame through the host and waits for an answer.
pub fn frame(lines: Vec<String>) -> Result<Input> {
    match send(Request::Frame(lines))? {
        Reply::Input(input) => Ok(input),
        Reply::Shown | Reply::Stepped { .. } => Err(Cancelled.into()),
    }
}

/// Draws one frame of an animation and carries straight on.
///
/// Answers whether to stop animating: either somebody has pressed something
/// and is waiting, or the host has gone and there is nothing to draw on.
#[must_use]
pub fn step(lines: Vec<String>) -> bool {
    match send(Request::Step(lines)) {
        Ok(Reply::Stepped { interrupted }) => interrupted,
        _ => true,
    }
}

/// Hands output to the host.
///
/// Failure is deliberately ignored: a line of output is not worth failing a
/// command that has already done its work.
pub fn show(lines: Vec<String>) {
    let _ = send(Request::Show(lines));
}

/// Shows one line, when that is all there is.
pub fn show_line(line: &str) {
    show(vec![line.to_owned()]);
}

/// Hosted tests share one global slot, so they run one at a time: a second
/// test installing a host while the first is using it would answer the wrong
/// questions. Every test that installs a host takes this first.
#[cfg(test)]
pub(crate) fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod hosting {
    //! The host is a single global slot, so these run one at a time: a second
    //! test installing a host while the first is using it would answer the
    //! wrong questions.

    use super::*;
    use crate::prompt::select::MenuItem;
    use crate::theme::Theme;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn items() -> Vec<MenuItem> {
        ["First", "Second"]
            .into_iter()
            .map(|label| MenuItem {
                label: label.into(),
                detail: None,
            })
            .collect()
    }

    #[test]
    fn a_hosted_menu_draws_through_the_host_and_takes_the_keys_it_sends() {
        let _serial = one_at_a_time();
        let channel = install();
        let asking = std::thread::spawn(|| {
            crate::prompt::select::choose("Pick one", &items(), 0, Theme::dark(false))
        });

        // The first frame is the menu as it opens.
        let (request, reply) = channel.requests.recv().expect("a frame");
        let Request::Frame(lines) = request else {
            panic!("a menu asks for a frame");
        };
        assert!(
            lines.iter().any(|line| line.contains("Pick one")),
            "{lines:?}"
        );
        assert!(
            lines.iter().any(|line| line.contains("Second")),
            "{lines:?}"
        );
        reply
            .send(Reply::Input(Input::Key(KeyEvent::new(
                KeyCode::Down,
                KeyModifiers::NONE,
            ))))
            .expect("the menu is listening");

        // It redraws with the cursor moved, and Enter takes that row.
        let (_, reply) = channel.requests.recv().expect("a second frame");
        reply
            .send(Reply::Input(Input::Key(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::NONE,
            ))))
            .expect("the menu is listening");

        assert_eq!(asking.join().expect("the menu ends").expect("a row"), 1);
        remove();
    }

    #[test]
    fn output_is_taken_without_waiting_for_anybody_to_press_anything() {
        let _serial = one_at_a_time();
        let channel = install();
        let printing = std::thread::spawn(|| {
            crate::render::print(&["a receipt".to_owned()]);
            crate::render::note("and a word about it");
        });

        for expected in ["a receipt", "and a word about it"] {
            let (request, reply) = channel.requests.recv().expect("output");
            let Request::Show(lines) = request else {
                panic!("output does not wait for a key");
            };
            assert_eq!(lines, [expected]);
            reply.send(Reply::Shown).expect("the writer is listening");
        }
        printing.join().expect("printing ends");
        remove();
    }

    #[test]
    fn a_host_that_goes_away_reads_as_a_cancellation_rather_than_a_hang() {
        let _serial = one_at_a_time();
        let channel = install();
        let asking = std::thread::spawn(|| {
            crate::prompt::select::choose("Pick one", &items(), 0, Theme::dark(false))
        });
        let (_request, reply) = channel.requests.recv().expect("a frame");
        // The screen it was drawing on has gone.
        drop(reply);
        drop(channel);
        let error = asking
            .join()
            .expect("the menu ends")
            .expect_err("cancelled");
        assert!(crate::prompt::is_cancelled(&error), "{error:#}");
        remove();
    }
}
