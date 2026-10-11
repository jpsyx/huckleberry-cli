//! Setup and utilities collect complete drafts, retaining values across Back.
use app::{
    commands::utility_form::{LoginForm, LoginInput, SettingsForm},
    config::Config,
    prompt::{
        flow::{FormEvent, FormModel, FormState, FormValue, fields},
        host::Input,
    },
    setup::form::SetupForm,
    theme::Theme,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
fn press<Form: FormModel>(form: &mut FormState<Form>, code: KeyCode) -> FormEvent {
    form.apply(
        Input::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        10000.0,
        (80, 24),
    )
}
#[test]
fn setup_back_does_not_save() {
    let original = Config::default();
    let mut form =
        FormState::create_form(SetupForm::create_setup(original.clone(), None), 10000.0).unwrap();
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Left);
    press(&mut form, KeyCode::Left);
    assert_eq!(press(&mut form, KeyCode::Left), FormEvent::Suspend);
    assert_eq!(app::setup::missing(&original).len(), 3);
    assert!(form.complete().is_err());
}
#[test]
fn setup_saves_one_completed_draft() {
    let mut form =
        FormState::create_form(SetupForm::create_setup(Config::default(), None), 10000.0).unwrap();
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Left);
    press(&mut form, KeyCode::Left);
    press(&mut form, KeyCode::Down);
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Enter);
    assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
    assert!(app::setup::missing(&form.complete().unwrap()).is_empty());
}
#[test]
fn login_back_keeps_masked_credentials() {
    let mut form = FormState::create_form(
        LoginForm::create_login(LoginInput::default()).unwrap(),
        10000.0,
    )
    .unwrap();
    form.apply(
        Input::Pasted("parent@example.invalid".into()),
        10000.0,
        (80, 24),
    );
    press(&mut form, KeyCode::Enter);
    form.apply(Input::Pasted("private password".into()), 10000.0, (80, 24));
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Left);
    let shown = form
        .render(10000.0, (80, 24), Theme::dark(false))
        .join("\n");
    assert!(!shown.contains("private password"));
    press(&mut form, KeyCode::Left);
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Enter);
    assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
    let answer = form.complete().unwrap();
    assert_eq!(answer.password, "private password");
    assert_eq!(answer.email, "parent@example.invalid");
}
#[test]
fn settings_values_are_scoped_by_key() {
    let mut model = SettingsForm::create_settings(Config::default(), None, None).unwrap();
    model
        .accept(
            &fields::create_id("key"),
            FormValue::Choice("timezone".into()),
            10000.0,
        )
        .unwrap();
    let mut form = FormState::create_form(model, 10000.0).unwrap();
    press(&mut form, KeyCode::Char('2'));
    press(&mut form, KeyCode::Enter);
    form.apply(Input::Pasted(" unfinished".into()), 10000.0, (80, 24));
    press(&mut form, KeyCode::Left);
    press(&mut form, KeyCode::Char('3'));
    press(&mut form, KeyCode::Enter);
    assert!(
        !form
            .render(10000.0, (80, 24), Theme::dark(false))
            .join("\n")
            .contains("unfinished")
    );
    press(&mut form, KeyCode::Left);
    press(&mut form, KeyCode::Char('2'));
    press(&mut form, KeyCode::Enter);
    assert!(
        form.render(10000.0, (80, 24), Theme::dark(false))
            .join("\n")
            .contains("unfinished")
    );
}
