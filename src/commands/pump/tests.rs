//! Request-level checks for the CLI's explicit values and completion receipts.

#[path = "../../../crates/huckleberry-api/tests/support/mod.rs"]
mod support;

use anyhow::Result;
use huckleberry_api::{Huckleberry, Session, Zone};
use serde_json::{Value, json};

use super::logging;
use crate::cli::{PumpLogOptions, PumpValues, Units};
use crate::prompt::host::{self, Reply, Request};
use crate::session::Context;

fn context() -> Context {
    Context {
        config: crate::config::Config::default(),
        config_path: "unused-config.toml".into(),
        credentials_path: "unused-credentials.toml".into(),
        theme: crate::theme::Theme::dark(false),
        verbose: false,
        child_override: None,
        offline: None,
    }
}

fn client(stub: &support::Stub) -> Huckleberry {
    Huckleberry::with_parts(
        reqwest::Client::new(),
        stub.documents_url.clone(),
        None,
        Zone::new("UTC").expect("a zone"),
        Some(Session {
            id_token: "test".into(),
            refresh_token: "test".into(),
            user_uid: "test".into(),
            expires_at: i64::MAX,
        }),
    )
}

fn document(value: &Value) -> Value {
    json!({"fields": huckleberry_api::firestore::value::fields_from_json(value.as_object().expect("an object"))})
}

fn values() -> PumpValues {
    PumpValues {
        amount: Some(90.0),
        units: Some(Units::Ml),
        notes: Some("a note".into()),
        ..PumpValues::default()
    }
}

fn hosted(
    operation: impl FnOnce() -> Result<Vec<support::Recorded>> + Send + 'static,
) -> (Vec<support::Recorded>, Vec<String>) {
    let _serial = host::one_at_a_time();
    let channel = host::install();
    let working = std::thread::spawn(operation);
    let mut shown = Vec::new();
    while !working.is_finished() {
        let Ok((request, reply)) = channel
            .requests
            .recv_timeout(std::time::Duration::from_millis(50))
        else {
            continue;
        };
        match request {
            Request::Show(lines) => {
                shown.extend(lines);
                reply.send(Reply::Shown).expect("output accepted");
            }
            Request::Frame(lines) => panic!("fully flagged pump must not ask: {lines:?}"),
            Request::Step(_) => {
                reply
                    .send(Reply::Stepped { interrupted: true })
                    .expect("drawing");
            }
        }
    }
    let result = working.join().expect("command ends");
    host::remove();
    (result.expect("command succeeds"), shown)
}

#[test]
fn manual_log_preserves_the_explicit_time_and_converts_minutes_to_seconds() {
    let started = recent_clock(0, 0);
    let (requests, shown) = hosted(|| {
        tokio::runtime::Runtime::new()
            .expect("runtime")
            .block_on(async {
                let stub =
                    support::Stub::start(vec![document(&json!({})), json!({}), json!({})]).await;
                let options = PumpLogOptions {
                    at: Some("12am".into()),
                    duration: Some(1.25),
                    values: values(),
                };
                logging::log(&context(), &client(&stub), "c1", &options).await?;
                Ok(stub.requests().await)
            })
    });
    assert_eq!(requests.len(), 3, "explicit values need no defaults read");
    let row = requests
        .iter()
        .find(|request| request.path.contains("/intervals/"))
        .expect("recorded row")
        .document();
    assert_eq!(row["start"], started);
    assert_eq!(row["duration"], 75.0);
    assert_eq!(row["leftAmount"], 45.0);
    assert_eq!(row["rightAmount"], 45.0);
    assert!(
        shown.iter().any(|line| line == "duration_seconds\t75"),
        "{shown:?}"
    );
}

#[test]
fn stopping_records_at_the_timer_start_and_receipts_its_actual_duration() {
    let started = recent_clock(0, 20) - 1200.0;
    let (requests, shown) = hosted(move || {
        tokio::runtime::Runtime::new().expect("runtime").block_on(async {
        let timer = document(&json!({"timer": {"active": true, "paused": false, "startTime": started * 1000.0, "uuid": "pump-session", "units": "oz"}}));
        let stub = support::Stub::start(vec![timer.clone(), timer, json!({}), json!({}), json!({})]).await;
        logging::stop(&context(), &client(&stub), "c1", Some("12:20 am"), &values()).await?;
        Ok(stub.requests().await)
    })
    });
    let row = requests
        .iter()
        .find(|request| request.path.contains("/intervals/"))
        .expect("recorded row")
        .document();
    assert_eq!(row["start"], started);
    assert_eq!(row["duration"], 1200.0);
    assert_eq!(row["units"], "ml");
    assert!(
        shown
            .iter()
            .any(|line| line == &format!("start\t{started}")),
        "{shown:?}"
    );
    assert!(
        shown.iter().any(|line| line == "duration_seconds\t1200"),
        "{shown:?}"
    );
}

fn recent_clock(hour: i8, minute: i8) -> f64 {
    crate::domain::clock::most_recent(
        crate::domain::clock::TimeOfDay::new(hour, minute).expect("a clock time"),
        huckleberry_api::client::now_seconds(),
        &crate::domain::Calendar::new("UTC").expect("a calendar"),
    )
}
