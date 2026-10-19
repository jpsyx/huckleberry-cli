//! Reopening completed sleeps never leaves both history and a running timer.
mod support;

use huckleberry_api::{Huckleberry, RowRef, Session, Zone, firestore::value};
use serde_json::{Value, json};
use support::Stub;

const REVISION: &str = "2026-10-19T10:00:00.000000Z";
const START: f64 = 1000.125;
const END: f64 = START + 600.0;
const TRACKERS: [&str; 6] = [
    "feed",
    "diaper",
    "health",
    "pump",
    "milestones",
    "activities",
];

fn client(stub: &Stub) -> Huckleberry {
    Huckleberry::with_parts(
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
    )
}
fn document(name: &str, fields: Value) -> Value {
    let Value::Object(fields) = fields else {
        panic!("expected fields")
    };
    json!({"name":name,"updateTime":REVISION,
        "fields":value::fields_from_json(&fields)})
}
fn sleep() -> Value {
    json!({"start":START,"duration":600.0,"offset":0,
        "details":{"notes":"still sleeping","location":"crib","unknown":{"keep":true}}})
}
fn source(batched: bool) -> Value {
    if batched {
        document(
            "sleep/c1/intervals/pack",
            json!({"multi":true,"data":{
                "a.b":sleep(),"neighbor":{"start":100.0,"duration":100.0,"offset":0}
            }}),
        )
    } else {
        document("sleep/c1/intervals/latest", sleep())
    }
}
fn target(batched: bool) -> RowRef {
    if batched {
        RowRef::batched("sleep", "pack", "a.b")
    } else {
        RowRef::loose("sleep", "latest")
    }
}
fn reads(batched: bool) -> Vec<Value> {
    let mut replies = vec![
        document("sleep/c1", json!({"timer":{"active":false,"uuid":"old"}})),
        json!({"documents":[source(batched)]}),
    ];
    replies.extend(TRACKERS.map(|_| json!({"documents":[]})));
    replies.push(document("feed/c1", json!({"timer":{"active":false}})));
    replies.push(document("pump/c1", json!({"timer":{"active":false}})));
    replies
}
fn decoded(write: &Value) -> Value {
    Value::Object(value::fields_to_json(
        write["update"]["fields"].as_object().unwrap(),
    ))
}

#[tokio::test]
async fn only_the_latest_sleep_without_later_events_is_offered() {
    let stub = Stub::start(reads(false)).await;
    assert_eq!(
        client(&stub).resumable_sleep("c1").await.unwrap(),
        Some(target(false))
    );
    assert!(
        stub.requests()
            .await
            .iter()
            .all(|request| request.method == "GET")
    );
}

#[tokio::test]
async fn every_history_tracker_blocks_resume_after_the_sleep_end() {
    for (index, tracker) in TRACKERS.iter().enumerate() {
        let mut replies = reads(false);
        replies[index + 2] = json!({"documents":[document(&format!("{tracker}/c1/intervals/later"),
            json!({"start":END + 1.0,"mode":"anything"}))]});
        let stub = Stub::start(replies).await;
        assert_eq!(
            client(&stub).resumable_sleep("c1").await.unwrap(),
            None,
            "{tracker}"
        );
    }
}

#[tokio::test]
async fn events_at_or_before_the_end_do_not_block_but_later_interval_ends_do() {
    for (event, allowed) in [
        (json!({"start":END}), true),
        (json!({"start":START - 5.0}), true),
        (json!({"start":START,"duration":601.0}), false),
        (
            json!({"start":START,"leftDuration":400.0,"rightDuration":201.0}),
            false,
        ),
        (json!({"start":"unknown"}), false),
    ] {
        let mut replies = reads(false);
        replies[2] = json!({"documents":[document("feed/c1/intervals/event",event)]});
        let stub = Stub::start(replies).await;
        assert_eq!(
            client(&stub).resumable_sleep("c1").await.unwrap().is_some(),
            allowed
        );
    }
}

#[tokio::test]
async fn active_or_paused_timers_block_reopening() {
    for (index, tracker) in [(0, "sleep"), (8, "feed"), (9, "pump")] {
        for paused in [false, true] {
            let mut replies = reads(false);
            replies[index] = document(
                &format!("{tracker}/c1"),
                json!({"timer":{"active":true,"paused":paused}}),
            );
            let stub = Stub::start(replies).await;
            assert_eq!(client(&stub).resumable_sleep("c1").await.unwrap(), None);
        }
    }
}

#[tokio::test]
async fn reopening_atomically_removes_only_the_selected_row_and_restores_its_timer() {
    for batched in [false, true] {
        let mut replies = reads(batched);
        replies.extend([source(batched), json!({})]);
        let stub = Stub::start(replies).await;
        client(&stub)
            .resume_completed_sleep("c1", &target(batched))
            .await
            .unwrap();
        let requests = stub.requests().await;
        let mutations: Vec<_> = requests.iter().filter(|r| r.method != "GET").collect();
        assert_eq!(mutations.len(), 1);
        assert!(mutations[0].path.ends_with(":commit"));
        let writes = mutations[0].body.as_ref().unwrap()["writes"]
            .as_array()
            .unwrap();
        assert_eq!(writes.len(), 2);
        for write in writes {
            assert_eq!(write["currentDocument"]["updateTime"], REVISION);
        }
        let timer = decoded(&writes[1]);
        assert_eq!(timer["timer"]["active"], true);
        assert_eq!(timer["timer"]["paused"], false);
        assert_eq!(timer["timer"]["timerStartTime"], START * 1000.0);
        assert_eq!(timer["timer"]["details"], sleep()["details"]);
        assert!(
            timer["timer"]["uuid"]
                .as_str()
                .is_some_and(|id| !id.is_empty())
        );
        assert!(timer["timer"].get("timerEndTime").is_none());
        if batched {
            assert_eq!(writes[0]["updateMask"]["fieldPaths"], json!(["data.`a.b`"]));
            assert_eq!(timer["prefs"]["lastSleep"]["start"], 100.0);
        } else {
            assert_eq!(writes[0]["delete"], "sleep/c1/intervals/latest");
            assert!(timer["prefs"].get("lastSleep").is_none());
        }
        assert!(
            writes[1]["updateMask"]["fieldPaths"]
                .as_array()
                .unwrap()
                .contains(&json!("prefs.lastSleep"))
        );
    }
}

#[tokio::test]
async fn saving_rechecks_eligibility_and_refuses_an_older_selected_sleep() {
    for later in [false, true] {
        let mut replies = reads(false);
        if later {
            replies[3] = json!({"documents":[document("diaper/c1/intervals/later",json!({"start":END+1.0}))]});
        }
        let stub = Stub::start(replies).await;
        let at = if later {
            target(false)
        } else {
            RowRef::loose("sleep", "older")
        };
        assert!(
            client(&stub)
                .resume_completed_sleep("c1", &at)
                .await
                .is_err()
        );
        assert!(stub.requests().await.iter().all(|r| r.method == "GET"));
    }
}

#[tokio::test]
async fn a_changed_source_or_missing_revision_never_causes_an_unconditional_write() {
    for missing_revision in [false, true] {
        let mut replies = reads(false);
        let mut changed = source(false);
        if missing_revision {
            changed.as_object_mut().unwrap().remove("updateTime");
        } else {
            changed["fields"]["duration"] = value::from_json(&json!(700.0));
        }
        replies.push(changed);
        let stub = Stub::start(replies).await;
        assert!(
            client(&stub)
                .resume_completed_sleep("c1", &target(false))
                .await
                .is_err()
        );
        assert!(stub.requests().await.iter().all(|r| r.method == "GET"));
    }
}

#[tokio::test]
async fn a_conflicting_commit_is_not_retried() {
    let mut replies: Vec<_> = reads(false).into_iter().map(|reply| (200, reply)).collect();
    replies.extend([
        (200, source(false)),
        (409, json!({"error":{"message":"changed"}})),
    ]);
    let stub = Stub::start_with_status(replies).await;
    assert!(
        client(&stub)
            .resume_completed_sleep("c1", &target(false))
            .await
            .is_err()
    );
    assert_eq!(
        stub.requests()
            .await
            .iter()
            .filter(|r| r.method == "POST")
            .count(),
        1
    );
}

#[tokio::test]
async fn malformed_timer_or_history_batch_cannot_prove_eligibility() {
    let mut malformed_timer = reads(false);
    malformed_timer[0] = document("sleep/c1", json!({"timer":{"uuid":"unknown"}}));
    let stub = Stub::start(malformed_timer).await;
    assert!(client(&stub).resumable_sleep("c1").await.unwrap().is_none());
    let mut malformed_batch = reads(false);
    malformed_batch[3] = json!({"documents":[document("diaper/c1/intervals/pack",json!({"multi":true,"data":"unreadable"}))]});
    let stub = Stub::start(malformed_batch).await;
    assert!(client(&stub).resumable_sleep("c1").await.is_err());
}

#[tokio::test]
async fn history_pagination_does_not_hide_a_later_event() {
    let mut replies = reads(false);
    replies[3] = json!({"documents":[],"nextPageToken":"more"});
    replies.insert(
        4,
        json!({"documents":[document("diaper/c1/intervals/later",json!({"start":END+1.0}))]}),
    );
    let stub = Stub::start(replies).await;
    assert!(client(&stub).resumable_sleep("c1").await.unwrap().is_none());
    assert_eq!(stub.request(4).await.query_values("pageToken"), ["more"]);
}

#[tokio::test]
async fn read_failure_is_not_treated_as_empty_history() {
    let mut replies: Vec<_> = reads(false).into_iter().map(|reply| (200, reply)).collect();
    replies[3] = (403, json!({"error":{"message":"forbidden"}}));
    let stub = Stub::start_with_status(replies).await;
    assert!(
        client(&stub)
            .resume_completed_sleep("c1", &target(false))
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

#[tokio::test]
async fn stopping_a_resumed_sleep_retains_raw_details_and_counts_the_gap() {
    let mut original = source(false);
    original["fields"]["details"]["mapValue"]["fields"]["attachment"] =
        json!({"referenceValue":"projects/example/databases/(default)/documents/notes/a"});
    let mut replies = reads(false);
    replies[1] = json!({"documents":[original]});
    replies.extend([original.clone(), json!({})]);
    let resume_stub = Stub::start(replies).await;
    client(&resume_stub)
        .resume_completed_sleep("c1", &target(false))
        .await
        .unwrap();
    let requests = resume_stub.requests().await;
    let commit = requests.last().unwrap().body.as_ref().unwrap();
    let timer = commit["writes"][1]["update"].clone();
    let stop_stub = Stub::start(vec![timer, json!({}), json!({})]).await;
    client(&stop_stub)
        .complete_sleep_at("c1", END + 300.0)
        .await
        .unwrap();
    let saved = stop_stub.request(1).await;
    assert_eq!(
        saved.body.as_ref().unwrap()["fields"]["details"],
        original["fields"]["details"]
    );
    assert_eq!(saved.document()["duration"], 900);
}
