//! The Firestore REST surface this crate is built on.
//!
//! Four modules, in the order a write passes through them:
//! [`query`] and [`field_path`] decide what a request asks for, [`value`]
//! translates between plain JSON and Firestore's tagged wire format, and
//! [`client`] is the only part that opens a socket.

pub mod client;
pub mod field_path;
pub mod query;
pub mod value;

pub use client::{Document, Firestore};
pub use field_path::FieldUpdate;
pub use query::{Op, Query};
