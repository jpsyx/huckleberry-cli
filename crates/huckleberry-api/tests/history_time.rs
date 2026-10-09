//! Time corrections must never lose rows or the fields the app adds to them.
mod support;

use huckleberry_api::firestore::value::{fields_from_json, fields_to_json};
use huckleberry_api::{Huckleberry, RowRef, Session, Zone};
use serde_json::{Value, json};
use support::Stub;

const REVISION: &str = "2026-09-01T12:00:00Z";
const ROOT: &str = "projects/p/databases/(default)/documents";

fn client(stub: &Stub) -> Huckleberry {
    Huckleberry::with_parts(
        reqwest::Client::new(),
        stub.documents_url.clone(),
        None,
        Zone::new("America/New_York").unwrap(),
        Some(Session {
            id_token: "token".into(),
            refresh_token: "refresh".into(),
            user_uid: "user".into(),
            expires_at: i64::MAX,
        }),
    )
}

fn document(path: &str, fields: Value) -> Value {
    let Value::Object(fields) = fields else {
        panic!("a document must be an object");
    };
    json!({"name": format!("{ROOT}/{path}"), "updateTime": REVISION,
        "fields": fields_from_json(&fields)})
}

fn decoded(write: &Value) -> Value {
    Value::Object(fields_to_json(
        write["update"]["fields"].as_object().unwrap(),
    ))
}

fn row() -> Value {
    json!({"start": 1000.0, "mode": "bottle", "offset": 240.0,
        "duration": 120.0, "end_offset": 240.0, "amount": 90.0,
        "units": "ml", "bottleType": "Formula", "unknown": {"keep": [1, 2]}})
}

#[tokio::test]
async fn loose_time_move_is_atomic_and_preserves_unknown_wire_types() {
    let mut source = document("feed/c1/intervals/1000000-abc", row());
    source["fields"]["opaque"] =
        json!({"referenceValue": "projects/p/databases/(default)/documents/x/y"});
    let tracker = document("feed/c1", json!({"timer": {"active": true}, "prefs": {}}));
    let stub = Stub::start(vec![
        source.clone(),
        tracker,
        json!({"documents": [source]}),
        json!({}),
    ])
    .await;
    let result = client(&stub)
        .update_history_time("c1", &RowRef::loose("feed", "1000000-abc"), 2000.0)
        .await
        .unwrap();
    assert_eq!(result, RowRef::loose("feed", "2000000-abc"));
    let requests = stub.requests().await;
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.method != "GET")
            .count(),
        1
    );
    let commit = requests.last().unwrap();
    assert_eq!(commit.path, "/v1/documents:commit");
    let writes = commit.body.as_ref().unwrap()["writes"].as_array().unwrap();
    assert_eq!(writes.len(), 3);
    assert_eq!(writes[0]["currentDocument"], json!({"exists": false}));
    assert_eq!(decoded(&writes[0])["start"], 2000.0);
    assert_eq!(decoded(&writes[0])["duration"], 120.0);
    assert_eq!(decoded(&writes[0])["unknown"], json!({"keep": [1, 2]}));
    assert_eq!(
        writes[0]["update"]["fields"]["opaque"]["referenceValue"],
        "projects/p/databases/(default)/documents/x/y"
    );
    assert_eq!(
        writes[1]["delete"],
        format!("{ROOT}/feed/c1/intervals/1000000-abc")
    );
    assert_eq!(writes[1]["currentDocument"]["updateTime"], REVISION);
    assert_eq!(decoded(&writes[2])["prefs"]["lastBottle"]["start"], 2000.0);
    assert!(decoded(&writes[2]).get("timer").is_none());
    assert_eq!(writes[2]["currentDocument"]["updateTime"], REVISION);
}

#[tokio::test]
async fn batched_move_removes_only_the_selected_key_and_updates_health_identity() {
    let source = document(
        "health/c1/data/pack",
        json!({"multi": true, "data": {
            "1000000-abc": {"start": 1000.0, "mode": "growth", "_id": "1000000-abc", "weight": 5.0},
            "neighbor": {"start": 900.0, "mode": "growth", "weight": 4.0}
        }}),
    );
    let stub = Stub::start(vec![
        source.clone(),
        document("health/c1", json!({})),
        json!({"documents": [source]}),
        json!({}),
    ])
    .await;
    let result = client(&stub)
        .update_history_time(
            "c1",
            &RowRef::batched("health", "pack", "1000000-abc"),
            2000.0,
        )
        .await
        .unwrap();
    assert_eq!(result, RowRef::loose("health", "2000000-abc"));
    let commit = stub.request(3).await.body.unwrap();
    assert_eq!(decoded(&commit["writes"][0])["_id"], "2000000-abc");
    assert_eq!(
        commit["writes"][1]["updateMask"]["fieldPaths"],
        json!(["data.`1000000-abc`"])
    );
    assert_eq!(decoded(&commit["writes"][1]), json!({}));
    assert_eq!(
        decoded(&commit["writes"][2])["prefs"]["lastGrowthEntry"]["_id"],
        "2000000-abc"
    );
}

#[tokio::test]
async fn moving_latest_earlier_reinstates_the_other_latest_row() {
    let source = document("feed/c1/intervals/1000000-abc", row());
    let older = document(
        "feed/c1/intervals/900000-other",
        json!({"start": 900.0, "mode": "bottle", "amount": 50.0}),
    );
    let tracker = document(
        "feed/c1",
        json!({"prefs": {"lastBottle": {"start": 1000.0}}}),
    );
    let stub = Stub::start(vec![
        source.clone(),
        tracker,
        json!({"documents": [source, older]}),
        json!({}),
    ])
    .await;
    client(&stub)
        .update_history_time("c1", &RowRef::loose("feed", "1000000-abc"), 800.0)
        .await
        .unwrap();
    let commit = stub.request(3).await.body.unwrap();
    assert_eq!(
        decoded(&commit["writes"][2])["prefs"]["lastBottle"]["start"],
        900.0
    );
}

#[tokio::test]
async fn unchanged_time_reads_the_row_without_writing() {
    let stub = Stub::start(vec![document("feed/c1/intervals/1000000-abc", row())]).await;
    let at = RowRef::loose("feed", "1000000-abc");
    assert_eq!(
        client(&stub)
            .update_history_time("c1", &at, 1000.0)
            .await
            .unwrap(),
        at
    );
    assert_eq!(stub.requests().await.len(), 1);
}

#[tokio::test]
async fn rejected_commit_has_no_fallback_writes_or_retry() {
    let source = document("pump/c1/intervals/1000000-abc", row());
    let stub = Stub::start_with_status(vec![
        (200, source.clone()),
        (
            200,
            document("pump/c1", json!({"prefs": {"lastPump": {"start": 1000.0}}})),
        ),
        (200, json!({"documents": [source]})),
        (409, json!({"error": {"message": "revision changed"}})),
    ])
    .await;
    assert!(
        client(&stub)
            .update_history_time("c1", &RowRef::loose("pump", "1000000-abc"), 2000.0)
            .await
            .is_err()
    );
    let requests = stub.requests().await;
    assert_eq!(requests.len(), 4);
    assert_eq!(requests[3].method, "POST");
    let writes = &requests[3].body.as_ref().unwrap()["writes"];
    assert_eq!(writes[0]["currentDocument"]["exists"], false);
    assert_eq!(writes[1]["currentDocument"]["updateTime"], REVISION);
}

#[tokio::test]
async fn stable_batched_sleep_id_preserves_duration_and_recalculates_dst_offsets() {
    let source = document(
        "sleep/c1/intervals/pack",
        json!({"multi": true, "data": {
            "stable": {"start": 1000.0, "duration": 7200.0, "offset": 300.0, "end_offset": 300.0}
        }}),
    );
    let stub = Stub::start(vec![
        source.clone(),
        document("sleep/c1", json!({})),
        json!({"documents": [source]}),
        json!({}),
    ])
    .await;
    let at = RowRef::batched("sleep", "pack", "stable");
    assert_eq!(
        client(&stub)
            .update_history_time("c1", &at, 1_741_500_000.0)
            .await
            .unwrap(),
        at
    );
    let commit = stub.request(3).await.body.unwrap();
    let fields = decoded(&commit["writes"][0]);
    assert_eq!(fields["data"]["stable"]["duration"], 7200.0);
    assert_eq!(fields["data"]["stable"]["offset"], 300.0);
    assert_eq!(fields["data"]["stable"]["end_offset"], 240.0);
    assert_eq!(
        commit["writes"][0]["updateMask"]["fieldPaths"],
        json!(["data.stable"])
    );
}

#[tokio::test]
async fn nursing_move_updates_last_side_and_uses_both_sides_for_end_offset() {
    let source = document(
        "feed/c1/intervals/1000000-abc",
        json!({
            "start": 1000.0, "mode": "breast", "leftDuration": 3600.0,
            "rightDuration": 3600.0, "lastSide": "right", "end_offset": 300.0
        }),
    );
    let stub = Stub::start(vec![
        source.clone(),
        document(
            "feed/c1",
            json!({"prefs": {
                "lastSide": {"start": 1000.0, "lastSide": "right"}
            }}),
        ),
        json!({"documents": [source]}),
        json!({}),
    ])
    .await;
    client(&stub)
        .update_history_time("c1", &RowRef::loose("feed", "1000000-abc"), 1_741_500_000.0)
        .await
        .unwrap();
    let commit = stub.request(3).await.body.unwrap();
    assert_eq!(decoded(&commit["writes"][0])["end_offset"], 240.0);
    let summary = decoded(&commit["writes"][2]);
    assert_eq!(
        summary["prefs"]["lastSide"],
        json!({"start": 1_741_500_000.0, "lastSide": "right"})
    );
    assert_eq!(summary["prefs"]["lastNursing"]["duration"], 7200.0);
}

#[tokio::test]
async fn missing_rows_and_missing_revisions_never_write() {
    let mut missing_revision = document("pump/c1/intervals/old", row());
    missing_revision
        .as_object_mut()
        .unwrap()
        .remove("updateTime");
    let cases = [
        (404, json!({}), RowRef::loose("pump", "old")),
        (200, missing_revision, RowRef::loose("pump", "old")),
        (
            200,
            document("pump/c1/intervals/pack", json!({"multi": true, "data": {}})),
            RowRef::batched("pump", "pack", "missing"),
        ),
    ];
    for (status, source, at) in cases {
        let stub = Stub::start_with_status(vec![(status, source)]).await;
        assert!(
            client(&stub)
                .update_history_time("c1", &at, 2000.0)
                .await
                .is_err()
        );
        assert_eq!(stub.requests().await.len(), 1);
    }
}

#[tokio::test]
async fn unsupported_timestamps_are_rejected_before_any_request() {
    let stub = Stub::start(vec![]).await;
    for time in [f64::NAN, f64::INFINITY, -1.0, f64::MAX] {
        assert!(
            client(&stub)
                .update_history_time("c1", &RowRef::loose("pump", "old"), time)
                .await
                .is_err()
        );
    }
    assert!(stub.requests().await.is_empty());
}

#[tokio::test]
async fn opaque_milestone_identifier_is_stable_but_timestamp_identifier_moves() {
    for (old_id, expected_id) in [
        ("first-smile", "first-smile"),
        ("1000000-abc", "2000000-abc"),
    ] {
        let stub = Stub::start(vec![
            document(
                &format!("milestones/c1/intervals/{old_id}"),
                json!({
                    "start": 1000.0, "milestoneId": "catalogue-id", "photo": "keep.jpg"
                }),
            ),
            json!({}),
        ])
        .await;
        let result = client(&stub)
            .update_history_time("c1", &RowRef::loose("milestones", old_id), 2000.0)
            .await
            .unwrap();
        assert_eq!(result.document_id, expected_id);
        let commit = stub.request(1).await.body.unwrap();
        assert_eq!(decoded(&commit["writes"][0])["milestoneId"], "catalogue-id");
        assert_eq!(decoded(&commit["writes"][0])["photo"], "keep.jpg");
    }
}
