//! Command preparation registry, extended alongside each typed family form.
use super::PreparedCommand;
use crate::{cli::Command, session::Context};
use anyhow::Result;

/// Prepares a retained command, or leaves a read-only operation to its existing viewer.
pub async fn prepare(
    _context: Context,
    _command: Command,
    _now: f64,
) -> Result<Option<PreparedCommand>> {
    Ok(None)
}
