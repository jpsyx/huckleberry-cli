//! Turning domain values into the text a person reads.
//!
//! Every function here takes values and a [`Theme`](crate::theme::Theme) and
//! returns `Vec<String>`: no printing, no clock, no network. That is what lets
//! a screen be asserted line by line rather than looked at, and it is why the
//! tests in this directory read like descriptions of what the screen says.
//!
//! The rules the screens keep, taken from the dashboard this tool grew out of:
//!
//! - **Nothing is ever painted red, flagged, or alarmed.** Typical ranges are
//!   grey text. A number outside a band is visibly outside it, and what to do
//!   about that is the reader's decision.
//! - **A number nobody recorded prints as a dash.** `0 ml` is a claim about
//!   the baby; `—` is a claim about the record.
//! - **Every "time ago" carries an honest "as of".** A stale reading can send
//!   somebody to wake a sleeping baby.

pub mod format;
pub mod log;
pub mod now;
pub mod stripes;
pub mod summary;
pub mod trends;

/// Writes lines to stdout, which is where data goes.
pub fn print(lines: &[String]) {
    for line in lines {
        println!("{line}");
    }
}
