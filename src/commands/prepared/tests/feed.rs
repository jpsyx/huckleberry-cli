//! Default reads occur once; navigating a feeding form never mutates a timer.
use super::support::{self, Fixture};
use crate::{
    cli::{FeedAction, NursingAction},
    commands::{feed::prepare::prepare_with_client, prepared::PreparedCommand},
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
#[test]
fn bottle_revisits_keep_defaults_and_submit_one_entry() {
    let (requests, _) = support::hosted(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
        let fixture = Fixture::create_fixture("bottle");
        let stub = support::api::Stub::start(vec![document(&json!({"prefs":{"bottleAmount":90.125,"bottleUnits":"ml","bottleType":"Formula"}})),
            document(&json!({})),json!({}),json!({})]).await;
        let mut form = prepare_with_client(fixture.context(),support::client(&stub),"test-child".into(),FeedAction::Bottle {
            at: None, amount: None, bottle_type: None, units: None, notes: None },10000.0).await?;
        press(&mut form,KeyCode::Enter);
        press(&mut form,KeyCode::Enter);
        press(&mut form,KeyCode::Char('2'));
        press(&mut form,KeyCode::Enter);
        form.apply(Input::Pasted("saved note".into()),10000.0,(80,24));
        for _ in 0..3 { press(&mut form,KeyCode::Left); }
        for _ in 0..20 { press(&mut form,KeyCode::Up); }
        for _ in 0..3 { press(&mut form,KeyCode::Enter); }
        assert_eq!(press(&mut form,KeyCode::Enter),FormEvent::Ready);
        assert_eq!(stub.requests().await.len(),1,"one defaults read across all revisits");
        form.execute().await?;
        Ok(stub.requests().await)
    })
    });
    let entries = requests
        .iter()
        .filter(|request| request.path.contains("/intervals/"))
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1);
    let recorded = entries[0].document();
    assert_eq!(recorded["start"], 8800.0);
    assert_eq!(recorded["amount"], 90.125);
    assert_eq!(recorded["bottleType"], "Breast Milk");
    assert_eq!(
        serde_json::from_str::<String>(recorded["notes"].as_str().unwrap()).unwrap(),
        "saved note"
    );
}

#[test]
fn nursing_transitions_wait_for_completion_and_never_replay() {
    for name in ["start", "pause", "resume", "switch", "stop", "cancel"] {
        let (requests, _) = support::hosted(move || {
            tokio::runtime::Runtime::new().unwrap().block_on(async move {
            let fixture = Fixture::create_fixture(name);
            let stub = support::api::Stub::start(vec![document(&json!({
                "prefs":{"lastSide":{"start":8800.0,"lastSide":"left"}},
                "timer":{"active":true,"paused":name == "resume","uuid":"test",
                    "feedStartTime":8800.0,"timerStartTime":8860.0,"leftDuration":60.0,"rightDuration":0.0,
                    "activeSide":"right","lastSide":"left"}})),json!({}),json!({})]).await;
            let mut form = prepare_with_client(fixture.context(),support::client(&stub),"test-child".into(),
                FeedAction::Nursing { action: action(name) },10000.0).await?;
            assert_eq!(stub.requests().await.len(),usize::from(name == "start"));
            if name == "cancel" { assert!(form.is_ready()); }
            else if name == "start" {
                press(&mut form,KeyCode::Enter); // suggested side
                press(&mut form,KeyCode::Left); // retained time
                press(&mut form,KeyCode::Enter);
                assert_eq!(press(&mut form,KeyCode::Enter),FormEvent::Ready);
            } else {
                press(&mut form,KeyCode::Left); // hours
                assert_eq!(press(&mut form,KeyCode::Left),FormEvent::Suspend);
                assert_eq!(press(&mut form,KeyCode::Enter),FormEvent::Ready);
            }
            assert_eq!(stub.requests().await.len(),usize::from(name == "start"));
            form.execute().await?;
            Ok(stub.requests().await)
        })
        });
        let writes = requests
            .iter()
            .filter(|request| request.method == "PATCH")
            .collect::<Vec<_>>();
        assert_eq!(writes.len(), if name == "stop" { 2 } else { 1 }, "{name}");
        let body = writes[0].document();
        match name {
            "start" => {
                assert_eq!(body["timer"]["feedStartTime"], 10000.0);
                assert_eq!(body["timer"]["activeSide"], "right");
            }
            "pause" | "switch" => assert_eq!(body["timer"]["rightDuration"], 1140.0),
            "resume" => assert_eq!(body["timer"]["timerStartTime"], 10000.0),
            "stop" => {
                assert_eq!(body["start"], 8800.0);
                assert_eq!(body["rightDuration"], 1140.0);
            }
            "cancel" => assert_eq!(body["timer"]["active"], false),
            _ => unreachable!(),
        }
    }
}
fn action(name: &str) -> NursingAction {
    match name {
        "start" => NursingAction::Start {
            start: None,
            side: None,
        },
        "pause" => NursingAction::Pause { at: None },
        "resume" => NursingAction::Resume {
            at: None,
            side: None,
        },
        "switch" => NursingAction::Switch { at: None },
        "stop" => NursingAction::Stop { at: None },
        "cancel" => NursingAction::Cancel,
        _ => unreachable!(),
    }
}
