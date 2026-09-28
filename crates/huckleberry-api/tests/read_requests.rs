//! What each read actually asks for.
//!
//! A windowed read is two queries, and these pin both: the range query over
//! the loose rows, and the `multi == true` query over the batched documents
//! whose contents Firestore cannot filter inside.

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

use huckleberry_api::{Credentials, Window};

/// A `runQuery` reply holding these documents.
fn query_reply(documents: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::Value::Array(
        documents
            .into_iter()
            .map(|entry| json!({ "document": entry }))
            .collect(),
    )
}

#[tokio::test]
async fn every_request_carries_the_session_token() {
    let stub = Stub::start(vec![document(
        "users/test-user",
        &json!({ "childList": [] }),
    )])
    .await;
    let _ = client(&stub).user().await;
    assert_eq!(
        stub.request(0).await.authorization.as_deref(),
        Some("Bearer test-id-token")
    );
}

#[tokio::test]
async fn the_account_is_read_from_the_signed_in_users_own_document() {
    let stub = Stub::start(vec![document(
        "users/test-user",
        &json!({ "firstname": "Ada", "childList": [{ "cid": "child-one" }] }),
    )])
    .await;

    let user = client(&stub).user().await.expect("the account");
    assert_eq!(user.first_child().expect("a child").cid, "child-one");

    let request = stub.request(0).await;
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, "/v1/documents/users/test-user");
}

#[tokio::test]
async fn a_missing_document_is_an_absence_rather_than_a_failure() {
    // The stub always answers 200, so this instead pins the nearer case: a
    // child document that decodes to an empty profile rather than an error.
    let stub = Stub::start(vec![document("childs/c1", &json!({}))]).await;
    let profile = client(&stub).child("c1").await.expect("a read");
    assert!(profile.expect("a profile").childs_name.is_none());
}

#[tokio::test]
async fn a_windowed_read_asks_twice_and_merges_the_answers() {
    let loose = query_reply(vec![document(
        "sleep/c1/intervals/a",
        &json!({ "start": 300, "duration": 60, "offset": 240 }),
    )]);
    let batched = query_reply(vec![document(
        "sleep/c1/intervals/batch",
        &json!({ "multi": true, "data": {
            "old": { "start": 100, "duration": 30, "offset": 240 },
            "outside": { "start": 9_999, "duration": 30, "offset": 240 },
        } }),
    )]);
    let stub = Stub::start(vec![loose, batched]).await;

    let sleeps = client(&stub)
        .sleep_intervals("c1", Window::new(0, 1_000))
        .await
        .expect("the read");

    assert_eq!(
        sleeps.len(),
        2,
        "the loose row and the one batched row in range"
    );
    assert_eq!(sleeps[0].row.start.as_i64(), 100, "oldest first");
    assert_eq!(sleeps[1].row.start.as_i64(), 300);

    let ranged = stub.request(0).await;
    assert_eq!(ranged.method, "POST");
    assert_eq!(ranged.path, "/v1/documents/sleep/c1:runQuery");
    let clause = &ranged.body.expect("a query body")["structuredQuery"];
    assert_eq!(clause["from"], json!([{ "collectionId": "intervals" }]));
    assert_eq!(
        clause["orderBy"],
        json!([{ "field": { "fieldPath": "start" }, "direction": "ASCENDING" }])
    );

    let multi_query = stub.request(1).await;
    let filter =
        &multi_query.body.expect("a query body")["structuredQuery"]["where"]["fieldFilter"];
    assert_eq!(filter["field"]["fieldPath"], json!("multi"));
    assert_eq!(filter["op"], json!("EQUAL"));
}

#[tokio::test]
async fn a_row_the_client_cannot_read_does_not_cost_the_caller_the_rest() {
    let stub = Stub::start(vec![
        query_reply(vec![
            document(
                "d/1",
                &json!({ "mode": "pee", "start": 100, "offset": 240 }),
            ),
            document("d/2", &json!({ "start": 200 })),
            document(
                "d/3",
                &json!({ "mode": "poo", "start": 300, "offset": 240 }),
            ),
        ]),
        query_reply(vec![]),
    ])
    .await;

    let diapers = client(&stub)
        .diaper_intervals("c1", Window::new(0, 1_000))
        .await
        .expect("the read");
    assert_eq!(diapers.len(), 2);
}

#[tokio::test]
async fn a_raw_collection_read_expands_the_batches_and_sorts_them() {
    let stub = Stub::start(vec![json!({ "documents": [
        document("pump/c1/intervals/one", &json!({ "start": 500 })),
        document("pump/c1/intervals/batch", &json!({ "multi": true, "data": {
            "a": { "start": 100 },
            "b": { "start": 900 },
        } })),
    ] })])
    .await;

    let rows = client(&stub)
        .collection_rows("pump", "c1", "reading pumping history")
        .await
        .expect("the read");
    let starts: Vec<f64> = rows
        .iter()
        .filter_map(|row| row.get("start").and_then(serde_json::Value::as_f64))
        .collect();
    assert_eq!(starts, vec![100.0, 500.0, 900.0]);

    let request = stub.request(0).await;
    assert_eq!(request.method, "GET");
    assert_eq!(request.path, "/v1/documents/pump/c1/intervals");
}

#[tokio::test]
async fn a_client_with_no_credentials_and_no_session_never_opens_a_socket() {
    let stub = Stub::start(vec![json!({})]).await;
    let anonymous = Huckleberry::with_parts(
        reqwest::Client::new(),
        stub.documents_url.clone(),
        None,
        Zone::utc(),
        None,
    );
    assert!(anonymous.user().await.is_err());
    assert!(stub.requests().await.is_empty());
}

#[tokio::test]
async fn credentials_are_never_put_in_a_url_or_a_firestore_request() {
    let stub = Stub::start(vec![document(
        "users/test-user",
        &json!({ "childList": [] }),
    )])
    .await;
    let signed_in = client(&stub).with_credentials(Credentials::new("a@b.c", "hunter2"));
    let _ = signed_in.user().await;

    let request = stub.request(0).await;
    assert!(!request.path.contains("hunter2"));
    assert!(!format!("{:?}", request.query).contains("hunter2"));
    assert!(!format!("{:?}", request.body).contains("hunter2"));
}

#[tokio::test]
async fn a_windowed_read_says_where_every_row_lives() {
    // What an edit needs and a decoded row does not carry: the document the
    // row is in, and the key it sits under when that document is a batch.
    let loose = query_reply(vec![document(
        "diaper/c1/intervals/row-one",
        &json!({ "mode": "pee", "start": 300, "offset": 240 }),
    )]);
    let batched = query_reply(vec![document(
        "diaper/c1/intervals/pack-one",
        &json!({ "multi": true, "data": {
            "inner": { "mode": "poo", "start": 100, "offset": 240 },
        } }),
    )]);
    let stub = Stub::start(vec![loose, batched]).await;

    let diapers = client(&stub)
        .diaper_intervals("c1", Window::new(0, 1_000))
        .await
        .expect("the read");

    assert_eq!(diapers[0].at.document_id, "pack-one");
    assert_eq!(diapers[0].at.batch_key.as_deref(), Some("inner"));
    assert_eq!(diapers[0].at.tracker, "diaper");
    assert_eq!(diapers[1].at.document_id, "row-one");
    assert_eq!(diapers[1].at.batch_key, None);
}
