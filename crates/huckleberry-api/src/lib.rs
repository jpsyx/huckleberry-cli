//! A Rust client for [Huckleberry](https://huckleberrycare.com), the baby
//! tracking app: read a child's sleep, feeds, nappies, pumping, health and
//! milestones, and write new entries back.
//!
//! # Credit
//!
//! **This crate is a Rust port of
//! [`py-huckleberry-api`](https://github.com/Woyken/py-huckleberry-api) by
//! [Woyken](https://github.com/Woyken), and it exists because that project
//! exists.**
//!
//! Huckleberry publishes no API and no documentation for one. Working out that
//! it is a Firebase application, which collection holds what, what every field
//! is called, which units each one is in, and what sequence of writes the app
//! expects for each operation is original research, and all of it is
//! Woyken's. This crate translates that knowledge into Rust; it does not
//! originate it.
//!
//! py-huckleberry-api is MIT licensed, Copyright (c) 2025 Woyken, and its
//! notice travels with this port in `NOTICE`.
//!
//! The port has the same coverage and the same field-for-field fidelity to
//! what the app itself writes. `docs/api.md` in the repository lists every
//! method beside its Python original and records the four places the two
//! deliberately differ.
//!
//! # How it talks to Huckleberry
//!
//! Huckleberry is a Firebase application. Signing in is Firebase Identity
//! Toolkit, and the data is Cloud Firestore, reached here over its **REST**
//! API rather than gRPC: that keeps the dependency tree to an HTTP client and
//! serde, at the cost of real-time listeners, which REST does not offer. See
//! [`ops::watch`] for what replaces them.
//!
//! # Getting started
//!
//! ```no_run
//! use huckleberry_api::{Credentials, Huckleberry, Window, client::now_seconds};
//!
//! # async fn example() -> Result<(), huckleberry_api::Error> {
//! let client = Huckleberry::new(
//!     Credentials::new("parent@example.com", "hunter2"),
//!     "America/New_York",
//! )?;
//!
//! let user = client.user().await?;
//! let child = user.first_child().expect("a child on the account");
//!
//! let week = Window::last_days(now_seconds(), 7);
//! let feeds = client.feed_intervals(&child.cid, week).await?;
//! println!("{} feeds in the last week", feeds.len());
//! # Ok(())
//! # }
//! ```
//!
//! # Three things that catch people out
//!
//! 1. **The sleep timer is in milliseconds and the feed timer is in seconds.**
//!    [`models::SleepTimer::started_at`] and [`models::FeedTimer::totals`] do
//!    the conversion so no caller has to remember.
//! 2. **A timer with `active: false` is not a paused session.** A finished
//!    feed is left as `{ active: false, paused: true }`, which reads like one.
//!    Use [`models::FeedDocument::running_timer`].
//! 3. **History comes from two queries, not one.** Older rows are packed into
//!    batched documents whose contents Firestore cannot filter, so every
//!    windowed read fetches the loose rows and the batches separately. That is
//!    already done for you; it is here because it explains the read count.
//!
//! # Safety and scope
//!
//! `#![forbid(unsafe_code)]` is set in `Cargo.toml`. Every write this crate
//! performs is one the app performs too, in the same shape; nothing here
//! deletes a child, an account, or history.

/// This crate's version, so a caller can report which one it is using.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[macro_use]
mod macros;

pub mod auth;
pub mod client;
pub mod constants;
pub mod error;
pub mod firestore;
pub mod ids;
pub mod models;
pub mod ops;
pub mod paths;
pub mod rows;
pub mod timezone;

pub use auth::Session;
pub use client::{Credentials, Huckleberry};
pub use error::{Error, Result};
pub use ops::diaper::DiaperDetails;
pub use ops::feed::CompletedNursing;
pub use ops::health::GrowthMeasurements;
pub use ops::sleep::CompletedSleep;
pub use ops::watch::Watching;
pub use ops::{TimerChange, Window};
pub use rows::{Located, RowRef};
pub use timezone::Zone;
