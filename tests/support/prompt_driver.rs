use app::prompt::{self, Choice, Question};
use app::theme::Theme;
fn main() -> anyhow::Result<()> {
    let theme = Theme::dark(false);
    let mode = std::env::args().nth(1).unwrap_or_default();
    let choices = [Choice { value: "yes", hint: "Yes" }, Choice { value: "no", hint: "No" }];
    let question = Question::new("choice", "Choose answer", "--answer").with_choices(&choices).with_default("no");
    if mode == "manual_time" {
        let context = app::session::Context { config: app::config::Config::default(), config_path: "synthetic.toml".into(), credentials_path: "synthetic-credentials.json".into(), theme, verbose: false, child_override: None, offline: None };
        let started = app::prompt::time::read_manual_start(&context, None)?;
        let ended = app::prompt::time::read_manual_end(&context, None, started)?;
        assert!((119.0 * 60.0..121.0 * 60.0).contains(&(ended - started)));
        println!("MANUAL_RELATIVE_OK");
        let event = app::prompt::time::read_at(&context, None)?;
        assert!((31.0 * 60.0..33.0 * 60.0).contains(&(ended - event)));
        println!("EVENT_RELATIVE_OK");
    } else if mode == "fields" {
        let context = app::session::Context { config: app::config::Config::default(), config_path: "synthetic.toml".into(), credentials_path: "synthetic-credentials.json".into(), theme, verbose: false, child_override: None, offline: None };
        let draft = app::edit::Draft::Diaper(app::edit::DiaperDraft { potty: true, mode: app::cli::DiaperKind::Pee, pee: None, poo: None, color: None, consistency: None, rash: false, how: None, notes: None });
        let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
        let kept = runtime.block_on(app::commands::edit::form::collect(&context, Some(&draft), 1000.0, false, vec![]))?;
        assert!(kept.is_empty());
        println!("FIELDS_KEPT");
        let cleared = runtime.block_on(app::commands::edit::form::collect(&context, Some(&draft), 1000.0, false, vec![]))?;
        assert_eq!(cleared, ["how="]);
        println!("OUTCOME_CLEARED");
    } else if mode == "sleep_fields" {
        let context = app::session::Context { config: app::config::Config::default(), config_path: "synthetic.toml".into(), credentials_path: "synthetic-credentials.json".into(), theme, verbose: false, child_override: None, offline: None };
        let draft = app::edit::Draft::Sleep(app::edit::SleepDraft { minutes: 90.0, notes: None });
        let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
        // 00:15 UTC on the first day, so the stored stop is 01:45 the same day.
        let stopped = runtime.block_on(app::commands::edit::form::collect(&context, Some(&draft), 900.0, false, vec![]))?;
        assert_eq!(stopped, ["duration=105"]);
        println!("SLEEP_STOP_SET");
    } else if mode == "waiting" {
        let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
        println!("WAITING_FOR_INTERRUPT");
        let result = runtime.block_on(app::interactive::interrupt::run(std::future::pending::<anyhow::Result<()>>()));
        assert!(prompt::is_cancelled(&result.unwrap_err()));
        println!("INTERRUPTED");
        println!("AFTER={}", prompt::ask(&question, theme)?);
    } else if mode == "failure" {
        let result: anyhow::Result<()> = (|| {
            let _guard = prompt::terminal::TerminalGuard::enter()?;
            Err(std::io::Error::other("synthetic I/O failure").into())
        })();
        assert!(result.is_err());
        println!("RESTORED_AFTER_ERROR");
    } else if mode == "list" {
        use app::listing::{Listing, Column, Role, Row};
        let columns = [Column::new("Name", Role::Key)];
        let rows = (0..80).map(|index| Row::new(format!("id-{index}"), [format!("Food {index:02}")])).collect();
        let result = Listing::new("Synthetic foods", "foods", &columns).rows(rows).choose(theme)?;
        println!("SELECTED={result:?}");
    } else if mode == "keep_clear" {
        let question = Question::new("note", "Existing note", "--notes").optional().with_default("exact stored note");
        println!("KEPT={:?}", prompt::ask_optional(&question, theme)?);
        println!("CLEARED={:?}", prompt::ask_optional(&question, theme)?);
        println!("AFTER={}", prompt::ask(&Question::new("answer", "After text", "--answer").with_choices(&choices), theme)?);
    } else if mode == "secret" {
        let secret = prompt::ask_secret(&Question::new("secret", "Secret input", "--password"), theme)?;
        assert_eq!(secret, "test-private-password");
        println!("SECRET_OK");
    } else if mode == "cancel_text" {
        let result = prompt::ask(&Question::new("text", "Text to cancel", "--text"), theme);
        assert!(prompt::is_cancelled(&result.unwrap_err()));
        println!("CANCELLED");
    } else if mode == "cancel" {
        let result = prompt::ask(&question, theme);
        assert!(prompt::is_cancelled(&result.unwrap_err()));
        println!("CANCELLED");
    } else {
        println!("CHOICE={}", prompt::ask(&question, theme)?);
        let optional = Question::new("note", "Optional note", "--notes").optional();
        println!("NOTE={:?}", prompt::ask_optional(&optional, theme)?);
        println!("TEXT={}", prompt::ask(&Question::new("time", "Enter time", "--at"), theme)?);
        println!("AFTER={}", prompt::ask(&question, theme)?);
    }
    Ok(())
}
