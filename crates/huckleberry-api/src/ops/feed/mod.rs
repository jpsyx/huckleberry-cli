//! Nursing sessions and bottles.
//!
//! Split in two because they have nothing in common but a collection:
//! [`nursing`] runs a timer with two sides that bank independently, and
//! [`bottle`] is one instant event written twice.

pub mod bottle;
pub mod nursing;

pub use nursing::{CompletedNursing, completed_nursing, nursing_totals, opposite_side};
