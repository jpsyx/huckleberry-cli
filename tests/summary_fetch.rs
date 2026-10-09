//! The summary read must keep sleep carried into the oldest full day.
#[path = "../crates/huckleberry-api/tests/support/mod.rs"]
mod support;

use huckleberry_api::{Huckleberry, Session, Window, Zone};
use serde_json::json;

#[tokio::test]
async fn summary_fetch_includes_sleep_starting_before_the_oldest_day() {
    let fields = huckleberry_api::firestore::value::fields_from_json(
        json!({
            "multi": true, "data": {
                "carry": { "start": 900, "duration": 300, "offset": 0 },
                "boundary": { "start": 800, "duration": 200, "offset": 0 },
                "old": { "start": 100, "duration": 100, "offset": 0 },
                "inside": { "start": 1400, "duration": 100, "offset": 0 }
            }
        })
        .as_object()
        .unwrap(),
    );
    let stub = support::Stub::start(vec![json!([{"document": {
        "name": "sleep/c1/intervals/batch", "fields": fields
    }}])])
    .await;
    let client = Huckleberry::with_parts(
        reqwest::Client::new(),
        stub.documents_url.clone(),
        None,
        Zone::new("UTC").unwrap(),
        Some(Session {
            id_token: "test".into(),
            refresh_token: "test".into(),
            user_uid: "test".into(),
            expires_at: i64::MAX,
        }),
    );
    let data = app::dataset::pull_window(&client, "c1", None, 7, "UTC", Window::new(1000, 2000))
        .await
        .unwrap();
    let starts: Vec<_> = data.sleep.iter().map(|sleep| sleep.start as i64).collect();
    assert_eq!(starts, vec![800, 900, 1400]);
    let requests = stub.requests().await;
    let sleep_query = requests
        .iter()
        .find(|request| {
            request.path.contains("/sleep/c1")
                && request
                    .body
                    .as_ref()
                    .is_some_and(|body| body["structuredQuery"]["orderBy"].is_array())
        })
        .expect("a start-filtered sleep read");
    assert!(
        sleep_query
            .body
            .as_ref()
            .unwrap()
            .to_string()
            .contains("LESS_THAN")
    );
}
