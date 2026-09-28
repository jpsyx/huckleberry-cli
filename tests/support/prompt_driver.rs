use app::prompt::{self, Choice, Question};
use app::theme::Theme;
fn main() -> anyhow::Result<()> {
    let theme = Theme::dark(false);
    let mode = std::env::args().nth(1).unwrap_or_default();
    let choices = [Choice { value: "yes", hint: "Yes" }, Choice { value: "no", hint: "No" }];
    let question = Question::new("choice", "Choose answer", "--answer").with_choices(&choices).with_default("no");
    if mode == "cancel" {
        let result = prompt::ask(&question, theme);
        assert!(prompt::is_cancelled(&result.unwrap_err()));
        println!("CANCELLED");
    } else {
        println!("CHOICE={}", prompt::ask(&question, theme)?);
        let optional = Question::new("note", "Optional note", "--notes").optional();
        println!("NOTE={:?}", prompt::ask_optional(&optional, theme)?);
        println!("TEXT={}", prompt::ask(&Question::new("time", "Enter time", "--at"), theme)?);
    }
    Ok(())
}
