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
    json!({ "name": name, "fields": tagged, "updateTime": "2025-09-22T20:20:00.123456Z" })
}

const START: f64 = 1_758_572_400.0;

fn sleep_document(active: bool, paused: bool, uuid: &str) -> serde_json::Value {
    document(
        "sleep/c1",
        &json!({"timer": {
        "active": active, "paused": paused, "uuid": uuid,
        "timerStartTime": START * 1000.0,
        "timerEndTime": (START + 600.0) * 1000.0,
        "details": {"notes": "keep this"}
    }, "prefs": {"lastSleep": {"start": START - 3600.0, "duration": 1200.0}}}),
    )
}

#[tokio::test]
async fn editing_live_sleep_patches_only_start_and_sync_fields() {
    for paused in [false, true] {
        let stub = Stub::start(vec![sleep_document(true, paused, "session"), json!({})]).await;
        let before = huckleberry_api::client::now_seconds();
        client(&stub)
            .update_sleep_start("c1", "session", START - 1200.0)
            .await
            .unwrap();
        let requests = stub.requests().await;
        assert_eq!(
            requests.len(),
            2,
            "one timer read and one patch, never history"
        );
        let write = &requests[1];
        assert_eq!(write.path, "/v1/documents/sleep/c1");
        assert_eq!(
            write.update_mask(),
            vec![
                "timer.local_timestamp",
                "timer.timerStartTime",
                "timer.timestamp"
            ]
        );
        let body = write.document();
        assert_eq!(
            body["timer"]["timerStartTime"],
            json!((START - 1200.0) * 1000.0)
        );
        assert!(body["timer"]["local_timestamp"].as_f64().unwrap() >= before);
        assert_eq!(
            write.query_values("currentDocument.updateTime"),
            vec!["2025-09-22T20:20:00.123456Z"]
        );
    }
}

#[tokio::test]
async fn a_finished_or_replaced_sleep_is_not_restarted_by_an_edit() {
    for (active, uuid) in [(false, "session"), (true, "replacement")] {
        let stub = Stub::start(vec![sleep_document(active, false, uuid)]).await;
        assert!(
            client(&stub)
                .update_sleep_start("c1", "session", START - 60.0)
                .await
                .is_err()
        );
        assert_eq!(stub.requests().await.len(), 1);
    }
}

#[tokio::test]
async fn a_sleep_edit_refuses_future_and_post_pause_starts() {
    for (paused, start) in [
        (false, huckleberry_api::client::now_seconds() + 3600.0),
        (true, START + 601.0),
    ] {
        let stub = Stub::start(vec![sleep_document(true, paused, "session")]).await;
        assert!(
            client(&stub)
                .update_sleep_start("c1", "session", start)
                .await
                .is_err()
        );
        assert!(
            stub.requests()
                .await
                .iter()
                .all(|request| request.method == "GET")
        );
    }
}

#[tokio::test]
async fn keeping_the_start_time_does_not_write() {
    let stub = Stub::start(vec![sleep_document(true, false, "session")]).await;
    assert_eq!(
        client(&stub)
            .update_sleep_start("c1", "session", START)
            .await
            .unwrap(),
        huckleberry_api::TimerChange::Unchanged
    );
    assert_eq!(stub.requests().await.len(), 1);
}

#[tokio::test]
async fn a_live_edit_requires_the_same_firestore_revision() {
    let stub = Stub::start(vec![sleep_document(true, false, "session"), json!({})]).await;
    client(&stub)
        .update_sleep_start("c1", "session", START - 60.0)
        .await
        .unwrap();
    assert_eq!(
        stub.request(1)
            .await
            .query_values("currentDocument.updateTime"),
        vec!["2025-09-22T20:20:00.123456Z"]
    );
}

#[tokio::test]
async fn a_conflicting_sleep_edit_fails_without_retrying_or_finishing() {
    let stub = Stub::start_with_status(vec![
        (200, sleep_document(true, false, "session")),
        (
            409,
            json!({"error": {"message": "document changed", "status": "ABORTED"}}),
        ),
    ])
    .await;
    let error = client(&stub)
        .update_sleep_start("c1", "session", START - 60.0)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        huckleberry_api::Error::Api { status: 409, .. }
    ));
    assert_eq!(
        stub.requests().await.len(),
        2,
        "never retry against the replacement session"
    );
}

#[tokio::test]
async fn missing_revision_refuses_an_unconditional_edit() {
    let mut reply = sleep_document(true, false, "session");
    reply.as_object_mut().unwrap().remove("updateTime");
    let stub = Stub::start(vec![reply]).await;
    assert!(
        client(&stub)
            .update_sleep_start("c1", "session", START - 60.0)
            .await
            .is_err()
    );
    assert_eq!(stub.requests().await.len(), 1);
}
