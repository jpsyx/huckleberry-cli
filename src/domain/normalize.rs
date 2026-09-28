//! From the API's Firestore models to the tool's own event types.
//!
//! One step, one place. Everything downstream of here works with millilitres,
//! seconds and Unix timestamps, and never has to ask whether this particular
//! bottle was recorded in ounces or whether this timer's start was in
//! milliseconds.
//!
//! Where a row cannot be understood it is dropped rather than guessed at. A
//! bottle with no amount keeps its place in the feed count and contributes no
//! volume, because a parent who forgot to type the number did not give the
//! baby nothing.

use huckleberry_api::models::child::ChildDocument;
use huckleberry_api::models::common::Number;
use huckleberry_api::models::diaper::DiaperEntry;
use huckleberry_api::models::feed::{FeedDocument, FeedInterval};
use huckleberry_api::models::health::GrowthEntry;
use huckleberry_api::models::milestone::Milestone;
use huckleberry_api::models::pump::PumpInterval;
use huckleberry_api::models::sleep::{SleepDocument, SleepInterval};
use huckleberry_api::{Located, RowRef};

use super::types::{
    Child, DiaperEvent, FeedEvent, GrowthPoint, LiveState, MilestoneEvent, PumpEvent, Size,
    SleepEvent,
};

/// The child, with the two hours that define a night already resolved.
#[must_use]
pub fn child(cid: &str, fallback_name: Option<&str>, profile: Option<&ChildDocument>) -> Child {
    let name = profile
        .and_then(|document| document.childs_name.as_deref())
        .or(fallback_name)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or("Baby")
        .to_owned();
    Child {
        cid: cid.to_owned(),
        name,
        birthdate: profile
            .and_then(|document| document.birthdate.as_ref())
            .map(huckleberry_api::models::TextOrNumber::to_text),
        night_start_hour: profile.map_or(20.0, ChildDocument::night_start_hour),
        morning_cutoff_hour: profile.map_or(7.0, ChildDocument::morning_cutoff_hour),
    }
}

/// Sleeps, oldest first.
#[must_use]
pub fn sleeps(rows: &[Located<SleepInterval>]) -> Vec<SleepEvent> {
    let mut events: Vec<SleepEvent> = rows
        .iter()
        .map(|located| {
            let row = &located.row;
            let details = row.details.as_ref();
            SleepEvent {
                at: Some(located.at.clone()),
                id: row
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("sleep-{}", row.start.as_i64())),
                start: row.start.as_f64(),
                duration: row.duration.as_f64(),
                locations: details
                    .and_then(|detail| detail.sleep_locations.as_ref())
                    .map(|places| owned(&places.recorded()))
                    .unwrap_or_default(),
                start_mood: details
                    .and_then(|detail| detail.start_sleep_condition.as_ref())
                    .map(|condition| owned(&condition.recorded()))
                    .unwrap_or_default(),
                end_mood: details
                    .and_then(|detail| detail.end_sleep_condition.as_ref())
                    .map(|condition| owned(&condition.recorded()))
                    .unwrap_or_default(),
                notes: details.and_then(|detail| detail.notes.clone()),
            }
        })
        .collect();
    events.sort_by(|left, right| order(left.start, right.start));
    events
}

/// Feeds, oldest first.
#[must_use]
pub fn feeds(rows: &[Located<FeedInterval>]) -> Vec<FeedEvent> {
    let mut events: Vec<FeedEvent> = rows
        .iter()
        .enumerate()
        .map(|(position, located)| match &located.row {
            FeedInterval::Bottle(bottle) => FeedEvent::Bottle {
                at: Some(located.at.clone()),
                id: format!("feed-{position}-{}", bottle.start.as_i64()),
                start: bottle.start.as_f64(),
                amount_ml: Some(bottle.millilitres()),
                bottle_type: Some(bottle.bottle_type.as_str().to_owned()),
                notes: bottle.notes.clone(),
            },
            FeedInterval::Breast(nursing) => FeedEvent::Nursing {
                at: Some(located.at.clone()),
                id: format!("feed-{position}-{}", nursing.start.as_i64()),
                start: nursing.start.as_f64(),
                left_seconds: nursing.left_duration.as_f64(),
                right_seconds: nursing.right_duration.as_f64(),
                last_side: Some(nursing.last_side.as_str().to_owned()),
                notes: nursing.notes.clone(),
            },
            FeedInterval::Solids(meal) => FeedEvent::Solids {
                at: Some(located.at.clone()),
                id: format!("feed-{position}-{}", meal.start.as_i64()),
                start: meal.start.as_f64(),
                foods: meal
                    .foods
                    .as_ref()
                    .map(|eaten| {
                        eaten
                            .values()
                            .map(|food| food.created_name.clone())
                            .collect()
                    })
                    .unwrap_or_default(),
                reaction: meal.reaction().map(|taken| taken.as_str().to_owned()),
                notes: meal.notes.clone(),
            },
        })
        .collect();
    events.sort_by(|left, right| order(left.start(), right.start()));
    events
}

/// Nappies and potty trips, oldest first.
#[must_use]
pub fn diapers(rows: &[Located<DiaperEntry>]) -> Vec<DiaperEvent> {
    let mut events: Vec<DiaperEvent> = rows
        .iter()
        .enumerate()
        .map(|(position, located)| {
            let row = &located.row;
            DiaperEvent {
                at: Some(located.at.clone()),
                id: format!("diaper-{position}-{}", row.start.as_i64()),
                start: row.start.as_f64(),
                mode: row.mode.as_str().to_owned(),
                wet: row.mode.is_wet(),
                dirty: row.mode.is_dirty(),
                pee_size: row
                    .quantity
                    .as_ref()
                    .and_then(|amounts| amounts.pee)
                    .map(size_from),
                poo_size: row
                    .quantity
                    .as_ref()
                    .and_then(|amounts| amounts.poo)
                    .map(size_from),
                color: row.color.as_ref().map(|shade| shade.as_str().to_owned()),
                consistency: row
                    .consistency
                    .as_ref()
                    .map(|texture| texture.as_str().to_owned()),
                rash: row.has_rash(),
                potty: row.is_potty_trip(),
                notes: row.notes.clone(),
            }
        })
        .collect();
    events.sort_by(|left, right| order(left.start, right.start));
    events
}

/// Pumping sessions, oldest first.
#[must_use]
pub fn pumps(rows: &[Located<PumpInterval>]) -> Vec<PumpEvent> {
    let mut events: Vec<PumpEvent> = rows
        .iter()
        .enumerate()
        .map(|(position, located)| {
            let row = &located.row;
            PumpEvent {
                at: Some(located.at.clone()),
                id: format!("pump-{position}-{}", row.start.as_i64()),
                start: row.start.as_f64(),
                left_ml: row
                    .left_amount
                    .map(|amount| row.units.to_millilitres(amount.as_f64())),
                right_ml: row
                    .right_amount
                    .map(|amount| row.units.to_millilitres(amount.as_f64())),
                total_ml: row.total_millilitres(),
                duration_seconds: row.duration.map(Number::as_f64),
                notes: row.notes.clone(),
            }
        })
        .collect();
    events.sort_by(|left, right| order(left.start, right.start));
    events
}

/// Milestones, oldest first.
#[must_use]
pub fn milestones(rows: &[Milestone]) -> Vec<MilestoneEvent> {
    let mut events: Vec<MilestoneEvent> = rows
        .iter()
        .enumerate()
        .map(|(position, row)| MilestoneEvent {
            // The milestones collection is read whole rather than windowed,
            // and nothing edits a milestone, so there is no reference to keep.
            at: None,
            id: row
                .milestone_id
                .clone()
                .unwrap_or_else(|| format!("milestone-{position}")),
            start: row.start.as_f64(),
            name: row.title().to_owned(),
            category: row.category.clone(),
            notes: row.notes.clone(),
            has_photo: row.has_photo(),
        })
        .collect();
    events.sort_by(|left, right| order(left.start, right.start));
    events
}

/// The last growth measurement.
#[must_use]
pub fn growth(entry: Option<&GrowthEntry>) -> Option<GrowthPoint> {
    let entry = entry?;
    Some(GrowthPoint {
        start: entry.start.as_f64(),
        weight_kg: entry.weight_kilograms(),
        weight_raw: entry.weight.map(Number::as_f64),
        weight_units: entry
            .weight_units
            .as_ref()
            .map(|unit| unit.as_str().to_owned()),
    })
}

/// What the two live timers say is happening.
///
/// Two traps, both load-bearing. The sleep timer's start is in
/// **milliseconds** and the feed timer's is in **seconds**: reading them the
/// same way puts one of them in 1970. And a timer counts as live only when
/// `active` is true: a finished feed is left as `{ active: false, paused:
/// true }`, which is a stale session and not a feed in progress.
#[must_use]
pub fn live(sleep: Option<&SleepDocument>, feed: Option<&FeedDocument>) -> LiveState {
    let sleep_timer = sleep.and_then(SleepDocument::running_timer);
    let feed_timer = feed.and_then(FeedDocument::running_timer);
    LiveState {
        sleep_active: sleep_timer.is_some(),
        sleep_start: sleep_timer.and_then(huckleberry_api::models::SleepTimer::started_at),
        sleep_paused: sleep_timer.is_some_and(|timer| timer.paused),
        nursing_active: feed_timer.is_some(),
        nursing_start: feed_timer.and_then(|timer| {
            timer
                .feed_start_time
                .or(timer.timer_start_time)
                .map(Number::as_f64)
        }),
        nursing_side: feed_timer.map(|timer| timer.current_side().as_str().to_owned()),
        nursing_paused: feed_timer.is_some_and(|timer| timer.paused),
    }
}

/// Huckleberry stores a nappy quantity as one of three numbers.
fn size_from(value: Number) -> Size {
    let amount = value.as_f64();
    if amount <= 0.0 {
        Size::Small
    } else if amount <= 50.0 {
        Size::Medium
    } else {
        Size::Large
    }
}

fn owned(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// Oldest first, with an unreadable timestamp sorting first rather than
/// panicking on a comparison that has no answer.
fn order(left: f64, right: f64) -> core::cmp::Ordering {
    left.partial_cmp(&right)
        .unwrap_or(core::cmp::Ordering::Equal)
}

/// Rows as a read hands them over: each at a place of its own.
#[cfg(test)]
fn located<T>(rows: Vec<T>) -> Vec<Located<T>> {
    rows.into_iter()
        .enumerate()
        .map(|(position, row)| Located::new(RowRef::loose("test", &format!("row-{position}")), row))
        .collect()
}

#[cfg(test)]
mod children {
    use super::*;
    use huckleberry_api::models::TextOrNumber;

    #[test]
    fn the_profile_name_wins_over_the_accounts_nickname() {
        let profile = ChildDocument {
            childs_name: Some("Bear".to_owned()),
            ..ChildDocument::default()
        };
        assert_eq!(child("c1", Some("Baby B"), Some(&profile)).name, "Bear");
    }

    #[test]
    fn the_nickname_stands_in_when_the_profile_has_no_name() {
        assert_eq!(child("c1", Some("Baby B"), None).name, "Baby B");
    }

    #[test]
    fn with_no_name_anywhere_there_is_still_something_to_print() {
        assert_eq!(child("c1", None, None).name, "Baby");
        assert_eq!(child("c1", Some("   "), None).name, "Baby");
    }

    #[test]
    fn the_night_hours_come_off_the_profile_already_on_a_twenty_four_hour_clock() {
        let profile = ChildDocument {
            night_start: Some(TextOrNumber::Number(Number::Float(8.0))),
            morning_cutoff: Some(TextOrNumber::Number(Number::Float(6.75))),
            ..ChildDocument::default()
        };
        let normalized = child("c1", None, Some(&profile));
        assert!((normalized.night_start_hour - 20.0).abs() < f64::EPSILON);
        assert!((normalized.morning_cutoff_hour - 6.75).abs() < f64::EPSILON);
    }
}

#[cfg(test)]
mod rows {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_bottle_in_ounces_arrives_in_millilitres() {
        let rows: Vec<FeedInterval> = serde_json::from_value(json!([{
            "mode": "bottle", "start": 100, "bottleType": "Breast Milk",
            "amount": 3, "units": "oz", "offset": 0,
        }]))
        .expect("a feed row");
        let normalized = feeds(&located(rows));
        let millilitres = normalized[0].millilitres().expect("an amount");
        assert!((millilitres - 88.72).abs() < 0.01, "{millilitres}");
    }

    #[test]
    fn feeds_come_out_oldest_first_whatever_order_they_arrived_in() {
        let rows: Vec<FeedInterval> = serde_json::from_value(json!([
            { "mode": "bottle", "start": 300, "bottleType": "Formula", "amount": 1, "units": "ml", "offset": 0 },
            { "mode": "bottle", "start": 100, "bottleType": "Formula", "amount": 1, "units": "ml", "offset": 0 },
        ]))
        .expect("feed rows");
        let normalized = feeds(&located(rows));
        assert!((normalized[0].start() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_nappy_derives_wet_and_dirty_from_its_mode() {
        let rows: Vec<DiaperEntry> =
            serde_json::from_value(json!([{ "mode": "both", "start": 1, "offset": 0 }]))
                .expect("a nappy row");
        let normalized = diapers(&located(rows));
        assert!(normalized[0].wet && normalized[0].dirty);
    }

    #[test]
    fn the_three_stored_numbers_become_the_three_sizes() {
        let rows: Vec<DiaperEntry> = serde_json::from_value(json!([{
            "mode": "both", "start": 1, "offset": 0,
            "quantity": { "pee": 0.0, "poo": 100.0 },
        }]))
        .expect("a nappy row");
        let normalized = diapers(&located(rows));
        assert_eq!(normalized[0].pee_size, Some(Size::Small));
        assert_eq!(normalized[0].poo_size, Some(Size::Large));
    }

    #[test]
    fn only_the_sleep_flags_recorded_true_survive() {
        let rows: Vec<SleepInterval> = serde_json::from_value(json!([{
            "start": 1, "duration": 60, "offset": 0,
            "details": { "sleepLocations": { "onOwnInBed": true, "car": false } },
        }]))
        .expect("a sleep row");
        assert_eq!(sleeps(&located(rows))[0].locations, vec!["onOwnInBed"]);
    }

    #[test]
    fn a_pumping_session_with_no_amounts_has_no_total_rather_than_zero() {
        let rows: Vec<PumpInterval> = serde_json::from_value(json!([{
            "start": 1, "entryMode": "leftright", "units": "ml", "offset": 0, "duration": 900,
        }]))
        .expect("a pump row");
        let normalized = pumps(&located(rows));
        assert_eq!(normalized[0].total_ml, None);
        assert_eq!(normalized[0].duration_seconds, Some(900.0));
    }

    #[test]
    fn a_meal_lists_what_was_eaten() {
        let rows: Vec<FeedInterval> = serde_json::from_value(json!([{
            "mode": "solids", "start": 1, "offset": 0,
            "foods": { "avocado": { "id": "avocado", "created_name": "Avocado", "source": "curated" } },
            "reactions": { "LOVED": true },
        }]))
        .expect("a solids row");
        let FeedEvent::Solids {
            foods, reaction, ..
        } = &feeds(&located(rows))[0]
        else {
            panic!("expected a meal");
        };
        assert_eq!(foods, &vec!["Avocado".to_owned()]);
        assert_eq!(reaction.as_deref(), Some("LOVED"));
    }
}

#[cfg(test)]
mod live_timers {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_running_sleep_start_is_converted_out_of_milliseconds() {
        let sleep: SleepDocument = serde_json::from_value(json!({ "timer": {
            "active": true, "paused": false, "uuid": "abc",
            "timerStartTime": 1_758_572_400_000.0_f64,
        } }))
        .expect("a sleep document");
        let state = live(Some(&sleep), None);
        assert!(state.sleep_active);
        assert_eq!(state.sleep_start, Some(1_758_572_400.0));
    }

    #[test]
    fn a_running_feed_start_is_already_seconds() {
        let feed: FeedDocument = serde_json::from_value(json!({ "timer": {
            "active": true, "paused": false, "uuid": "abc",
            "feedStartTime": 1_758_572_400.0_f64, "activeSide": "right",
        } }))
        .expect("a feed document");
        let state = live(None, Some(&feed));
        assert_eq!(state.nursing_start, Some(1_758_572_400.0));
        assert_eq!(state.nursing_side.as_deref(), Some("right"));
    }

    #[test]
    fn a_finished_feed_left_looking_paused_is_not_a_feed_in_progress() {
        // The exact shape a completed nursing session leaves behind.
        let feed: FeedDocument = serde_json::from_value(json!({ "timer": {
            "active": false, "paused": true, "uuid": "abc", "feedStartTime": 1.0,
        } }))
        .expect("a feed document");
        let state = live(None, Some(&feed));
        assert!(!state.nursing_active);
        assert_eq!(state.nursing_start, None);
    }

    #[test]
    fn nothing_running_is_a_state_rather_than_an_absence() {
        assert_eq!(live(None, None), LiveState::default());
    }
}
