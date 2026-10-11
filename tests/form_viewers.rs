//! Hosted navigation unwinds to Home without swallowing it as a read failure.
use app::{
    interactive,
    listing::{Column, Listing, Role, Row},
    prompt::host::{self, Input, Reply, Request},
    theme::Theme,
    tui::job::{Landing, settle},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
const THEME: Theme = Theme::dark(false);
fn journey(
    work: impl FnOnce() -> anyhow::Result<()> + Send + 'static,
    keys: Vec<KeyCode>,
) -> (Landing, Vec<String>) {
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let channel = host::install();
    host::set_size(100, 40);
    let worker = std::thread::spawn(move || settle(work().map(|()| false), THEME));
    let mut frames = Vec::new();
    let mut keys = keys.into_iter();
    while !worker.is_finished() {
        let Ok((request, reply)) = channel
            .requests
            .recv_timeout(std::time::Duration::from_millis(100))
        else {
            continue;
        };
        let answer = match request {
            Request::Frame(lines) => {
                frames.push(lines.join("\n"));
                assert!(frames.len() < 15, "unexpected extra frames");
                Reply::Input(Input::Key(KeyEvent::new(
                    keys.next().unwrap_or(KeyCode::Esc),
                    KeyModifiers::NONE,
                )))
            }
            Request::Show(_) => Reply::Shown,
            Request::Step(_) => Reply::Stepped { interrupted: false },
        };
        let _ = reply.send(answer);
    }
    host::remove();
    (worker.join().unwrap(), frames)
}
fn listing() -> anyhow::Result<()> {
    Listing::new("Foods", "foods", &[Column::new("Food", Role::Value)])
        .rows(vec![Row::new("banana", ["Banana".into()])])
        .show(THEME)
}
#[test]
fn escape_leaves_search_and_goes_home() {
    let (landing, frames) = journey(
        listing,
        vec![
            KeyCode::Char('/'),
            KeyCode::Char('h'),
            KeyCode::Char('a'),
            KeyCode::Esc,
        ],
    );
    assert_eq!(landing, Landing::Home);
    assert_eq!(frames.len(), 4);
    assert!(frames[3].contains("ha"));
}
#[test]
fn viewer_back_returns_to_parent() {
    for key in [KeyCode::Left, KeyCode::Char('h'), KeyCode::Char('a')] {
        let (landing, frames) = journey(listing, vec![key]);
        assert_eq!(landing, Landing::Cancelled);
        assert_eq!(frames.len(), 1);
    }
}
#[test]
fn help_back_restores_topic_choice() {
    let (landing, frames) = journey(
        || interactive::help(THEME),
        vec![
            KeyCode::Down,
            KeyCode::Enter,
            KeyCode::Left,
            KeyCode::Enter,
            KeyCode::Esc,
        ],
    );
    assert_eq!(landing, Landing::Home);
    assert_eq!(frames.len(), 5);
    assert!(frames[0].contains("Help for which command?"));
    assert!(frames[3].contains("Help for which command?"));
    assert_eq!(frames[2], frames[4], "the same topic remains selected");
}
#[test]
fn home_is_not_swallowed_by_read_retry() {
    let directory = std::env::temp_dir().join(format!("h-viewer-home-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let globals = interactive::session::SessionOptions {
        config: Some(directory.join("config.toml")),
        ..Default::default()
    };
    let (landing, frames) = journey(
        move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(interactive::operation::run(
                    &interactive::catalog::CommandPath(vec!["config".into(), "show".into()]),
                    &globals,
                    THEME,
                ))
                .map(|_| ())
        },
        vec![KeyCode::Esc],
    );
    std::fs::remove_dir_all(directory).unwrap();
    assert_eq!(landing, Landing::Home);
    assert_eq!(frames.len(), 1);
}
#[test]
fn result_navigation_never_reopens_a_submission() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&calls);
    let (landing, frames) = journey(
        move || {
            count.fetch_add(1, Ordering::SeqCst);
            interactive::pause(THEME)
        },
        vec![KeyCode::Esc, KeyCode::Enter],
    );
    assert_eq!(landing, Landing::Home);
    assert_eq!(frames.len(), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn entry_details_back_keeps_the_selected_row() {
    let (landing, frames) = journey(
        || {
            Listing::new("Entries", "entries", &[Column::new("Entry", Role::Value)])
                .rows(vec![
                    Row::new("a", ["first".into()])
                        .detail(vec![("Note".into(), "first detail".into())]),
                    Row::new("b", ["second".into()])
                        .detail(vec![("Note".into(), "second detail".into())]),
                ])
                .show(THEME)
        },
        vec![
            KeyCode::Down,
            KeyCode::Enter,
            KeyCode::Left,
            KeyCode::Enter,
            KeyCode::Esc,
        ],
    );
    assert_eq!(landing, Landing::Home);
    assert_eq!(frames[1], frames[3]);
    assert_eq!(frames[2], frames[4]);
    assert!(frames[2].contains("second detail"));
}

#[test]
fn summary_back_returns_to_parent_and_escape_goes_home() {
    for code in [
        KeyCode::Backspace,
        KeyCode::Char('q'),
        KeyCode::Char('Q'),
        KeyCode::Esc,
    ] {
        let (landing, frames) = journey(summary, vec![code]);
        assert_eq!(
            landing,
            if code == KeyCode::Esc {
                Landing::Home
            } else {
                Landing::Cancelled
            }
        );
        assert_eq!(frames.len(), 1);
    }
}

fn summary() -> anyhow::Result<()> {
    let dataset: app::domain::Dataset = serde_json::from_value(serde_json::json!({
        "fetched_at":10000.0,"timezone":"UTC","days":1,"child":{"cid":"test","name":"Baby","birthdate":null,"night_start_hour":20.0,"morning_cutoff_hour":7.0},
        "growth":null,"sleep":[],"feeds":[],"diapers":[],"pumps":[],"milestones":[],"live":{},"notes":[]
    }))?;
    app::summary::show(
        &app::render::summary::view::View {
            is_hosted: true,
            rows: &[],
            dataset: &dataset,
            calendar: &app::domain::Calendar::new("UTC")?,
            units: app::cli::Units::Ml,
            now: 10000.0,
        },
        THEME,
    )
}

#[test]
fn summary_left_keys_move_columns_without_leaving_the_hosted_view() {
    let journeys: Vec<_> = [
        KeyCode::Left,
        KeyCode::Char('h'),
        KeyCode::Char('H'),
        KeyCode::Char('a'),
        KeyCode::Char('A'),
    ]
    .into_iter()
    .map(|left| journey(summary, vec![KeyCode::Right, left, left, KeyCode::Esc]))
    .collect();
    assert_eq!(
        journeys
            .iter()
            .map(|(_, frames)| frames.len())
            .collect::<Vec<_>>(),
        vec![4; 5],
        "every left alias must redraw Summary, including at its first column"
    );
    for (landing, frames) in journeys {
        assert_eq!(landing, Landing::Home);
        assert_ne!(frames[0], frames[1], "right selects another column");
        assert_eq!(frames[0], frames[2], "left restores the first column");
        assert_eq!(frames[2], frames[3], "left stays at the first column");
    }
}
