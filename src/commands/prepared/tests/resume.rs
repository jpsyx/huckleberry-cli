//! Completed sleep actions remain read-only until submission and recheck at execution.
use super::support::{self, Fixture};
use crate::{
    commands::edit::{
        prepare,
        retained::{EditForm, EditTarget},
    },
    domain::Calendar,
    edit::{Draft, SleepDraft},
    prompt::{flow::FormEvent, host::Input},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::{Value, json};

fn document(name: &str, fields: Value) -> Value {
    let Value::Object(fields) = fields else {
        panic!("expected fields")
    };
    json!({"name":name,"updateTime":"2026-10-19T10:00:00.000000Z",
        "fields":huckleberry_api::firestore::value::fields_from_json(&fields)})
}
fn replies(later: bool) -> Vec<Value> {
    let sleep = document(
        "sleep/test-child/intervals/latest",
        json!({"start":1000.0,"duration":600.0,"offset":0}),
    );
    let mut replies = vec![
        document("sleep/test-child", json!({"timer":{"active":false}})),
        json!({"documents":[sleep]}),
    ];
    replies.extend((0..6).map(|_| json!({"documents":[]})));
    if later {
        replies[3] = json!({"documents":[document("diaper/test-child/intervals/later",json!({"start":1700.0}))]});
    }
    replies.extend([
        document("feed/test-child", json!({})),
        document("pump/test-child", json!({})),
        sleep,
        json!({}),
    ]);
    replies
}
fn model(explicit: bool) -> EditForm {
    EditForm::create_edit(
        Calendar::new("UTC").unwrap(),
        vec![],
        vec![EditTarget {
            token: "sleep/latest".into(),
            started: 1000.0,
            timer: None,
            draft: Some(Draft::Sleep(SleepDraft {
                minutes: 10.0,
                notes: None,
            })),
        }],
        Some("sleep/latest".into()),
        if explicit {
            vec!["resume=true".into()]
        } else {
            vec![]
        },
    )
    .unwrap()
    .with_resumable_sleep(Some("sleep/latest"))
}
#[test]
fn resumed_sleep_uses_the_same_executor_for_menu_and_flags() {
    for explicit in [false, true] {
        let (requests, shown) = support::hosted(move || {
            tokio::runtime::Runtime::new().unwrap().block_on(async {
                let fixture = Fixture::create_fixture("resume");
                let stub = support::api::Stub::start(replies(false)).await;
                let mut form = prepare::prepare_model(
                    fixture.context(),
                    support::client(&stub),
                    "test-child".into(),
                    model(explicit),
                    2000.0,
                )?;
                if !explicit {
                    assert!(
                        form.render(2000.0, (80, 24), crate::theme::Theme::dark(false))
                            .join("\n")
                            .contains("Resume sleep")
                    );
                    form.apply(
                        Input::Key(KeyEvent::new(KeyCode::Char('5'), KeyModifiers::NONE)),
                        2000.0,
                        (80, 24),
                    );
                    assert_eq!(
                        form.apply(
                            Input::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
                            2000.0,
                            (80, 24)
                        ),
                        FormEvent::Ready
                    );
                }
                assert!(stub.requests().await.is_empty());
                form.execute().await?;
                Ok(stub.requests().await)
            })
        });
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.method == "POST")
                .count(),
            1
        );
        assert!(shown.join("\n").contains("Sleep resumed"));
    }
}
#[test]
fn later_events_found_at_submission_refuse_a_previously_offered_resume() {
    support::hosted(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let fixture = Fixture::create_fixture("resume-stale");
            let stub = support::api::Stub::start(replies(true)).await;
            let form = prepare::prepare_model(
                fixture.context(),
                support::client(&stub),
                "test-child".into(),
                model(true),
                2000.0,
            )?;
            assert!(form.execute().await.is_err());
            assert!(
                stub.requests()
                    .await
                    .iter()
                    .all(|request| request.method == "GET")
            );
            Ok(())
        })
    });
}
