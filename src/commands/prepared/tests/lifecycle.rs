//! Exercise the actual shell job around a controlled one-time write.
use crate::{
    commands::prepared::{CommandProgress, PreparedCommand},
    interactive::catalog::CommandPath,
    prompt::{
        flow::{FormModel, FormValue, View, ViewId},
        host::{self, Input},
    },
    theme::Theme,
    tui::{
        forms::Forms,
        job::{Job, Landing},
    },
};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Note(String);
impl FormModel for Note {
    type Output = String;
    fn route(&self) -> Vec<ViewId> {
        vec![ViewId {
            field: "note",
            owner: None,
        }]
    }
    fn create_view(&self, _: &ViewId, _: f64) -> Result<View> {
        Ok(View::create_text("Note", &self.0, false))
    }
    fn accept(&mut self, _: &ViewId, value: FormValue, _: f64) -> Result<()> {
        let FormValue::Text(value) = value else {
            panic!("text expected")
        };
        self.0 = value;
        Ok(())
    }
    fn forward(&self, _: &ViewId) -> Option<ViewId> {
        None
    }
    fn complete(&self) -> Result<String> {
        Ok(self.0.clone())
    }
}
const fn key(code: KeyCode) -> Input {
    Input::Key(KeyEvent::new(code, KeyModifiers::NONE))
}
fn job(prepared: PreparedCommand) -> Job {
    Job::resume(
        Forms::create_forms(CommandPath(vec!["note".into()]), prepared),
        Theme::dark(false),
    )
}

#[test]
fn suspension_releases_the_host_and_restores_without_preparing_again() {
    let _serial = host::one_at_a_time();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let prepared = PreparedCommand::create_prepared(Note(String::new()), 1000.0, |_| async {
            panic!("suspending must not execute")
        })
        .unwrap();
        let mut job = job(prepared);
        job.feed(Input::Pasted("unfinished".into()), 1000.0);
        job.collect(1000.0).await;
        job.feed(key(KeyCode::Left), 1000.0);
        job.collect(1000.0).await;
        assert!(job.finished());
        let (landing, draft) = job.finish();
        assert_eq!(landing, Landing::Stay);
        assert!(!host::hosted());
        let mut draft = draft.unwrap();
        assert!(draft.resume(&CommandPath(vec!["note".into()])));
        let mut job = Job::resume(draft, Theme::dark(false));
        job.collect(1200.0).await;
        assert!(job.visible(24).join("\n").contains("unfinished"));
        job.feed(key(KeyCode::Esc), 1200.0);
        job.collect(1200.0).await;
        let (landing, draft) = job.finish();
        assert_eq!(landing, Landing::Home);
        assert!(draft.is_none());
    });
}

#[test]
fn queued_enter_cannot_repeat_a_write_and_early_escape_keeps_the_receipt() {
    let _serial = host::one_at_a_time();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let calls = Arc::new(AtomicUsize::new(0));
        let writes = calls.clone();
        let (release, wait) = tokio::sync::oneshot::channel();
        let prepared =
            PreparedCommand::create_prepared(Note(String::new()), 1000.0, move |_| async move {
                writes.fetch_add(1, Ordering::SeqCst);
                wait.await.unwrap();
                crate::render::note("Saved exactly once");
                Ok(CommandProgress::Finished { home: false })
            })
            .unwrap();
        let mut job = job(prepared);
        job.feed(key(KeyCode::Enter), 1000.0);
        job.feed(key(KeyCode::Enter), 1000.0);
        job.collect(1000.0).await;
        job.feed(key(KeyCode::Esc), 1000.0);
        assert!(!job.finished());
        release.send(()).unwrap();
        drive_until(&mut job, |job| {
            job.visible(24).join("\n").contains("Ready?")
        })
        .await;
        assert!(job.visible(24).join("\n").contains("Saved exactly once"));
        assert!(
            !job.finished(),
            "early Escape must not swallow a later receipt"
        );
        job.feed(key(KeyCode::Enter), 1000.0);
        drive_until(&mut job, Job::finished).await;
        let (landing, draft) = job.finish();
        assert_eq!(landing, Landing::Home);
        assert!(draft.is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    });
}

async fn drive_until(job: &mut Job, done: impl Fn(&Job) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while !done(job) {
        job.collect(1000.0).await;
        if std::time::Instant::now() > deadline {
            host::remove();
            panic!("job did not settle");
        }
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
}
