use crate::prompt::host::{self, Input, Reply, Request};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn summary_columns_cycle_in_the_host_and_escape_returns_to_the_menu() {
    let _serial = host::one_at_a_time();
    let directory = std::env::temp_dir().join(format!("h-summary-host-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let globals = fixture(&directory);
    let channel = host::install();
    host::set_size(90, 30);
    let worker = std::thread::spawn(move || {
        tokio::runtime::Runtime::new().unwrap().block_on(super::run(
            &super::CommandPath(vec!["summary".into()]),
            &globals,
            crate::theme::Theme::dark(false),
        ))
    });
    let frames = answer_frames(channel);
    host::remove();
    let result = worker.join().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
    result.unwrap();
    assert_eq!(frames.len(), 3, "{frames:?}");
    assert!(frames[0].contains("[feeds]"), "{}", frames[0]);
    assert!(frames[1].contains("[milk ml]"), "{}", frames[1]);
    assert!(frames[2].contains("[feeds]"), "{}", frames[2]);
}

/// Own the receiver so a regression that asks an extra question cannot leave a worker hung.
fn answer_frames(channel: host::Channel) -> Vec<String> {
    let mut frames = Vec::new();
    while let Ok((request, reply)) = channel
        .requests
        .recv_timeout(std::time::Duration::from_secs(3))
    {
        match request {
            Request::Frame(lines) => {
                frames.push(lines.join("\n"));
                let key = match frames.len() {
                    _ if frames.last().unwrap().contains("Anything else?") => KeyCode::Esc,
                    1 => KeyCode::Tab,
                    2 => KeyCode::BackTab,
                    _ => KeyCode::Esc,
                };
                reply
                    .send(Reply::Input(Input::Key(KeyEvent::new(
                        key,
                        KeyModifiers::NONE,
                    ))))
                    .unwrap();
            }
            Request::Show(_) => {
                reply.send(Reply::Shown).unwrap();
            }
            Request::Step(_) => {
                reply.send(Reply::Stepped { interrupted: false }).unwrap();
            }
        }
        if frames.len() == 3
            || frames
                .last()
                .is_some_and(|frame| frame.contains("Anything else?"))
        {
            break;
        }
    }
    drop(channel);
    frames
}

fn fixture(directory: &std::path::Path) -> super::SessionOptions {
    let config = directory.join("config.toml");
    std::fs::write(&config, "timezone = 'America/New_York'\nday_start = '07:00'\nday_end = '20:00'\nday_mode = 'discrete'\n").unwrap();
    let snapshot = crate::dataset::Snapshot {
        version: crate::dataset::SNAPSHOT_VERSION,
        dataset: crate::domain::fixtures::dataset(),
    };
    let offline = directory.join("snapshot.json");
    std::fs::write(&offline, serde_json::to_string(&snapshot).unwrap()).unwrap();
    super::SessionOptions {
        config: Some(config),
        offline: Some(offline),
        ..Default::default()
    }
}
