//! Cooperative interruption of awaited work in the persistent session.
use anyhow::{Context, Result};
use std::future::Future;

/// Cancels waiting on Ctrl-C and never retries a potentially submitted request.
pub async fn run<T>(operation: impl Future<Output = Result<T>>) -> Result<T> {
    tokio::select! {
        biased;
        signal = tokio::signal::ctrl_c() => {
            signal.context("listening for operation interruption")?;
            crate::render::note("Stopped waiting. A submitted request may have completed; check logs before retrying.");
            Err(crate::prompt::Cancelled.into())
        }
        result = operation => result,
    }
}
