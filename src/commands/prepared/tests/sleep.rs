//! Overlap navigation never writes; a consumed partial failure cannot replay.
use super::support::{self, Fixture};
use crate::{
    cli::{Overlap, SleepAction},
    commands::sleep::prepare,
    interactive::catalog::CommandPath,
    prompt::host::{self, Input},
    theme::Theme,
    tui::{forms::Forms, job::Job},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use huckleberry_api::firestore::value::fields_from_json;
use serde_json::json;

fn running(start: f64) -> serde_json::Value {
    json!({"fields":fields_from_json(json!({"timer":{"active":true,"paused":false,"uuid":"one","timerStartTime":start*1000.0}}).as_object().unwrap())})
}
const fn key(code: KeyCode) -> Input {
    Input::Key(KeyEvent::new(code, KeyModifiers::NONE))
}
async fn drive(job: &mut Job, done: impl Fn(&Job) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !done(job) {
        job.collect(2000.0).await;
        if std::time::Instant::now() >= deadline {
            host::remove();
            panic!("job did not settle: {}", job.visible(24).join("\n"));
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[test]
fn partial_overlap_failure_is_never_replayed() {
    let _serial = host::one_at_a_time();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let fixture = Fixture::create_fixture("sleep-partial");
        let timer = running(900.0);
        let stub = support::api::Stub::start_with_status(vec![
            (200, timer.clone()),
            (200, timer.clone()),
            (200, timer),
            (200, json!({})),
            (200, json!({"fields":{}})),
            (500, json!({"error":{"message":"manual write refused"}})),
        ])
        .await;
        let prepared = prepare::prepare_with_client(
            fixture.context(),
            support::client(&stub),
            "test-child".into(),
            SleepAction::Manual {
                start: Some("30 min ago".into()),
                end: Some("10 min ago".into()),
                overlap: Some(Overlap::DiscardSleep),
            },
            2000.0,
        )
        .await
        .unwrap();
        let mut job = Job::resume(
            Forms::create_forms(CommandPath(vec!["sleep".into(), "manual".into()]), prepared),
            Theme::dark(false),
        );
        drive(&mut job, |job| {
            job.visible(24).join("\n").contains("Ready?")
        })
        .await;
        let shown = job.visible(24).join("\n");
        assert!(
            shown.contains("Threw away the sleep in progress"),
            "{shown}"
        );
        job.feed(key(KeyCode::Left), 2000.0);
        job.feed(key(KeyCode::Enter), 2000.0);
        drive(&mut job, Job::finished).await;
        let (_, draft) = job.finish();
        assert!(draft.is_none());
        assert!(shown.contains("manual write refused"), "{shown}");

        let requests = stub.requests().await;
        assert_eq!(
            requests
                .iter()
                .filter(|request| request.method == "PATCH")
                .count(),
            2,
            "{requests:?}"
        );
    });
}
