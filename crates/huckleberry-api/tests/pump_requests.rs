//! Pump requests preserve upstream shapes and independent tracker state.
mod support;

use huckleberry_api::firestore::value::{fields_from_json, fields_to_json};
use huckleberry_api::models::feed::VolumeUnits;
use huckleberry_api::models::pump::{PumpAmounts, PumpDocument};
use huckleberry_api::ops::pump::PumpEntry;
use huckleberry_api::{Error, Huckleberry, RowRef, Session, TimerChange, Zone};
use serde_json::{Value, json};
use support::Stub;

const START: f64 = 1_741_503_000.0;

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
    json!({"name": format!("projects/p/databases/(default)/documents/{path}"),
        "updateTime": "2026-09-01T12:00:00Z",
        "fields": fields_from_json(&fields)})
}

fn timer(paused: bool) -> Value {
    json!({"active":true, "paused":paused, "uuid":"same-session",
        "startTime":START * 1000.0, "endTime":(START + 120.0) * 1000.0,
        "units":"oz", "unknown":{"keep":true}})
}

const fn entry(amounts: PumpAmounts) -> PumpEntry<'static> {
    PumpEntry {
        amounts,
        duration: Some(3600.0),
        units: VolumeUnits::Millilitres,
        notes: Some(" after breakfast "),
    }
}

#[tokio::test]
async fn total_is_split_between_sides_with_event_time_ids_and_offsets() {
    let stub = Stub::start(vec![document("pump/c1", json!({})), json!({})]).await;
    client(&stub)
        .log_pump_at("c1", entry(PumpAmounts::Total(100.0)), START)
        .await
        .unwrap();
    let writes: Vec<_> = stub
        .requests()
        .await
        .into_iter()
        .filter(|r| r.method == "PATCH")
        .collect();
    assert_eq!(writes.len(), 2);
    assert!(writes[0].path.contains("/pump/c1/intervals/1741503000000-"));
    let row = writes[0].document();
    assert_eq!(row["entryMode"], "total");
    assert_eq!(row["leftAmount"], 50.0);
    assert_eq!(row["rightAmount"], 50.0);
    assert_eq!(row["start"], START);
    assert_eq!(row["offset"], 300.0);
    assert_eq!(row["end_offset"], 240.0);
    assert_eq!(row["notes"], "after breakfast");
    assert!(row["lastUpdated"].as_f64().unwrap() > START);
    assert_eq!(
        writes[1].document()["prefs"]["lastPump"]["leftAmount"],
        50.0
    );
    assert!(
        writes[1]
            .update_mask()
            .contains(&"prefs.lastPump".to_owned())
    );
    assert!(
        writes[1]
            .update_mask()
            .iter()
            .all(|path| path.starts_with("prefs."))
    );
}

#[tokio::test]
async fn backdated_sides_preserve_a_newer_summary_and_omit_absent_duration() {
    let stub = Stub::start(vec![
        document("pump/c1", json!({"prefs":{"lastPump":{"start":START+1.0}}})),
        json!({}),
    ])
    .await;
    let mut values = entry(PumpAmounts::LeftRight {
        left: 40.0,
        right: 0.0,
    });
    values.duration = None;
    values.notes = Some(" ");
    client(&stub)
        .log_pump_at("c1", values, START)
        .await
        .unwrap();
    let requests = stub.requests().await;
    assert_eq!(requests.len(), 2);
    let row = requests[1].document();
    assert_eq!(row["entryMode"], "leftright");
    assert_eq!(row["leftAmount"], 40.0);
    assert_eq!(row["rightAmount"], 0.0);
    for field in ["duration", "end_offset", "notes"] {
        assert!(row.get(field).is_none());
    }
}

#[tokio::test]
async fn invalid_amounts_durations_units_and_times_never_touch_the_network() {
    let stub = Stub::start(vec![]).await;
    let api = client(&stub);
    for amounts in [
        PumpAmounts::Total(-1.0),
        PumpAmounts::Total(f64::NAN),
        PumpAmounts::LeftRight {
            left: 1.0,
            right: f64::INFINITY,
        },
    ] {
        assert!(matches!(
            api.log_pump_at("c1", entry(amounts), START).await,
            Err(Error::Invalid(_))
        ));
    }
    for duration in [-1.0, f64::INFINITY, f64::NAN] {
        let mut values = entry(PumpAmounts::Total(1.0));
        values.duration = Some(duration);
        assert!(api.log_pump_at("c1", values, START).await.is_err());
    }
    let mut values = entry(PumpAmounts::Total(1.0));
    values.units = VolumeUnits::Unknown("cup".into());
    assert!(api.log_pump_at("c1", values, START).await.is_err());
    for at in [-1.0, f64::NAN, f64::INFINITY, 9_999_999_999.0] {
        assert!(
            api.log_pump_at("c1", entry(PumpAmounts::Total(1.0)), at)
                .await
                .is_err()
        );
    }
    assert!(stub.requests().await.is_empty());
}

#[tokio::test]
async fn starting_reuses_uuid_and_writes_timer_leaves_only() {
    let stub = Stub::start(vec![
        document(
            "pump/c1",
            json!({"timer":{"active":false,"uuid":"same-session"}}),
        ),
        json!({}),
    ])
    .await;
    assert_eq!(
        client(&stub).start_pump_at("c1", START).await.unwrap(),
        TimerChange::Applied
    );
    let write = stub.request(1).await;
    assert_eq!(write.document()["timer"]["uuid"], "same-session");
    assert_eq!(write.document()["timer"]["startTime"], START * 1000.0);
    assert_eq!(write.document()["timer"]["paused"], false);
    assert!(
        write
            .update_mask()
            .iter()
            .all(|field| field.starts_with("timer."))
    );
    assert!(write.update_mask().contains(&"timer.endTime".into()));
}

#[tokio::test]
async fn timer_transitions_use_milliseconds_and_ignore_repeated_actions() {
    let stub = Stub::start(vec![
        document("pump/c1", json!({"timer":timer(false)})),
        json!({}),
    ])
    .await;
    assert_eq!(
        client(&stub)
            .pause_pump_at("c1", START + 180.0)
            .await
            .unwrap(),
        TimerChange::Applied
    );
    assert_eq!(
        stub.request(1).await.document()["timer"]["endTime"],
        (START + 180.0) * 1000.0
    );
    let paused = Stub::start(vec![
        document("pump/c1", json!({"timer":timer(true)})),
        json!({}),
    ])
    .await;
    assert_eq!(
        client(&paused)
            .pause_pump_at("c1", START + 180.0)
            .await
            .unwrap(),
        TimerChange::Unchanged
    );
    assert_eq!(paused.requests().await.len(), 1);
}

#[tokio::test]
async fn resume_clears_the_paused_endpoint_and_does_not_restart_the_session() {
    let stub = Stub::start(vec![
        document("pump/c1", json!({"timer":timer(true)})),
        json!({}),
    ])
    .await;
    assert_eq!(
        client(&stub)
            .resume_pump_at("c1", START + 180.0)
            .await
            .unwrap(),
        TimerChange::Applied
    );
    let write = stub.request(1).await;
    assert_eq!(write.document()["timer"]["paused"], false);
    assert!(write.document()["timer"].get("endTime").is_none());
    assert!(write.update_mask().contains(&"timer.endTime".into()));
    assert!(!write.update_mask().contains(&"timer.startTime".into()));
}

#[tokio::test]
async fn paused_completion_uses_pause_time_and_timer_units_and_retains_newer_summary() {
    let source = document(
        "pump/c1",
        json!({"timer":timer(true),"prefs":{"lastPump":{"start":START+500.0}}}),
    );
    let stub = Stub::start(vec![source.clone(), source, json!({})]).await;
    let completed = client(&stub)
        .complete_pump_at("c1", PumpAmounts::Total(4.0), None, None, START + 1000.0)
        .await
        .unwrap()
        .unwrap();
    assert!((completed.start - START).abs() < f64::EPSILON);
    assert!((completed.duration - 120.0).abs() < f64::EPSILON);
    let writes: Vec<_> = stub
        .requests()
        .await
        .into_iter()
        .filter(|r| r.method == "PATCH")
        .collect();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0].document()["units"], "oz");
    assert_eq!(writes[0].document()["duration"], 120.0);
    assert_eq!(writes[1].document()["timer"]["active"], false);
    assert_eq!(writes[1].document()["timer"]["uuid"], "same-session");
    assert!(
        writes[1]
            .update_mask()
            .iter()
            .all(|field| field.starts_with("timer."))
    );
    assert!(writes[1].update_mask().contains(&"timer.endTime".into()));
}

#[tokio::test]
async fn timer_rejects_missing_start_and_backward_transitions_without_writes() {
    for timer in [json!({"active":true,"uuid":"x"}), timer(true)] {
        let stub = Stub::start(vec![document("pump/c1", json!({"timer":timer}))]).await;
        assert!(
            client(&stub)
                .complete_pump_at("c1", PumpAmounts::Total(1.0), None, None, START - 1.0)
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
async fn cancellation_does_not_write_history_or_replace_unknown_timer_fields() {
    let stub = Stub::start(vec![
        document("pump/c1", json!({"timer":timer(false)})),
        json!({}),
    ])
    .await;
    assert_eq!(
        client(&stub).cancel_pump("c1").await.unwrap(),
        TimerChange::Applied
    );
    let write = stub.request(1).await;
    assert_eq!(write.path, "/v1/documents/pump/c1");
    assert_eq!(write.document()["timer"]["active"], false);
    assert!(!write.update_mask().contains(&"timer".into()));
    assert!(!write.update_mask().contains(&"timer.unknown".into()));
}

#[test]
fn model_reads_millisecond_timers_and_tolerates_unfamiliar_optional_fields() {
    let document: PumpDocument = serde_json::from_value(json!({"timer":timer(true),
        "prefs":{"lastPump":{"start":START,"leftAmount":"unreadable"}, "reminderV2":42}}))
    .unwrap();
    let timer = document.running_timer().unwrap();
    assert_eq!(timer.started_at(), Some(START));
    assert_eq!(timer.elapsed_seconds(START + 600.0), Some(120.0));
    assert!(
        document
            .prefs
            .unwrap()
            .last_pump
            .unwrap()
            .left_amount
            .is_none()
    );
}

#[tokio::test]
async fn deleting_latest_pump_repairs_its_summary_without_touching_timer() {
    let remaining = json!({"start":START-100.0,"entryMode":"leftright","leftAmount":10.0,"rightAmount":20.0,"units":"ml","offset":300.0});
    let stub = Stub::start(vec![
        json!({}),
        document(
            "pump/c1",
            json!({"timer":timer(false),"prefs":{"lastPump":{"start":START}}}),
        ),
        json!({"documents":[document("pump/c1/intervals/older", remaining.clone())]}),
        json!({}),
    ])
    .await;
    client(&stub)
        .delete_history_row("c1", &RowRef::loose("pump", "latest"), START)
        .await
        .unwrap();
    let write = stub.request(3).await;
    assert_eq!(write.update_mask(), vec!["prefs.lastPump"]);
    assert_eq!(write.document()["prefs"]["lastPump"], remaining);
}

#[tokio::test]
async fn batched_edit_changes_only_selected_fields_and_repairs_last_pump() {
    let row = json!({"start":START,"duration":60.0,"notes":"old","unknown":true});
    let stub = Stub::start(vec![document("pump/c1/intervals/pack", json!({"multi":true,"data":{"a.b":row,"sibling":{"start":START-1.0}}})),json!({}),
        document("pump/c1", json!({"prefs":{"lastPump":{"start":START}}})),
        json!({"documents":[document("pump/c1/intervals/pack", json!({"multi":true,"data":{"a.b":{"start":START,"entryMode":"total","leftAmount":20.0,"rightAmount":20.0,"units":"ml"}}}))]}),json!({})]).await;
    let mut values = entry(PumpAmounts::Total(40.0));
    values.duration = None;
    values.notes = None;
    client(&stub)
        .update_pump_entry("c1", &RowRef::batched("pump", "pack", "a.b"), values)
        .await
        .unwrap();
    let write = stub.request(1).await;
    assert!(
        write
            .update_mask()
            .iter()
            .all(|field| field.starts_with("data.`a.b`."))
    );
    for field in ["duration", "end_offset", "notes"] {
        assert!(write.update_mask().contains(&format!("data.`a.b`.{field}")));
        assert!(write.document()["data"]["a.b"].get(field).is_none());
    }
    assert_eq!(stub.request(4).await.update_mask(), vec!["prefs.lastPump"]);
}

#[tokio::test]
async fn moving_pump_time_atomically_repairs_the_summary() {
    let source = document(
        "pump/c1/intervals/1741503000000-old",
        json!({"start":START,"entryMode":"total","leftAmount":20.0,"rightAmount":20.0,"units":"ml","offset":300.0}),
    );
    let stub = Stub::start(vec![
        source.clone(),
        document("pump/c1", json!({"prefs":{"lastPump":{"start":START}}})),
        json!({"documents":[source]}),
        json!({}),
    ])
    .await;
    client(&stub)
        .update_history_time(
            "c1",
            &RowRef::loose("pump", "1741503000000-old"),
            START + 100.0,
        )
        .await
        .unwrap();
    let commit = stub.request(3).await.body.unwrap();
    let writes = commit["writes"].as_array().unwrap();
    assert_eq!(writes.len(), 3);
    let summary = Value::Object(fields_to_json(
        writes[2]["update"]["fields"].as_object().unwrap(),
    ));
    assert_eq!(summary["prefs"]["lastPump"]["start"], START + 100.0);
}

#[tokio::test]
async fn latest_pump_and_watcher_read_the_typed_tracker() {
    let source = document(
        "pump/c1",
        json!({"timer":timer(false),
        "prefs":{"lastPump":{"start":START,"entryMode":"total","leftAmount":2.0,"rightAmount":2.0,"units":"oz"}}}),
    );
    let stub = Stub::start(vec![source]).await;
    let api = client(&stub);
    assert!(
        (api.latest_pump("c1")
            .await
            .unwrap()
            .unwrap()
            .start
            .unwrap()
            .as_f64()
            - START)
            .abs()
            < f64::EPSILON
    );
    let mut observed_start = None;
    api.watch_pump(
        "c1",
        std::time::Duration::ZERO,
        |document| {
            observed_start = document
                .running_timer()
                .and_then(huckleberry_api::models::PumpTimer::started_at);
            huckleberry_api::Watching::Stop
        },
        |_| huckleberry_api::Watching::Stop,
    )
    .await
    .unwrap();
    assert_eq!(observed_start, Some(START));
    assert!(
        stub.requests()
            .await
            .iter()
            .all(|request| request.path == "/v1/documents/pump/c1")
    );
}

#[tokio::test]
async fn start_and_log_create_missing_tracker_documents() {
    let stub = Stub::start_with_status(vec![
        (404, json!({"error":{"message":"missing"}})),
        (200, json!({})),
    ])
    .await;
    assert_eq!(
        client(&stub).start_pump_at("c1", START).await.unwrap(),
        TimerChange::Applied
    );
    let write = stub.request(1).await;
    assert!(write.query_values("currentDocument.exists").is_empty());
    assert!(write.document()["timer"]["uuid"].as_str().unwrap().len() == 16);
    let stub = Stub::start_with_status(vec![
        (404, json!({"error":{"message":"missing"}})),
        (200, json!({})),
    ])
    .await;
    client(&stub)
        .log_pump_at("c1", entry(PumpAmounts::Total(0.0)), START)
        .await
        .unwrap();
    let summary = stub.request(2).await;
    assert!(summary.query_values("currentDocument.exists").is_empty());
    assert_eq!(summary.document()["prefs"]["lastPump"]["leftAmount"], 0.0);
}

#[tokio::test]
async fn active_start_and_inactive_transitions_write_nothing() {
    let stub = Stub::start(vec![document("pump/c1", json!({"timer":timer(false)}))]).await;
    assert_eq!(
        client(&stub).start_pump_at("c1", START).await.unwrap(),
        TimerChange::Unchanged
    );
    assert_eq!(stub.requests().await.len(), 1);
    let stub = Stub::start(vec![document(
        "pump/c1",
        json!({"timer":{"active":false,"uuid":"same-session"}}),
    )])
    .await;
    let api = client(&stub);
    assert_eq!(api.pause_pump("c1").await.unwrap(), TimerChange::NotRunning);
    assert_eq!(
        api.resume_pump("c1").await.unwrap(),
        TimerChange::NotRunning
    );
    assert_eq!(
        api.cancel_pump("c1").await.unwrap(),
        TimerChange::NotRunning
    );
    assert_eq!(
        api.complete_pump("c1", PumpAmounts::Total(1.0), None, None)
            .await
            .unwrap(),
        None
    );
    assert!(
        stub.requests()
            .await
            .iter()
            .all(|request| request.method == "GET")
    );
}

#[tokio::test]
async fn running_completion_uses_explicit_endpoint_and_does_not_retimestamp_preferences() {
    let source = document(
        "pump/c1",
        json!({"timer":timer(false),"prefs":{"lastPump":{"start":START-1.0}}}),
    );
    let stub = Stub::start(vec![source.clone(), source, json!({})]).await;
    let result = client(&stub)
        .complete_pump_at(
            "c1",
            PumpAmounts::LeftRight {
                left: 30.0,
                right: 20.0,
            },
            Some(VolumeUnits::Millilitres),
            Some(" done "),
            START + 3600.0,
        )
        .await
        .unwrap()
        .unwrap();
    assert!((result.duration - 3600.0).abs() < f64::EPSILON);
    let row = stub.request(2).await.document();
    assert_eq!(row["notes"], "done");
    assert_eq!(row["end_offset"], 240.0);
    assert_eq!(stub.request(3).await.update_mask(), vec!["prefs.lastPump"]);
}

#[tokio::test]
async fn failed_history_write_leaves_the_timer_active() {
    let source = document("pump/c1", json!({"timer":timer(false)}));
    let stub = Stub::start_with_status(vec![
        (200, source.clone()),
        (200, source),
        (403, json!({"error":{"message":"denied"}})),
    ])
    .await;
    assert!(
        client(&stub)
            .complete_pump_at("c1", PumpAmounts::Total(1.0), None, None, START + 120.0)
            .await
            .is_err()
    );
    let requests = stub.requests().await;
    assert_eq!(requests.len(), 3);
    assert!(
        requests
            .iter()
            .filter(|request| request.method == "PATCH")
            .all(|request| request.path.contains("/intervals/"))
    );
}

#[tokio::test]
async fn raw_duration_edit_recomputes_the_endpoint_offset() {
    let stub = Stub::start(vec![
        document("pump/c1/intervals/row", json!({"start":START})),
        json!({}),
        document("pump/c1", json!({})),
        json!({"documents":[]}),
    ])
    .await;
    client(&stub)
        .update_history_row(
            "c1",
            &RowRef::loose("pump", "row"),
            &[huckleberry_api::firestore::FieldUpdate::set(
                "duration",
                json!(3600.0),
            )],
            "editing pump duration",
        )
        .await
        .unwrap();
    let write = stub.request(1).await;
    assert_eq!(write.document()["end_offset"], 240.0);
    assert_eq!(write.update_mask(), vec!["duration", "end_offset"]);
}

#[tokio::test]
async fn deleting_the_only_pump_clears_the_last_summary() {
    let stub = Stub::start(vec![
        json!({}),
        document("pump/c1", json!({"prefs":{"lastPump":{"start":START}}})),
        json!({"documents":[]}),
        json!({}),
    ])
    .await;
    client(&stub)
        .delete_history_row("c1", &RowRef::loose("pump", "row"), START)
        .await
        .unwrap();
    let write = stub.request(3).await;
    assert_eq!(write.update_mask(), vec!["prefs.lastPump"]);
    assert!(write.document().get("prefs").is_none());
}
