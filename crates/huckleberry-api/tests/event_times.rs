//! What each write actually sends.
//!
//! These drive the real client against a stub Firestore on a loopback socket.
//! They exist because the request shape is the part of this crate that a unit
//! test cannot see and that Huckleberry will not forgive: a missing
//! `updateMask` entry silently fails to write a field, and a `PATCH` where the
//! app expects a merge quietly deletes the rest of the document.

mod support;

use huckleberry_api::{Huckleberry, Session, Zone};
use serde_json::json;
use support::Stub;

/// A client pointed at the stub, with a session that will not expire during
/// the test, so nothing tries to reach the real Identity Toolkit.
fn client(stub: &Stub) -> Huckleberry {
    Huckleberry::with_parts(
        reqwest::Client::new(),
        stub.documents_url.clone(),
        None,
        Zone::new("America/New_York").expect("a real timezone"),
        Some(Session {
            id_token: "test-id-token".to_owned(),
            refresh_token: "test-refresh-token".to_owned(),
            user_uid: "test-user".to_owned(),
            expires_at: i64::MAX,
        }),
    )
}

/// A Firestore document reply, with the fields already tagged.
fn document(name: &str, fields: &serde_json::Value) -> serde_json::Value {
    let tagged: serde_json::Map<String, serde_json::Value> =
        huckleberry_api::firestore::value::fields_from_json(fields.as_object().expect("an object"));
    json!({ "name": name, "fields": tagged })
}

use huckleberry_api::models::diaper::DiaperMode;
use huckleberry_api::models::feed::{BottleType, FeedSide, VolumeUnits};
use huckleberry_api::models::health::MeasurementSystem;
use huckleberry_api::models::solids::FoodReference;
use huckleberry_api::{DiaperDetails, GrowthMeasurements};

const EVENT_TIME: f64 = 1_735_732_800.0;

async fn record_event(client: &Huckleberry, kind: &str) {
    match kind {
        "bottle" => {
            client
                .log_bottle_at(
                    "c1",
                    90.0,
                    BottleType::Formula,
                    VolumeUnits::Millilitres,
                    None,
                    EVENT_TIME,
                )
                .await
        }
        "solids" => {
            client
                .log_solids_at(
                    "c1",
                    &[FoodReference::curated("pear", "Pear", "some")],
                    None,
                    None,
                    None,
                    EVENT_TIME,
                )
                .await
        }
        "diaper" => {
            client
                .log_diaper_at("c1", DiaperMode::Pee, &DiaperDetails::default(), EVENT_TIME)
                .await
        }
        "potty" => {
            client
                .log_potty_at(
                    "c1",
                    DiaperMode::Pee,
                    huckleberry_api::models::diaper::PottyResult::WentPotty,
                    &DiaperDetails::default(),
                    EVENT_TIME,
                )
                .await
        }
        "growth" => {
            client
                .log_growth_at(
                    "c1",
                    &GrowthMeasurements {
                        weight: Some(3.4),
                        ..GrowthMeasurements::default()
                    },
                    MeasurementSystem::Metric,
                    EVENT_TIME,
                )
                .await
        }
        _ => unreachable!(),
    }
    .expect("recorded");
}

#[tokio::test]
async fn event_time_reaches_history_ids_offsets_and_summaries() {
    for (kind, summary) in [
        ("bottle", "lastBottle"),
        ("solids", "lastSolid"),
        ("diaper", "lastDiaper"),
        ("potty", "lastPotty"),
        ("growth", "lastGrowthEntry"),
    ] {
        let stub = Stub::start(vec![
            document("tracker/c1", &json!({})),
            json!({}),
            json!({}),
        ])
        .await;
        let before = huckleberry_api::client::now_seconds();
        record_event(&client(&stub), kind).await;
        let requests = stub.requests().await;
        let row = requests
            .iter()
            .find(|request| {
                request.method == "PATCH"
                    && (request.path.contains("/intervals/") || request.path.contains("/data/"))
            })
            .unwrap();
        let body = row.document();
        assert_eq!(body["start"], json!(EVENT_TIME), "{kind}");
        assert_eq!(body["offset"], json!(300.0), "winter in New York");
        assert!(body["lastUpdated"].as_f64().unwrap() >= before);
        assert!(row.path.contains("1735732800000-"), "{}", row.path);
        let prefs = requests.last().unwrap().document();
        assert_eq!(
            prefs["prefs"][summary]["start"],
            json!(EVENT_TIME),
            "{kind}"
        );
        assert!(prefs["prefs"]["local_timestamp"].as_f64().unwrap() >= before);
    }
}

#[tokio::test]
async fn backdated_events_do_not_replace_newer_summaries() {
    for (kind, summary) in [
        ("bottle", "lastBottle"),
        ("solids", "lastSolid"),
        ("diaper", "lastDiaper"),
        ("potty", "lastPotty"),
        ("growth", "lastGrowthEntry"),
    ] {
        let stub = Stub::start(vec![
            document(
                "tracker/c1",
                &json!({"prefs": {summary: {"start": EVENT_TIME + 3600.0}}}),
            ),
            json!({}),
        ])
        .await;
        record_event(&client(&stub), kind).await;
        let writes: Vec<_> = stub
            .requests()
            .await
            .into_iter()
            .filter(|request| request.method == "PATCH")
            .collect();
        assert_eq!(writes.len(), 1, "only history changes for {kind}");
        assert_eq!(writes[0].document()["start"], json!(EVENT_TIME));
    }
}

fn nursing_timer(paused: bool) -> serde_json::Value {
    document(
        "feed/c1",
        &json!({"timer": {
            "active": true, "paused": paused, "uuid": "abc",
            "feedStartTime": EVENT_TIME, "timerStartTime": EVENT_TIME + 60.0,
            "leftDuration": 60.0, "rightDuration": 0.0,
            "activeSide": "right", "lastSide": "left"
        }}),
    )
}

#[tokio::test]
async fn nursing_start_uses_event_time_but_current_sync_time() {
    let stub = Stub::start(vec![json!({})]).await;
    let before = huckleberry_api::client::now_seconds();
    client(&stub)
        .start_nursing_at("c1", FeedSide::Left, EVENT_TIME)
        .await
        .unwrap();
    let body = stub.request(0).await.document();
    assert_eq!(body["timer"]["feedStartTime"], json!(EVENT_TIME));
    assert_eq!(body["timer"]["timerStartTime"], json!(EVENT_TIME));
    assert!(body["timer"]["local_timestamp"].as_f64().unwrap() >= before);
}

#[tokio::test]
async fn nursing_transitions_bank_only_until_the_selected_time() {
    for action in ["pause", "switch", "stop", "resume"] {
        let stub = Stub::start(vec![
            nursing_timer(action == "resume"),
            json!({}),
            json!({}),
        ])
        .await;
        let client = client(&stub);
        let at = EVENT_TIME + 180.0;
        match action {
            "pause" => {
                client.pause_nursing_at("c1", at).await.unwrap();
            }
            "switch" => {
                client.switch_nursing_side_at("c1", at).await.unwrap();
            }
            "resume" => {
                client.resume_nursing_at("c1", None, at).await.unwrap();
            }
            "stop" => {
                let completed = client.complete_nursing_at("c1", at).await.unwrap().unwrap();
                assert!((completed.total_seconds() - 180.0).abs() < f64::EPSILON);
            }
            _ => unreachable!(),
        }
        let body = stub.request(1).await.document();
        if action == "stop" {
            assert_eq!(body["rightDuration"], json!(120.0));
        } else if action == "resume" {
            assert_eq!(body["timer"]["timerStartTime"], json!(at));
        } else {
            assert_eq!(body["timer"]["rightDuration"], json!(120.0));
        }
    }
}

#[tokio::test]
async fn nursing_cannot_transition_before_its_current_segment() {
    for action in ["pause", "switch", "stop", "resume"] {
        let stub = Stub::start(vec![nursing_timer(action == "resume")]).await;
        let client = client(&stub);
        let at = EVENT_TIME + 30.0;
        let failed = match action {
            "pause" => client.pause_nursing_at("c1", at).await.is_err(),
            "switch" => client.switch_nursing_side_at("c1", at).await.is_err(),
            "stop" => client.complete_nursing_at("c1", at).await.is_err(),
            "resume" => client.resume_nursing_at("c1", None, at).await.is_err(),
            _ => unreachable!(),
        };
        assert!(failed, "{action}");
        assert_eq!(stub.requests().await.len(), 1);
    }
}

#[tokio::test]
async fn sleep_pause_and_stop_use_the_selected_time_in_milliseconds_and_seconds() {
    for action in ["pause", "stop"] {
        let stub = Stub::start(vec![document("sleep/c1", &json!({"timer": {
            "active": true, "paused": false, "uuid": "abc", "timerStartTime": EVENT_TIME * 1000.0
        }})), json!({}), json!({})]).await;
        let client = client(&stub);
        if action == "pause" {
            client
                .pause_sleep_at("c1", EVENT_TIME + 180.0)
                .await
                .unwrap();
            assert_eq!(
                stub.request(1).await.document()["timer"]["timerEndTime"],
                json!((EVENT_TIME + 180.0) * 1000.0)
            );
        } else {
            let completed = client
                .complete_sleep_at("c1", EVENT_TIME + 180.0)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(completed.duration, 180);
        }
    }
}

#[tokio::test]
async fn finishing_backdated_nursing_keeps_newer_feed_summaries() {
    let mut timer = nursing_timer(false);
    let mut fields =
        huckleberry_api::firestore::value::fields_to_json(timer["fields"].as_object().unwrap());
    fields.insert("prefs".into(), json!({"lastNursing": {"start": EVENT_TIME + 3600.0}, "lastSide": {"start": EVENT_TIME + 3600.0, "lastSide": "left"}}));
    timer = document("feed/c1", &json!(fields));
    let stub = Stub::start(vec![timer, json!({}), json!({})]).await;
    client(&stub)
        .complete_nursing_at("c1", EVENT_TIME + 180.0)
        .await
        .unwrap();
    let mask = stub.request(2).await.update_mask();
    assert!(
        !mask
            .iter()
            .any(|field| field == "prefs.lastNursing" || field == "prefs.lastSide")
    );
    assert_eq!(
        stub.request(2).await.document()["timer"]["active"],
        json!(false)
    );
}

#[tokio::test]
async fn a_sleep_cannot_end_before_a_resume_or_fallback_start() {
    for timer in [
        json!({"active": true, "paused": false, "uuid": "abc", "timerStartTime": EVENT_TIME * 1000.0, "timerEndTime": (EVENT_TIME + 120.0) * 1000.0}),
        json!({"active": true, "paused": false, "uuid": "abc", "timestamp": {"seconds": EVENT_TIME + 120.0}}),
    ] {
        let stub = Stub::start(vec![document("sleep/c1", &json!({"timer": timer}))]).await;
        assert!(
            client(&stub)
                .complete_sleep_at("c1", EVENT_TIME + 60.0)
                .await
                .is_err()
        );
        assert_eq!(stub.requests().await.len(), 1);
    }
}

#[tokio::test]
async fn sleep_resume_retains_its_time_as_the_next_transition_boundary() {
    let stub = Stub::start(vec![document("sleep/c1", &json!({"timer": {
        "active": true, "paused": true, "uuid": "abc", "timerStartTime": EVENT_TIME * 1000.0,
        "timerEndTime": (EVENT_TIME + 60.0) * 1000.0
    }})), json!({})]).await;
    client(&stub)
        .resume_sleep_at("c1", EVENT_TIME + 120.0)
        .await
        .unwrap();
    assert_eq!(
        stub.request(1).await.document()["timer"]["timerEndTime"],
        json!((EVENT_TIME + 120.0) * 1000.0)
    );
}

#[tokio::test]
async fn a_paused_sleep_uses_its_actual_end_offset_not_the_stop_write_time() {
    let stub = Stub::start(vec![document("sleep/c1", &json!({"timer": {
        "active": true, "paused": true, "uuid": "abc", "timerStartTime": EVENT_TIME * 1000.0,
        "timerEndTime": (EVENT_TIME + 60.0) * 1000.0
    }})), json!({}), json!({})]).await;
    // July is daylight time, but the recorded sleep ended in January.
    client(&stub)
        .complete_sleep_at("c1", 1_751_371_200.0)
        .await
        .unwrap();
    assert_eq!(stub.request(1).await.document()["end_offset"], json!(300.0));
}
