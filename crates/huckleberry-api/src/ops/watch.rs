//! Watching a tracker for changes.
//!
//! The Python client sets up Firestore snapshot listeners, which are a
//! bidirectional gRPC stream. The REST API this crate is built on has no
//! equivalent, so watching here is polling: read the document, hand it to the
//! caller when it differs from last time, wait, repeat.
//!
//! That is a real difference and worth saying plainly. A listener is pushed
//! within a second of a write; a poll is late by up to its interval, and it
//! costs one read per interval per watcher. For a command-line dashboard
//! refreshing every few seconds that trade is fine, and it is the only one
//! available without a gRPC stack.

use core::time::Duration;

use serde::de::DeserializeOwned;

use crate::client::Huckleberry;
use crate::error::Result;
use crate::models::diaper::DiaperDocument;
use crate::models::feed::FeedDocument;
use crate::models::health::HealthDocument;
use crate::models::pump::PumpDocument;
use crate::models::sleep::SleepDocument;
use crate::paths;

/// What a watcher should do after being handed a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Watching {
    /// Keep polling.
    Continue,
    /// Stop, and return from the watch.
    Stop,
}

impl Huckleberry {
    /// Watches the pump tracker, including timer and last-session changes.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::watch_tracker`].
    pub async fn watch_pump<OnChange, OnError>(
        &self,
        cid: &str,
        interval: Duration,
        on_change: OnChange,
        on_error: OnError,
    ) -> Result<()>
    where
        OnChange: FnMut(&PumpDocument) -> Watching,
        OnError: FnMut(&crate::error::Error) -> Watching,
    {
        self.watch_tracker(paths::PUMP, cid, interval, on_change, on_error)
            .await
    }

    /// Polls a tracker's document, calling `on_change` whenever it differs
    /// from the last reading.
    ///
    /// The first successful read always counts as a change, so a caller sees
    /// the current state without a special case for it. A failed read does
    /// not end the watch: a dashboard should survive a dropped connection, so
    /// the failure is handed to `on_error`, which decides.
    ///
    /// # Errors
    ///
    /// Only what `on_error` chooses to propagate by returning
    /// [`Watching::Stop`] after a failure.
    pub async fn watch_tracker<T, OnChange, OnError>(
        &self,
        tracker: &str,
        cid: &str,
        interval: Duration,
        mut on_change: OnChange,
        mut on_error: OnError,
    ) -> Result<()>
    where
        T: DeserializeOwned + PartialEq,
        OnChange: FnMut(&T) -> Watching,
        OnError: FnMut(&crate::error::Error) -> Watching,
    {
        let path = paths::tracker(tracker, cid);
        let operation = "watching a tracker";
        let mut previous: Option<T> = None;
        loop {
            match self.document::<T>(&path, operation).await {
                Ok(Some(current)) => {
                    if previous.as_ref() != Some(&current) {
                        if on_change(&current) == Watching::Stop {
                            return Ok(());
                        }
                        previous = Some(current);
                    }
                }
                // A tracker with no document yet is not a failure; it is a
                // tracker nothing has been logged in.
                Ok(None) => {}
                Err(failure) => {
                    if on_error(&failure) == Watching::Stop {
                        return Err(failure);
                    }
                }
            }
            tokio::time::sleep(interval).await;
        }
    }

    /// Watches the sleep tracker.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::watch_tracker`].
    pub async fn watch_sleep<OnChange, OnError>(
        &self,
        cid: &str,
        interval: Duration,
        on_change: OnChange,
        on_error: OnError,
    ) -> Result<()>
    where
        OnChange: FnMut(&SleepDocument) -> Watching,
        OnError: FnMut(&crate::error::Error) -> Watching,
    {
        self.watch_tracker(paths::SLEEP, cid, interval, on_change, on_error)
            .await
    }

    /// Watches the feed tracker.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::watch_tracker`].
    pub async fn watch_feed<OnChange, OnError>(
        &self,
        cid: &str,
        interval: Duration,
        on_change: OnChange,
        on_error: OnError,
    ) -> Result<()>
    where
        OnChange: FnMut(&FeedDocument) -> Watching,
        OnError: FnMut(&crate::error::Error) -> Watching,
    {
        self.watch_tracker(paths::FEED, cid, interval, on_change, on_error)
            .await
    }

    /// Watches the diaper tracker.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::watch_tracker`].
    pub async fn watch_diaper<OnChange, OnError>(
        &self,
        cid: &str,
        interval: Duration,
        on_change: OnChange,
        on_error: OnError,
    ) -> Result<()>
    where
        OnChange: FnMut(&DiaperDocument) -> Watching,
        OnError: FnMut(&crate::error::Error) -> Watching,
    {
        self.watch_tracker(paths::DIAPER, cid, interval, on_change, on_error)
            .await
    }

    /// Watches the health tracker.
    ///
    /// # Errors
    ///
    /// As [`Huckleberry::watch_tracker`].
    pub async fn watch_health<OnChange, OnError>(
        &self,
        cid: &str,
        interval: Duration,
        on_change: OnChange,
        on_error: OnError,
    ) -> Result<()>
    where
        OnChange: FnMut(&HealthDocument) -> Watching,
        OnError: FnMut(&crate::error::Error) -> Watching,
    {
        self.watch_tracker(paths::HEALTH, cid, interval, on_change, on_error)
            .await
    }
}
