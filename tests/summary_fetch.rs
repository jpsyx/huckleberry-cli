//! Summary reads keep timed activity carried into the oldest full family day.
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

#[tokio::test]
async fn summary_fetch_keeps_only_nursing_overlapping_the_oldest_day() {
    let (stub, client) = feed_history().await;
    let data = app::dataset::pull_window(&client, "c1", None, 7, "UTC", Window::new(1000, 2000))
        .await
        .unwrap();
    let starts: Vec<_> = data.feeds.iter().map(|feed| feed.start() as i64).collect();
    assert_eq!(starts, vec![900, 1000, 1400, 1500]);
    let carry = &data.feeds[0];
    assert_eq!(carry.nursing_seconds(), Some(300.0));
    assert_eq!(carry.at().unwrap().batch_key.as_deref(), Some("carry"));
    assert_eq!(feed_query_start(&stub).await, i64::MIN.to_string());
}

#[tokio::test]
async fn ordinary_pull_keeps_its_start_filtered_feed_window() {
    let (stub, client) = feed_history().await;
    let data = app::dataset::pull(&client, "c1", None, 1, "UTC", 87_400.0)
        .await
        .unwrap();
    let starts: Vec<_> = data.feeds.iter().map(|feed| feed.start() as i64).collect();
    assert_eq!(starts, vec![1000, 1400, 1500, 2000]);
    assert_eq!(feed_query_start(&stub).await, "1000");
}

async fn feed_query_start(stub: &support::Stub) -> String {
    let requests = stub.requests().await;
    let query = requests
        .iter()
        .find(|request| {
            request.path.contains("/feed/c1")
                && request
                    .body
                    .as_ref()
                    .is_some_and(|body| body["structuredQuery"]["orderBy"].is_array())
        })
        .expect("a start-filtered feed read");
    let filters =
        query.body.as_ref().unwrap()["structuredQuery"]["where"]["compositeFilter"]["filters"]
            .as_array()
            .unwrap();
    let lower_bound = filters
        .iter()
        .find(|filter| filter["fieldFilter"]["op"] == "GREATER_THAN_OR_EQUAL")
        .expect("the start lower bound");
    lower_bound["fieldFilter"]["value"]["integerValue"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn feed_history() -> (support::Stub, Huckleberry) {
    let fields = huckleberry_api::firestore::value::fields_from_json(
        json!({ "multi": true, "data": {
            "carry": nursing(900, 100, 200),
            "boundary": nursing(800, 100, 100),
            "old": nursing(100, 100, 100),
            "bottle_old": bottle(950),
            "meal_old": { "mode": "solids", "start": 999, "offset": 0 },
            "start": nursing(1000, 50, 50),
            "inside": nursing(1400, 100, 200),
            "bottle_inside": bottle(1500),
            "end": nursing(2000, 100, 100)
        }})
        .as_object()
        .unwrap(),
    );
    let stub = support::Stub::start(vec![json!([{"document": {
        "name": "feed/c1/intervals/batch", "fields": fields
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
    (stub, client)
}

fn nursing(start: i64, left: i64, right: i64) -> serde_json::Value {
    json!({ "mode": "breast", "start": start, "leftDuration": left,
        "rightDuration": right, "lastSide": "right", "offset": 0 })
}

fn bottle(start: i64) -> serde_json::Value {
    json!({ "mode": "bottle", "start": start, "amount": 90,
        "bottleType": "Formula", "units": "ml", "offset": 0 })
}
