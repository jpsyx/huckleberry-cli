//! First-use collection never writes before its consuming executor is taken.
use super::support::{Fixture, hosted};
use crate::{
    commands::prepared::{self, CommandProgress},
    prompt::{flow::FormEvent, host::Input},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
fn key(code: KeyCode) -> Input {
    Input::Key(KeyEvent::new(code, KeyModifiers::NONE))
}
#[test]
fn first_use_escape_returns_to_home_without_saving() {
    let fixture = Fixture::create_fixture("setup-escape");
    let mut context = fixture.context();
    context.offline = Some("unused-snapshot".into());
    let config_path = context.config_path.clone();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut command = runtime
        .block_on(prepared::startup(context, 10000.0))
        .unwrap()
        .unwrap();
    command.apply(key(KeyCode::Enter), 10000.0, (80, 24));
    assert_eq!(
        command.apply(key(KeyCode::Esc), 10000.0, (80, 24)),
        FormEvent::Home
    );
    assert!(!config_path.exists());
    let mut context = fixture.context();
    context.offline = Some("unused-snapshot".into());
    let command = runtime
        .block_on(prepared::prepare(
            context,
            crate::cli::Command::Now { json: false },
            10000.0,
        ))
        .unwrap()
        .unwrap();
    assert!(
        !command.is_ready(),
        "leaving setup must not bypass its gate"
    );
}
#[test]
fn prerequisite_completion_resumes_requested_command_once() {
    let fixture = Fixture::create_fixture("setup-continue");
    let mut context = fixture.context();
    let snapshot_path = context.config_path.with_extension("json");
    let snapshot = crate::dataset::Snapshot {
        version: crate::dataset::SNAPSHOT_VERSION,
        dataset: crate::domain::fixtures::dataset(),
    };
    std::fs::write(&snapshot_path, serde_json::to_string(&snapshot).unwrap()).unwrap();
    context.offline = Some(snapshot_path);
    let config_path = context.config_path.clone();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut command = runtime
        .block_on(prepared::prepare(
            context,
            crate::cli::Command::Now { json: true },
            10000.0,
        ))
        .unwrap()
        .unwrap();
    for _ in 0..2 {
        command.apply(key(KeyCode::Enter), 10000.0, (80, 24));
    }
    assert!(!config_path.exists());
    command.apply(key(KeyCode::Left), 10000.0, (80, 24));
    command.apply(key(KeyCode::Enter), 10000.0, (80, 24));
    assert_eq!(
        command.apply(key(KeyCode::Enter), 10000.0, (80, 24)),
        FormEvent::Ready
    );
    assert!(!config_path.exists());
    let (progress, _) = hosted(move || runtime.block_on(command.execute()));
    assert!(crate::setup::missing(&crate::config::load(&config_path).unwrap()).is_empty());
    let CommandProgress::Continue(command) = progress else {
        panic!("requested action must follow setup")
    };
    assert!(command.is_ready());
    let (result, output) = hosted(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(command.execute())
    });
    assert!(matches!(result, CommandProgress::Finished { .. }));
    assert_eq!(
        output
            .iter()
            .filter(|line| line.contains("today_window"))
            .count(),
        1
    );
}

#[test]
fn completed_authentication_is_not_replayed_by_back_in_setup() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let fixture = Fixture::create_fixture("auth-continue");
    let mut context = fixture.context();
    context.offline = Some(context.config_path.with_extension("json"));
    std::fs::write(
        context.offline.as_ref().unwrap(),
        serde_json::to_string(&crate::dataset::Snapshot {
            version: crate::dataset::SNAPSHOT_VERSION,
            dataset: crate::domain::fixtures::dataset(),
        })
        .unwrap(),
    )
    .unwrap();
    let authentications = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&authentications);
    let mut command = super::super::gates::create_login(context,Some(crate::cli::Command::Now { json: true }),10000.0,move |context, answer| async move {
        count.fetch_add(1,Ordering::SeqCst);
        let session = huckleberry_api::auth::parse_sign_in(&serde_json::json!({"idToken":"local-token","refreshToken":"local-refresh","localId":"local-user","expiresIn":"3600"}),10000.0)?;
        context.save_credentials(&crate::credentials::Resolved {
            email: Some(answer.email), password: Some(answer.password), session: Some(session), password_from_environment: false,
        })?;
        context.reload()
    }).unwrap();
    command.apply(
        Input::Pasted("parent@example.invalid".into()),
        10000.0,
        (80, 24),
    );
    command.apply(key(KeyCode::Enter), 10000.0, (80, 24));
    command.apply(Input::Pasted("test-secret".into()), 10000.0, (80, 24));
    command.apply(key(KeyCode::Enter), 10000.0, (80, 24));
    assert_eq!(
        command.apply(key(KeyCode::Enter), 10000.0, (80, 24)),
        FormEvent::Ready
    );
    assert_eq!(authentications.load(Ordering::SeqCst), 0);
    let (progress, _) = hosted(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(command.execute())
    });
    let CommandProgress::Continue(mut command) = progress else {
        panic!("setup follows sign-in")
    };
    command.apply(key(KeyCode::Enter), 10000.0, (80, 24));
    command.apply(key(KeyCode::Enter), 10000.0, (80, 24));
    command.apply(key(KeyCode::Left), 10000.0, (80, 24));
    command.apply(key(KeyCode::Left), 10000.0, (80, 24));
    assert_eq!(
        command.apply(key(KeyCode::Left), 10000.0, (80, 24)),
        FormEvent::Suspend
    );
    for _ in 0..3 {
        command.apply(key(KeyCode::Enter), 10000.0, (80, 24));
    }
    let (progress, _) = hosted(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(command.execute())
    });
    let CommandProgress::Continue(command) = progress else {
        panic!("requested command follows setup")
    };
    let (_, output) = hosted(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(command.execute())
    });
    assert_eq!(authentications.load(Ordering::SeqCst), 1);
    assert_eq!(
        output
            .iter()
            .filter(|line| line.contains("today_window"))
            .count(),
        1
    );
}
