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

use core::str::FromStr;

use huckleberry_api::models::diaper::{DiaperAmount, DiaperMode, PooColor};
use huckleberry_api::models::feed::{BottleType, FeedSide, VolumeUnits};
use huckleberry_api::models::health::MeasurementSystem;
use huckleberry_api::{DiaperDetails, GrowthMeasurements, TimerChange};

#[tokio::test]
async fn starting_a_sleep_merges_rather_than_replacing_the_document() {
    let stub = Stub::start(vec![json!({})]).await;
    client(&stub).start_sleep("c1").await.expect("the write");

    let request = stub.request(0).await;
    assert_eq!(request.method, "PATCH");
    assert_eq!(request.path, "/v1/documents/sleep/c1");
    // A merge names every leaf, which is what leaves `prefs` untouched.
    let mask = request.update_mask();
    assert!(mask.contains(&"timer.active".to_owned()), "{mask:?}");
    assert!(mask.contains(&"timer.uuid".to_owned()), "{mask:?}");
    assert!(
        mask.iter().any(|entry| entry.contains("`10-20_minutes`")),
        "the awkward key is backticked in the mask: {mask:?}"
    );
    assert!(
        !mask.iter().any(|entry| entry == "timer"),
        "a bare `timer` in the mask would delete prefs' sibling fields: {mask:?}"
    );

    let document = request.document();
    assert_eq!(document["timer"]["active"], json!(true));
    assert_eq!(document["timer"]["paused"], json!(false));
    let started_ms = document["timer"]["timerStartTime"]
        .as_f64()
        .expect("a start time");
    assert!(
        started_ms > 1_700_000_000_000.0,
        "the sleep timer is in milliseconds, got {started_ms}"
    );
}

#[tokio::test]
async fn pausing_a_sleep_reads_the_timer_then_writes_only_the_timer_fields() {
    let stub = Stub::start(vec![
        document(
            "sleep/c1",
            &json!({ "timer": {
                "active": true, "paused": false, "uuid": "0123456789abcdef",
                "timerStartTime": 1_758_572_400_000.0_f64,
            } }),
        ),
        json!({}),
    ])
    .await;

    let change = client(&stub).pause_sleep("c1").await.expect("the write");
    assert_eq!(change, TimerChange::Applied);

    let write = stub.request(1).await;
    assert_eq!(
        write.update_mask(),
        vec![
            "timer.active",
            "timer.local_timestamp",
            "timer.paused",
            "timer.timerEndTime",
            "timer.timestamp",
        ]
    );
    // An update must not create the document: pausing a sleep that is not
    // there is a mistake worth hearing about.
    assert_eq!(write.query_values("currentDocument.exists"), vec!["true"]);
    assert_eq!(write.document()["timer"]["paused"], json!(true));
}

#[tokio::test]
async fn pausing_when_nothing_is_running_writes_nothing_at_all() {
    let stub = Stub::start(vec![document(
        "sleep/c1",
        &json!({ "timer": { "active": false, "paused": false, "uuid": "abc" } }),
    )])
    .await;

    let change = client(&stub).pause_sleep("c1").await.expect("the read");
    assert_eq!(change, TimerChange::NotRunning);
    assert_eq!(stub.requests().await.len(), 1, "it read, and stopped");
}

#[tokio::test]
async fn pausing_an_already_paused_sleep_writes_nothing_at_all() {
    let stub = Stub::start(vec![document(
        "sleep/c1",
        &json!({ "timer": { "active": true, "paused": true, "uuid": "abc" } }),
    )])
    .await;

    assert_eq!(
        client(&stub).pause_sleep("c1").await.expect("the read"),
        TimerChange::Unchanged
    );
    assert_eq!(stub.requests().await.len(), 1);
}

#[tokio::test]
async fn finishing_a_sleep_writes_the_interval_then_clears_the_timer() {
    let started_ms = (huckleberry_api::client::now_seconds() - 3600.0) * 1000.0;
    let stub = Stub::start(vec![
        document(
            "sleep/c1",
            &json!({ "timer": {
                "active": true, "paused": false, "uuid": "0123456789abcdef",
                "timerStartTime": started_ms,
            } }),
        ),
        json!({}),
        json!({}),
    ])
    .await;

    let completed = client(&stub)
        .complete_sleep("c1")
        .await
        .expect("the write")
        .expect("a completed sleep");
    assert!((3595..=3605).contains(&completed.duration), "{completed:?}");

    let interval = stub.request(1).await;
    assert!(
        interval
            .path
            .starts_with("/v1/documents/sleep/c1/intervals/"),
        "{}",
        interval.path
    );
    assert!(
        interval.update_mask().is_empty(),
        "a history row is written whole, not merged"
    );
    let row = interval.document();
    assert_eq!(row["duration"], json!(completed.duration));
    // New York, so the stored offset is positive: minutes to add to reach UTC.
    assert!(row["offset"].as_f64().expect("an offset") > 0.0);

    let cleared = stub.request(2).await;
    assert_eq!(cleared.path, "/v1/documents/sleep/c1");
    assert_eq!(cleared.document()["timer"]["active"], json!(false));
    assert_eq!(
        cleared.document()["timer"]["uuid"],
        json!("0123456789abcdef"),
        "the session id survives so the app can match the entry"
    );
    assert!(
        cleared
            .update_mask()
            .contains(&"prefs.lastSleep".to_owned())
    );
}

#[tokio::test]
async fn finishing_a_nursing_session_banks_the_running_side_and_clears_the_durations() {
    let now = huckleberry_api::client::now_seconds();
    let stub = Stub::start(vec![
        document(
            "feed/c1",
            &json!({ "timer": {
                "active": true, "paused": false, "uuid": "0123456789abcdef",
                "feedStartTime": now - 600.0,
                "timerStartTime": now - 120.0,
                "leftDuration": 300.0, "rightDuration": 0.0,
                "activeSide": "right",
            } }),
        ),
        json!({}),
        json!({}),
    ])
    .await;

    let completed = client(&stub)
        .complete_nursing("c1")
        .await
        .expect("the write")
        .expect("a completed feed");
    assert!((completed.left_seconds - 300.0).abs() < 1.0);
    assert!((completed.right_seconds - 120.0).abs() < 1.0);
    assert_eq!(completed.last_side, FeedSide::Right);

    let row = stub.request(1).await.document();
    assert_eq!(row["mode"], json!("breast"));
    assert_eq!(row["lastSide"], json!("right"));

    let cleared = stub.request(2).await;
    let mask = cleared.update_mask();
    for deleted in [
        "timer.activeSide",
        "timer.leftDuration",
        "timer.rightDuration",
    ] {
        assert!(
            mask.contains(&deleted.to_owned()),
            "{deleted} should be in {mask:?}"
        );
    }
    let body = cleared.document();
    assert!(
        body["timer"].get("leftDuration").is_none(),
        "a deleted field is in the mask and not in the body"
    );
}

#[tokio::test]
async fn a_bottle_is_written_as_a_row_and_as_the_next_default() {
    let stub = Stub::start(vec![json!({}), json!({})]).await;
    client(&stub)
        .log_bottle("c1", 90.0, BottleType::Formula, VolumeUnits::Millilitres)
        .await
        .expect("the write");

    let row = stub.request(0).await;
    assert!(row.path.starts_with("/v1/documents/feed/c1/intervals/"));
    let interval = row.document();
    assert_eq!(interval["mode"], json!("bottle"));
    assert_eq!(interval["amount"], json!(90.0));
    // The row spells it `amount`/`units`; the summary spells the same two
    // numbers `bottleAmount`/`bottleUnits`.
    assert_eq!(interval["units"], json!("ml"));

    let prefs = stub.request(1).await;
    assert_eq!(prefs.path, "/v1/documents/feed/c1");
    let mask = prefs.update_mask();
    assert!(mask.contains(&"prefs.bottleAmount".to_owned()), "{mask:?}");
    assert!(
        mask.contains(&"prefs.lastBottle.start".to_owned()),
        "{mask:?}"
    );
    assert_eq!(
        prefs.document()["prefs"]["lastBottle"]["bottleAmount"],
        json!(90.0)
    );
}

#[tokio::test]
async fn a_nappy_writes_only_what_was_recorded() {
    let stub = Stub::start(vec![json!({}), json!({})]).await;
    client(&stub)
        .log_diaper(
            "c1",
            DiaperMode::Both,
            &DiaperDetails {
                poo_amount: Some(DiaperAmount::Medium),
                color: Some(PooColor::from_str("yellow").expect("a real colour")),
                rash: false,
                ..DiaperDetails::default()
            },
        )
        .await
        .expect("the write");

    let row = stub.request(0).await.document();
    assert_eq!(row["mode"], json!("both"));
    assert_eq!(row["quantity"]["poo"], json!(50.0));
    assert!(
        row["quantity"].get("pee").is_none(),
        "nothing was said about wet"
    );
    assert_eq!(row["color"], json!("yellow"));
    assert!(
        row.get("diaperRash").is_none(),
        "no rash means no field, not a stored false"
    );
    assert!(row.get("isPotty").is_none(), "a nappy is not a potty trip");

    // The summary is replaced whole rather than merged field by field, which
    // is what stops a `pee` summary keeping the colour of the `poo` before it.
    assert_eq!(
        stub.request(1).await.update_mask(),
        vec![
            "prefs.lastDiaper",
            "prefs.local_timestamp",
            "prefs.timestamp",
        ]
    );
}

#[tokio::test]
async fn a_potty_trip_goes_in_the_same_collection_and_says_so() {
    let stub = Stub::start(vec![json!({}), json!({})]).await;
    client(&stub)
        .log_potty(
            "c1",
            DiaperMode::Pee,
            huckleberry_api::models::diaper::PottyResult::WentPotty,
            &DiaperDetails::default(),
        )
        .await
        .expect("the write");

    let row = stub.request(0).await;
    assert!(row.path.starts_with("/v1/documents/diaper/c1/intervals/"));
    assert_eq!(row.document()["isPotty"], json!(true));
    assert_eq!(row.document()["howItHappened"], json!("wentPotty"));

    let mask = stub.request(1).await.update_mask();
    assert!(
        mask.iter()
            .any(|entry| entry.starts_with("prefs.lastPotty")),
        "{mask:?}"
    );
}

#[tokio::test]
async fn growth_goes_to_the_data_subcollection_not_intervals() {
    let stub = Stub::start(vec![json!({}), json!({})]).await;
    client(&stub)
        .log_growth(
            "c1",
            &GrowthMeasurements {
                weight: Some(3.4),
                ..GrowthMeasurements::default()
            },
            MeasurementSystem::Metric,
        )
        .await
        .expect("the write");

    let row = stub.request(0).await;
    assert!(
        row.path.starts_with("/v1/documents/health/c1/data/"),
        "health is the tracker that uses `data`: {}",
        row.path
    );
    let entry = row.document();
    assert_eq!(entry["mode"], json!("growth"));
    assert_eq!(entry["weightUnits"], json!("kg"));
    assert!(entry.get("height").is_none(), "only what was measured");
}

#[tokio::test]
async fn growth_with_nothing_measured_is_refused_before_anything_is_sent() {
    let stub = Stub::start(vec![json!({})]).await;
    let refused = client(&stub)
        .log_growth(
            "c1",
            &GrowthMeasurements::default(),
            MeasurementSystem::Metric,
        )
        .await;
    assert!(refused.is_err());
    assert!(stub.requests().await.is_empty(), "nothing was sent");
}
