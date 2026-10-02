//! The time dial: three turning columns instead of a typed time.
//!
//! A parent at 3am has one hand free and is not going to type "1:44 pm". The
//! dial is the phone's time picker in a terminal: an hour, a minute and a
//! half of the day, each turned with the arrow keys, each showing the values
//! either side of the one chosen so it reads as a wheel rather than a number.
//!
//! It is one component used everywhere a time is asked for, so a fix here is
//! a fix in all of them. [`model`] decides what keys do, [`draw`] turns that
//! into lines, and `terminal` owns the screen or lets the shell own it.

pub mod draw;
pub mod model;
mod terminal;

pub use terminal::ask;
