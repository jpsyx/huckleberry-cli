use app::prompt::{self, Choice, Question};
use app::theme::Theme;
fn main() -> anyhow::Result<()> {
    let theme = Theme::dark(false);
    let mode = std::env::args().nth(1).unwrap_or_default();
    let choices = [Choice { value: "yes", hint: "Yes" }, Choice { value: "no", hint: "No" }];
    let question = Question::new("choice", "Choose answer", "--answer").with_choices(&choices).with_default("no");
    if mode == "failure" {
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
