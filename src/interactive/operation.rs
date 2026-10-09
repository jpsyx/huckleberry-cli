//! Direct command execution and error recovery for menu selections.
use super::{
    catalog::CommandPath,
    draft::CommandDraft,
    pause,
    session::{SessionOptions, load_context},
    session_menu::choose,
};
use crate::{
    cli::{
        AuthAction, ChildAction, Command, ConfigAction, FeedAction, FoodsAction, NursingAction,
        PumpAction, SleepAction,
    },
    prompt,
    theme::Theme,
};
use anyhow::Result;

/// Returns true after a successful action that should return to home.
pub async fn run(path: &CommandPath, globals: &SessionOptions, theme: Theme) -> Result<bool> {
    let draft = CommandDraft::new(path.clone());
    if draft
        .resolve(globals)?
        .command
        .as_ref()
        .is_some_and(is_read_view)
    {
        return run_view(&draft, globals, theme).await;
    }
    execute(&draft, globals, theme).await
}

async fn run_view(draft: &CommandDraft, globals: &SessionOptions, theme: Theme) -> Result<bool> {
    loop {
        let mut labels = vec!["Back".into()];
        match execute(draft, globals, theme).await {
            Ok(_) => {}
            Err(error) if prompt::is_cancelled(&error) => return Ok(false),
            Err(error) => {
                crate::render::note(&theme.error_line("error:", &format!("{error:#}")));
                labels.push("Retry".into());
            }
        }
        match choose("Anything else?", &labels, 0, theme)? {
            1 => {}
            _ => return Ok(false),
        }
    }
}

// Only read-only views offer an explicit retry after failure.
const fn is_read_view(command: &Command) -> bool {
    matches!(
        command,
        Command::Now { .. }
            | Command::Dash { .. }
            | Command::Summary { .. }
            | Command::Trends { .. }
            | Command::Stripes { .. }
            | Command::Log { .. }
            | Command::Info
            | Command::Auth {
                action: AuthAction::Status
            }
            | Command::Child {
                action: ChildAction::List | ChildAction::Show
            }
            | Command::Config {
                action: ConfigAction::Show | ConfigAction::Path
            }
            | Command::Foods {
                action: FoodsAction::List { .. }
            }
            | Command::Sleep {
                action: SleepAction::Status
            }
            | Command::Pump {
                action: PumpAction::Status
            }
            | Command::Feed {
                action: FeedAction::Nursing {
                    action: NursingAction::Status
                }
            }
    )
}

async fn execute(draft: &CommandDraft, globals: &SessionOptions, theme: Theme) -> Result<bool> {
    if draft.is_unchanged_edit() {
        crate::render::note("The entry is unchanged.");
        pause(theme)?;
        return Ok(true);
    }
    let cli = draft.resolve(globals)?;
    let context = load_context(globals, theme)?;
    let command = cli
        .command
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no command selected"))?;
    crate::commands::dispatch(&context, command).await?;
    if is_read_view(command) {
        return Ok(false);
    }
    // Utilities keep their submenu; completed recording actions go home.
    let home = !matches!(
        command,
        Command::Foods { .. }
            | Command::Child { .. }
            | Command::Config { .. }
            | Command::Export { .. }
    );
    pause(theme)?;
    Ok(home)
}

#[cfg(test)]
mod tests {
    use super::is_read_view;
    use crate::cli::Cli;
    use clap::Parser;

    #[test]
    fn backing_out_of_logs_returns_without_a_followup_prompt() {
        use crossterm::event::KeyCode;
        for key in [
            KeyCode::Esc,
            KeyCode::Left,
            KeyCode::Char('h'),
            KeyCode::Char('a'),
            KeyCode::Char('q'),
        ] {
            let (frames, _) = log_journey(key, false);
            assert_eq!(
                frames.len(),
                1,
                "Back should close the log browser: {frames:?}"
            );
            assert!(frames[0].contains("Bottle"), "{frames:?}");
        }
    }

    #[test]
    fn logs_keep_new_details_and_empty_results_visible() {
        use crossterm::event::KeyCode;
        let (frames, output) = log_journey(KeyCode::Enter, false);
        assert_eq!(frames.len(), 2, "{frames:?}");
        assert!(frames[1].contains("Anything else?"), "{frames:?}");
        assert!(output.contains("Formula"), "{output}");
        let (frames, output) = log_journey(KeyCode::Esc, true);
        assert_eq!(frames.len(), 1, "{frames:?}");
        assert!(frames[0].contains("Anything else?"), "{frames:?}");
        assert!(output.contains("nothing logged"), "{output}");
    }

    fn log_journey(first_key: crossterm::event::KeyCode, empty: bool) -> (Vec<String>, String) {
        use crate::prompt::host;
        let _serial = host::one_at_a_time();
        let directory = std::env::temp_dir().join(format!("h-log-back-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let globals = log_fixture(&directory, empty);
        let channel = host::install();
        let worker = std::thread::spawn(move || {
            tokio::runtime::Runtime::new().unwrap().block_on(super::run(
                &super::CommandPath(vec!["log".into()]),
                &globals,
                crate::theme::Theme::dark(false),
            ))
        });
        let (frames, output) = drive_log(&channel, &worker, first_key);
        host::remove();
        let result = worker.join().unwrap();
        std::fs::remove_dir_all(directory).unwrap();
        assert!(
            matches!(result, Ok(false)) || result.as_ref().is_err_and(crate::prompt::is_cancelled),
            "{result:?}"
        );
        (frames, output)
    }

    fn drive_log(
        channel: &crate::prompt::host::Channel,
        worker: &std::thread::JoinHandle<anyhow::Result<bool>>,
        first_key: crossterm::event::KeyCode,
    ) -> (Vec<String>, String) {
        use crate::prompt::host::{Input, Reply, Request};
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut frames = Vec::new();
        let mut output = String::new();
        while !worker.is_finished() {
            let Ok((request, reply)) = channel
                .requests
                .recv_timeout(std::time::Duration::from_millis(100))
            else {
                continue;
            };
            let answer = match request {
                Request::Frame(lines) => {
                    let key = if frames.is_empty() {
                        first_key
                    } else {
                        KeyCode::Esc
                    };
                    frames.push(lines.join("\n"));
                    Reply::Input(Input::Key(KeyEvent::new(key, KeyModifiers::NONE)))
                }
                Request::Show(lines) => {
                    output.push_str(&lines.join("\n"));
                    Reply::Shown
                }
                Request::Step(_) => Reply::Stepped { interrupted: false },
            };
            reply.send(answer).unwrap();
        }
        (frames, output)
    }

    fn log_fixture(directory: &std::path::Path, empty: bool) -> super::SessionOptions {
        let config = directory.join("config.toml");
        std::fs::write(&config, "timezone = 'America/New_York'\n").unwrap();
        let mut data = crate::domain::fixtures::dataset();
        if !empty {
            data.feeds.push(crate::domain::fixtures::bottle(
                data.fetched_at - 600.0,
                90.0,
            ));
        }
        let snapshot = crate::dataset::Snapshot {
            version: crate::dataset::SNAPSHOT_VERSION,
            dataset: data,
        };
        let offline = directory.join("snapshot.json");
        std::fs::write(&offline, serde_json::to_string(&snapshot).unwrap()).unwrap();
        super::SessionOptions {
            config: Some(config),
            offline: Some(offline),
            ..Default::default()
        }
    }

    #[test]
    fn read_views_offer_retry() {
        for path in [
            "now",
            "dash",
            "summary",
            "trends",
            "stripes",
            "log",
            "info",
            "auth status",
            "child list",
            "child show",
            "config show",
            "config path",
            "foods list",
            "sleep status",
            "pump status",
            "feed nursing status",
        ] {
            let cli =
                Cli::try_parse_from(std::iter::once("h").chain(path.split_whitespace())).unwrap();
            assert!(is_read_view(cli.command.as_ref().unwrap()), "{path}");
        }
    }

    #[test]
    fn writes_and_exports_do_not_offer_retry() {
        for path in [
            "diaper",
            "potty",
            "growth",
            "edit",
            "delete",
            "export",
            "feed bottle",
            "feed solids",
            "sleep start",
            "sleep stop",
            "sleep pause",
            "sleep resume",
            "sleep cancel",
            "sleep manual",
            "pump log",
            "pump start",
            "pump pause",
            "pump resume",
            "pump stop",
            "pump cancel",
            "feed nursing start",
            "feed nursing stop",
            "feed nursing pause",
            "feed nursing resume",
            "feed nursing switch",
            "feed nursing cancel",
            "auth login",
            "auth logout",
            "child use",
            "config set",
            "foods add",
        ] {
            let cli =
                Cli::try_parse_from(std::iter::once("h").chain(path.split_whitespace())).unwrap();
            assert!(!is_read_view(cli.command.as_ref().unwrap()), "{path}");
        }
    }
}

#[cfg(test)]
#[path = "operation/summary_tests.rs"]
mod summary_tests;
