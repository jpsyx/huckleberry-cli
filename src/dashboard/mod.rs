//! The full-screen dashboard.
//!
//! Split so that nothing that decides anything also owns the terminal:
//! [`state`] holds what is showing and what a keystroke does to it, all pure
//! and all tested; [`draw`] turns that into widgets; and
//! [`crate::commands::dash`] owns the alternate screen, the event loop and the
//! network.

pub mod draw;
pub mod state;

pub use draw::draw;
pub use state::{Action, State, Tab, action_for};
