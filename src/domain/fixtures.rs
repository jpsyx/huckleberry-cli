//! Datasets the domain tests are built from.
//!
//! One place, so a test that needs "a dataset with two bottles yesterday"
//! composes it from small helpers rather than restating twenty fields. Test
//! code only: the module is `#[cfg(test)]` at its declaration.

use super::types::{Child, Dataset, DiaperEvent, FeedEvent, LiveState, SleepEvent};

/// 2025-09-22T18:00:00Z, which is 2pm in New York.
pub const AFTERNOON: f64 = 1_758_564_000.0;
/// 2025-09-23T07:00:00Z, which is 3am in New York.
pub const THREE_AM: f64 = 1_758_610_800.0;

pub fn child() -> Child {
    Child {
        cid: "c1".to_owned(),
        name: "Bear".to_owned(),
        birthdate: Some("2025-09-01".to_owned()),
        night_start_hour: 20.0,
        morning_cutoff_hour: 7.0,
    }
}

pub fn dataset() -> Dataset {
    Dataset {
        fetched_at: AFTERNOON,
        timezone: "America/New_York".to_owned(),
        days: 7,
        child: child(),
        growth: None,
        sleep: Vec::new(),
        feeds: Vec::new(),
        diapers: Vec::new(),
        pumps: Vec::new(),
        milestones: Vec::new(),
        live: LiveState::default(),
        notes: Vec::new(),
    }
}

pub fn sleep(start: f64, duration: f64) -> SleepEvent {
    SleepEvent {
        at: None,
        id: format!("sleep-{start}"),
        start,
        duration,
        locations: Vec::new(),
        start_mood: Vec::new(),
        end_mood: Vec::new(),
        notes: None,
    }
}

pub fn bottle(start: f64, amount: f64) -> FeedEvent {
    FeedEvent::Bottle {
        at: None,
        id: format!("feed-{start}"),
        start,
        amount_ml: Some(amount),
        bottle_type: Some("Formula".to_owned()),
        notes: None,
    }
}

pub fn diaper(start: f64, wet: bool, dirty: bool) -> DiaperEvent {
    DiaperEvent {
        at: None,
        id: format!("diaper-{start}"),
        start,
        mode: "both".to_owned(),
        wet,
        dirty,
        pee_size: None,
        poo_size: None,
        color: None,
        consistency: None,
        rash: false,
        potty: false,
        notes: None,
    }
}
