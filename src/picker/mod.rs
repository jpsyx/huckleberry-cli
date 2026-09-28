//! Choosing one entry off a list, full screen.
//!
//! The same split as the dashboard: [`state`] decides and is pure, [`draw`]
//! turns it into widgets, and [`crate::commands::edit`] owns the terminal.
//!
//! This is the full-screen picker `docs/decisions.md` left the door open for.
//! It is deliberately the only one: a question with a fixed set of answers
//! still belongs in `src/prompt.rs`, and this exists because forty entries
//! with a scroll are not a numbered list anybody can read.

pub mod draw;
pub mod state;

pub use draw::draw;
pub use state::{Action, Picker, action_for, editable};
