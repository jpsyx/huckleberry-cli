//! Bursts use retained controls once and are discarded at the form boundary.
use app::{
    commands::{
        diaper_form::{DiaperForm, DiaperInput},
        prepared::{CommandProgress, PreparedCommand},
    },
    domain::Calendar,
    interactive::catalog::CommandPath,
    prompt::host::{self, Input},
    theme::Theme,
    tui::{
        forms::Forms,
        job::{Job, Landing},
    },
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
const fn key(code: KeyCode) -> Input {
    Input::Key(KeyEvent::new(code, KeyModifiers::NONE))
}
fn create_job(calls: Arc<AtomicUsize>) -> Job {
    let model = DiaperForm::create_diaper(Calendar::new("UTC").unwrap(), DiaperInput::default());
    let prepared = PreparedCommand::create_prepared(model, 10000.0, move |_| async move {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(CommandProgress::Finished { home: true })
    })
    .unwrap();
    Job::resume(
        Forms::create_forms(CommandPath(vec!["diaper".into()]), prepared),
        Theme::dark(false),
    )
}
#[tokio::test(flavor = "multi_thread")]
async fn fast_navigation_keeps_paste_but_clears_exit_queue() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut job = create_job(calls.clone());
    // Time, wet contents, amount, no rash; then an unfinished note.
    for _ in 0..4 {
        job.feed(key(KeyCode::Enter), 10000.0);
        job.collect(10000.0).await;
    }
    for input in [
        Input::Pasted("ha complete\n pasted note".into()),
        key(KeyCode::Left),
        key(KeyCode::Enter),
    ] {
        job.feed(input, 10000.0);
    }
    for _ in 0..3 {
        job.collect(10000.0).await;
    }
    let shown = job.visible(40).join("\n");
    assert!(shown.contains("ha complete pasted note"), "{shown}");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "pasted newline is not submission"
    );
    job.feed(key(KeyCode::Esc), 10000.0);
    job.feed(key(KeyCode::Enter), 10000.0);
    job.collect(10000.0).await;
    assert!(job.finished());
    let (landing, draft) = job.finish();
    assert_eq!(landing, Landing::Home);
    assert!(draft.is_none());
    assert!(!host::hosted());
    let mut fresh = create_job(calls.clone());
    fresh.collect(10000.0).await;
    assert!(fresh.visible(40).join("\n").contains("When?"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    fresh.feed(key(KeyCode::Esc), 10000.0);
    fresh.collect(10000.0).await;
    let _ = fresh.finish();
}
