//! Back edits a retained draft; it does not undo or discard later answers.
use anyhow::{Result, bail};
use app::prompt::flow::{FormEvent, FormModel, FormState, FormValue, View, ViewId};
use app::prompt::host::Input;
use app::theme::Theme;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Default)]
struct Entry {
    at: String,
    dirty: bool,
    colour: Option<String>,
    notes: String,
}

const fn id(field: &'static str) -> ViewId {
    ViewId { field, owner: None }
}

impl FormModel for Entry {
    type Output = (String, Option<String>, String);

    fn route(&self) -> Vec<ViewId> {
        let mut route = vec![id("at"), id("mode")];
        if self.dirty {
            route.push(id("colour"));
        }
        route.push(id("notes"));
        route
    }

    fn create_view(&self, view: &ViewId, _now: f64) -> Result<View> {
        Ok(match view.field {
            "at" => View::create_text("Value", "1000", false),
            "mode" => View::create_choices(
                "Contents",
                vec![
                    ("Dirty".into(), FormValue::Choice("dirty".into())),
                    ("Wet".into(), FormValue::Choice("wet".into())),
                ],
                0,
            ),
            "colour" => View::create_choices(
                "Colour",
                vec![
                    ("Skip".into(), FormValue::Skipped),
                    ("Brown".into(), FormValue::Choice("brown".into())),
                ],
                0,
            ),
            _ => View::create_text("Value", "", false),
        })
    }

    fn accept(&mut self, view: &ViewId, value: FormValue, _now: f64) -> Result<()> {
        match (view.field, value) {
            ("at", FormValue::Text(value)) => {
                if value.parse::<u32>().is_err() {
                    bail!("Enter a number");
                }
                self.at = value;
            }
            ("mode", FormValue::Choice(value)) => self.dirty = value == "dirty",
            ("colour", FormValue::Choice(value)) => self.colour = Some(value),
            ("colour", FormValue::Skipped) => self.colour = None,
            ("notes", FormValue::Text(value)) => self.notes = value,
            _ => bail!("wrong field value"),
        }
        Ok(())
    }

    fn forward(&self, view: &ViewId) -> Option<ViewId> {
        let route = self.route();
        route
            .iter()
            .position(|item| item == view)
            .and_then(|index| route.get(index + 1).cloned())
    }

    fn complete(&self) -> Result<Self::Output> {
        Ok((
            self.at.clone(),
            self.colour.clone().filter(|_| self.dirty),
            self.notes.clone(),
        ))
    }
}

fn press(form: &mut FormState<Entry>, code: KeyCode) -> FormEvent {
    form.apply(
        Input::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        2000.0,
        (80, 24),
    )
}

fn shown(form: &FormState<Entry>) -> String {
    form.render(2000.0, (80, 24), Theme::dark(false)).join("\n")
}

fn at_notes() -> FormState<Entry> {
    let mut form = FormState::create_form(Entry::default(), 2000.0).unwrap();
    for code in [
        KeyCode::Enter,
        KeyCode::Enter,
        KeyCode::Down,
        KeyCode::Enter,
    ] {
        assert_eq!(press(&mut form, code), FormEvent::Stay);
    }
    form
}

#[test]
fn back_keeps_forward_answers() {
    let mut form = at_notes();
    press(&mut form, KeyCode::Char('x'));
    for _ in 0..3 {
        assert_eq!(press(&mut form, KeyCode::Left), FormEvent::Stay);
    }
    press(&mut form, KeyCode::Backspace);
    press(&mut form, KeyCode::Char('1'));
    press(&mut form, KeyCode::Enter);
    assert!(shown(&form).contains("> 1. Dirty"));
    press(&mut form, KeyCode::Enter);
    assert!(shown(&form).contains("> 2. Brown"));
    press(&mut form, KeyCode::Enter);
    assert!(shown(&form).contains("> x"));
    assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
    assert_eq!(
        form.complete().unwrap(),
        ("1001".into(), Some("brown".into()), "x".into())
    );
}

#[test]
fn hidden_answers_return_to_their_own_fields() {
    let mut form = at_notes();
    press(&mut form, KeyCode::Left);
    press(&mut form, KeyCode::Left);
    press(&mut form, KeyCode::Down);
    press(&mut form, KeyCode::Enter);
    assert!(!shown(&form).contains("Colour"));
    assert_eq!(form.complete().unwrap().1, None);
    press(&mut form, KeyCode::Left);
    press(&mut form, KeyCode::Up);
    press(&mut form, KeyCode::Enter);
    assert!(shown(&form).contains("> 2. Brown"));
}

#[test]
fn skip_is_an_answer() {
    let mut form = at_notes();
    press(&mut form, KeyCode::Left);
    press(&mut form, KeyCode::Up);
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Left);
    assert!(shown(&form).contains("> 1. Skip"));
    assert_eq!(form.complete().unwrap().1, None);
}

#[test]
fn back_does_not_validate() {
    let mut form = FormState::create_form(Entry::default(), 2000.0).unwrap();
    press(&mut form, KeyCode::Char('x'));
    assert_eq!(press(&mut form, KeyCode::Left), FormEvent::Suspend);
    assert!(shown(&form).contains("1000x"));
    assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Stay);
    assert!(shown(&form).contains("Enter a number"));
}
