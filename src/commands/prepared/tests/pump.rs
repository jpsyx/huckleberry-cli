//! Pump request boundaries keep timer semantics and one-time submission intact.
use super::support::{self, Fixture};
use crate::{
    cli::{PumpAction, PumpLogOptions, PumpValues, Units},
    commands::{prepared::PreparedCommand, pump::prepare::prepare_with_client},
    prompt::{flow::FormEvent, host::Input},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::{Value, json};
fn press(form: &mut PreparedCommand, code: KeyCode) -> FormEvent {
    form.apply(
        Input::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        10000.0,
        (80, 24),
    )
}
fn document(value: &Value) -> Value {
    json!({"fields":huckleberry_api::firestore::value::fields_from_json(value.as_object().unwrap())})
}
fn timer(paused: bool) -> Value {
    document(
        &json!({"timer":{"active":true,"paused":paused,"uuid":"test",
    "startTime":8_800_000.0,"endTime":9_000_000.0,"units":"ml"}}),
    )
}
#[test]
fn stop_keeps_timer_start_after_correcting_the_retained_end() {
    let (requests, shown) = support::hosted(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let fixture = Fixture::create_fixture("pump-stop");
            let stub = support::api::Stub::start(vec![
                timer(false),
                timer(false),
                json!({}),
                json!({}),
                json!({}),
            ])
            .await;
            let mut form = prepare_with_client(
                fixture.context(),
                support::client(&stub),
                "test-child".into(),
                PumpAction::Stop {
                    at: None,
                    values: PumpValues {
                        amount: Some(90.0),
                        units: Some(Units::Ml),
                        ..PumpValues::default()
                    },
                },
                10000.0,
            )
            .await?;
            press(&mut form, KeyCode::Enter);
            form.apply(Input::Pasted("kept note".into()), 10000.0, (80, 24));
            press(&mut form, KeyCode::Left);
            for _ in 0..10 {
                press(&mut form, KeyCode::Up);
            }
            press(&mut form, KeyCode::Enter);
            assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
            assert!(stub.requests().await.is_empty());
            form.execute().await?;
            Ok(stub.requests().await)
        })
    });
    let entries = requests
        .iter()
        .filter(|request| request.path.contains("/intervals/"))
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1);
    let row = entries[0].document();
    assert_eq!(row["start"], 8800.0);
    assert_eq!(row["duration"], 600.0);
    assert_eq!(row["notes"], "kept note");
    assert!(shown.iter().any(|line| line == "duration_seconds\t600"));
}
#[test]
fn manual_pump_revisits_read_defaults_once_and_record_custom_duration() {
    let (requests, _) = support::hosted(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
        let fixture = Fixture::create_fixture("pump-log");
        let stub = support::api::Stub::start(vec![document(&json!({"prefs":{"lastPump":{"leftAmount":45.0,"rightAmount":45.0,"units":"ml"}}})),
            document(&json!({})),json!({}),json!({})]).await;
        let mut form = prepare_with_client(fixture.context(),support::client(&stub),"test-child".into(),PumpAction::Log {
            options:PumpLogOptions::default() },10000.0).await?;
        for _ in 0..3 { press(&mut form,KeyCode::Enter); }
        press(&mut form,KeyCode::Char('8'));
        press(&mut form,KeyCode::Enter);
        form.apply(Input::Pasted("1.25".into()),10000.0,(80,24));
        press(&mut form,KeyCode::Left);
        press(&mut form,KeyCode::Enter);
        press(&mut form,KeyCode::Enter);
        assert_eq!(press(&mut form,KeyCode::Enter),FormEvent::Ready);
        assert_eq!(stub.requests().await.len(),1);
        form.execute().await?;
        Ok(stub.requests().await)
    })
    });
    let rows = requests
        .iter()
        .filter(|request| request.path.contains("/intervals/"))
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 1);
    let row = rows[0].document();
    assert_eq!(row["duration"], 75.0);
    assert_eq!(row["leftAmount"], 45.0);
    assert_eq!(row["rightAmount"], 45.0);
}
#[test]
fn pump_timer_actions_do_not_write_until_the_form_is_ready() {
    for name in ["start", "pause", "resume", "cancel"] {
        let (requests, _) = support::hosted(move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async move {
                    let fixture = Fixture::create_fixture(&format!("pump-{name}"));
                    let initial = if name == "start" {
                        document(&json!({}))
                    } else {
                        timer(name == "resume")
                    };
                    let stub = support::api::Stub::start(vec![initial, json!({})]).await;
                    let action = match name {
                        "start" => PumpAction::Start { start: None },
                        "pause" => PumpAction::Pause { at: None },
                        "resume" => PumpAction::Resume { at: None },
                        _ => PumpAction::Cancel,
                    };
                    let mut form = prepare_with_client(
                        fixture.context(),
                        support::client(&stub),
                        "test-child".into(),
                        action,
                        10000.0,
                    )
                    .await?;
                    if name == "cancel" {
                        assert!(form.is_ready());
                    } else {
                        assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
                    }
                    assert!(stub.requests().await.is_empty());
                    form.execute().await?;
                    Ok(stub.requests().await)
                })
        });
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.method == "PATCH")
                .count(),
            1,
            "{name}"
        );
    }
}
