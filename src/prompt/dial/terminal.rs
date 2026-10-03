//! Turning the dial: hosted by the shell, or owning the terminal itself.
//!
//! One loop, two owners, the same state and the same renderer: only who holds
//! the screen differs, which is what keeps a single implementation of the
//! dial rather than one for each.

use std::io::{Write, stderr};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use crossterm::{cursor, event, execute, terminal};

use super::draw;
use super::model::{Dial, DialAction};
use crate::prompt::{Cancelled, host, terminal::TerminalGuard};
use crate::theme::Theme;

/// How long one frame of a turn is held on the screen.
///
/// Short enough that a five minute turn lands in a seventh of a second, long
/// enough that the numbers are seen to travel rather than to blink. A dial
/// that jumps gives no clue which way it went, which is the whole reason the
/// intervening minutes are drawn at all.
const FRAME: Duration = Duration::from_millis(35);

/// Turns a dial until somebody takes a time off it.
///
/// # Errors
///
/// Cancelled when somebody backs out, and a failure when there is no screen
/// to draw a dial on.
pub fn ask(
    label: &str,
    dial: Dial,
    theme: Theme,
    settle: impl FnMut(&mut Dial, usize),
) -> Result<Dial> {
    if host::hosted() {
        return hosted(label, dial, theme, settle);
    }
    if !crate::prompt::available() {
        bail!("turning a dial needs terminal input and stderr");
    }
    let mut guard = TerminalGuard::enter()?;
    let result = owned(label, dial, theme, settle);
    let restored = guard.restore();
    eprintln!();
    restored?;
    result
}

/// The dial, drawn by whatever is hosting the screen.
fn hosted(
    label: &str,
    mut dial: Dial,
    theme: Theme,
    mut settle: impl FnMut(&mut Dial, usize),
) -> Result<Dial> {
    loop {
        let (width, _) = host::size();
        let host::Input::Key(key) = host::frame(draw::render(label, &dial, width, theme))? else {
            // There is nothing to paste onto a dial.
            continue;
        };
        let before = dial.clone();
        match dial.apply(key) {
            DialAction::Stay => {}
            DialAction::Submit => return Ok(dial),
            DialAction::Cancel => return Err(Cancelled.into()),
            DialAction::Turned(column) => {
                settle(&mut dial, column);
                for passing in dial.passed_through(&before) {
                    if host::step(draw::render(label, &passing, width, theme)) {
                        break;
                    }
                    std::thread::sleep(FRAME);
                }
            }
        }
    }
}

/// The dial, owning the terminal.
fn owned(
    label: &str,
    mut dial: Dial,
    theme: Theme,
    mut settle: impl FnMut(&mut Dial, usize),
) -> Result<Dial> {
    let mut output = stderr();
    let mut drawn = 0;
    execute!(output, cursor::Hide)?;
    loop {
        let width = terminal::size().unwrap_or((80, 24)).0;
        let lines = draw::render(label, &dial, width, theme);
        redraw(&mut output, &lines, drawn)?;
        drawn = lines.len();
        let event::Event::Key(key) = event::read().context("turning the dial")? else {
            continue;
        };
        let before = dial.clone();
        match dial.apply(key) {
            DialAction::Stay => {}
            DialAction::Submit => {
                redraw(&mut output, &[], drawn)?;
                execute!(output, cursor::Show)?;
                return Ok(dial);
            }
            DialAction::Cancel => {
                redraw(&mut output, &[], drawn)?;
                execute!(output, cursor::Show)?;
                return Err(Cancelled.into());
            }
            DialAction::Turned(column) => {
                settle(&mut dial, column);
                for passing in dial.passed_through(&before) {
                    let lines = draw::render(label, &passing, width, theme);
                    redraw(&mut output, &lines, drawn)?;
                    drawn = lines.len();
                    // Somebody holding the key down is already ahead of the
                    // animation; catching up matters more than finishing it.
                    if event::poll(Duration::ZERO).unwrap_or(false) {
                        break;
                    }
                    std::thread::sleep(FRAME);
                }
            }
        }
    }
}

fn redraw(output: &mut impl Write, lines: &[String], previous: usize) -> Result<()> {
    if previous > 0 {
        execute!(
            output,
            cursor::MoveUp(u16::try_from(previous).unwrap_or(u16::MAX))
        )?;
    }
    execute!(
        output,
        cursor::MoveToColumn(0),
        terminal::Clear(terminal::ClearType::FromCursorDown)
    )?;
    for line in lines {
        write!(output, "{line}\r\n")?;
    }
    output.flush().context("drawing the dial")
}

#[cfg(test)]
mod hosted_dial {
    //! The loop itself, with a plain dial of numbers: what it draws, what it
    //! animates, and how it ends. The host is a single global slot, so these
    //! take its lock and run one at a time.
    use super::*;
    use crate::prompt::dial::wheel::Wheel;
    use crate::prompt::host::{Input, Reply, Request};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::sync::mpsc::SyncSender;

    fn press(code: KeyCode, modifiers: KeyModifiers) -> Reply {
        Reply::Input(Input::Key(KeyEvent::new(code, modifiers)))
    }

    fn next(channel: &host::Channel) -> (Request, SyncSender<Reply>) {
        channel
            .requests
            .recv_timeout(Duration::from_secs(5))
            .expect("a request within five seconds")
    }

    /// Minutes, standing at forty-four, leaping five.
    fn minutes() -> Dial {
        Dial::new(vec![Wheel::new(
            (0..60).map(|minute| format!("{minute:02}")).collect(),
            44,
            5,
            "",
        )])
    }

    /// What the middle row of whatever was drawn is showing.
    fn middle(lines: &[String]) -> String {
        lines
            .iter()
            .filter(|line| line.chars().any(char::is_numeric))
            .nth(2)
            .expect("a middle row")
            .trim()
            .to_owned()
    }

    fn turning() -> std::thread::JoinHandle<Result<Dial>> {
        std::thread::spawn(|| {
            ask(
                "How many?",
                minutes(),
                crate::theme::Theme::dark(false),
                |_, _| {},
            )
        })
    }

    /// A dial that jumps gives no clue which way it went. The places in
    /// between are drawn so the leap can be seen to travel.
    #[test]
    fn a_leap_draws_every_place_it_passes_on_the_way() {
        let _serial = host::one_at_a_time();
        let channel = host::install();
        let asking = turning();

        let (_, reply) = next(&channel);
        reply
            .send(press(KeyCode::Down, KeyModifiers::SHIFT))
            .expect("listening");

        let mut passed = Vec::new();
        let reply = loop {
            match next(&channel) {
                (Request::Step(lines), reply) => {
                    passed.push(middle(&lines));
                    reply
                        .send(Reply::Stepped { interrupted: false })
                        .expect("listening");
                }
                (Request::Frame(lines), reply) => {
                    assert_eq!(middle(&lines), "49", "and lands five on");
                    break reply;
                }
                (Request::Show(lines), _) => panic!("a dial shows nothing: {lines:?}"),
            }
        };
        assert_eq!(passed, ["45", "46", "47", "48"]);

        reply
            .send(press(KeyCode::Enter, KeyModifiers::NONE))
            .expect("listening");
        let dial = asking.join().expect("ends").expect("a dial");
        assert_eq!(dial.value(0), "49");
        host::remove();
    }

    /// A plain turn moves one place, so there is nothing to animate.
    #[test]
    fn a_single_step_is_not_animated_at_all() {
        let _serial = host::one_at_a_time();
        let channel = host::install();
        let asking = turning();

        let (_, reply) = next(&channel);
        reply
            .send(press(KeyCode::Down, KeyModifiers::NONE))
            .expect("listening");

        let (request, reply) = next(&channel);
        let Request::Frame(lines) = request else {
            panic!("one step draws no animation");
        };
        assert_eq!(middle(&lines), "45");
        reply
            .send(press(KeyCode::Enter, KeyModifiers::NONE))
            .expect("listening");
        asking.join().expect("ends").expect("a dial");
        host::remove();
    }

    /// Somebody holding the key down is already ahead of the animation, so
    /// catching up matters more than finishing it.
    #[test]
    fn a_leap_gives_up_its_animation_when_somebody_is_already_waiting() {
        let _serial = host::one_at_a_time();
        let channel = host::install();
        let asking = turning();

        let (_, reply) = next(&channel);
        reply
            .send(press(KeyCode::Down, KeyModifiers::SHIFT))
            .expect("listening");

        let (request, reply) = next(&channel);
        assert!(matches!(request, Request::Step(_)), "it starts animating");
        reply
            .send(Reply::Stepped { interrupted: true })
            .expect("listening");

        let (request, reply) = next(&channel);
        let Request::Frame(lines) = request else {
            panic!("it stops animating and asks again");
        };
        assert_eq!(middle(&lines), "49", "without losing where it landed");
        reply
            .send(press(KeyCode::Enter, KeyModifiers::NONE))
            .expect("listening");
        asking.join().expect("ends").expect("a dial");
        host::remove();
    }

    #[test]
    fn backing_out_of_the_dial_is_a_cancellation_rather_than_a_value() {
        let _serial = host::one_at_a_time();
        let channel = host::install();
        let asking = turning();
        let (_, reply) = next(&channel);
        reply
            .send(press(KeyCode::Esc, KeyModifiers::NONE))
            .expect("listening");
        let error = asking.join().expect("ends").expect_err("cancelled");
        assert!(crate::prompt::is_cancelled(&error), "{error:#}");
        host::remove();
    }
}
