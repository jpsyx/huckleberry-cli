//! Retained drafts have no waiting workers; only final submission starts work.
use anyhow::Result;
use app::{
    commands::prepared::{CommandProgress, PreparedCommand},
    interactive::catalog::CommandPath,
    prompt::{
        flow::{FormEvent, FormModel, FormValue, View, ViewId},
        host::Input,
    },
    theme::Theme,
    tui::forms::{Forms, Phase},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Note(String);
impl FormModel for Note {
    type Output = String;
    fn route(&self) -> Vec<ViewId> {
        vec![ViewId {
            field: "note",
            owner: None,
        }]
    }
    fn create_view(&self, _: &ViewId, _: f64) -> Result<View> {
        Ok(View::create_text("Note", &self.0, false))
    }
    fn accept(&mut self, _: &ViewId, value: FormValue, _: f64) -> Result<()> {
        let FormValue::Text(value) = value else {
            panic!("text expected")
        };
        self.0 = value;
        Ok(())
    }
    fn forward(&self, _: &ViewId) -> Option<ViewId> {
        None
    }
    fn complete(&self) -> Result<String> {
        Ok(self.0.clone())
    }
}
fn path(name: &str) -> CommandPath {
    CommandPath(vec![name.into()])
}
const fn key(code: KeyCode) -> Input {
    Input::Key(KeyEvent::new(code, KeyModifiers::NONE))
}
fn form(calls: Arc<AtomicUsize>) -> Forms {
    let prepared =
        PreparedCommand::create_prepared(Note(String::new()), 1000.0, move |_| async move {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(CommandProgress::Finished { home: false })
        })
        .unwrap();
    Forms::create_forms(path("note"), prepared)
}
fn press(form: &mut Forms, code: KeyCode) -> FormEvent {
    form.apply(key(code), 1000.0, (80, 24))
}

#[test]
fn first_view_suspends_without_a_waiter() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut form = form(calls.clone());
    assert_eq!(press(&mut form, KeyCode::Left), FormEvent::Suspend);
    assert_eq!(form.phase(), Phase::Suspended);
    assert!(form.take_execution().is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}
#[test]
fn same_command_restores_draft() {
    let mut form = form(Arc::new(AtomicUsize::new(0)));
    form.apply(Input::Pasted("unfinished note".into()), 1000.0, (80, 24));
    press(&mut form, KeyCode::Left);
    assert!(form.resume(&path("note")));
    assert_eq!(form.phase(), Phase::Collecting);
    assert!(
        form.render(1200.0, (80, 24), Theme::dark(false))
            .join("\n")
            .contains("unfinished note")
    );
}
#[test]
fn another_command_clears_draft() {
    let mut form = form(Arc::new(AtomicUsize::new(0)));
    press(&mut form, KeyCode::Left);
    assert!(!form.resume(&path("diaper")));
    assert!(!form.resume(&path("note")));
    assert!(form.render(1000.0, (80, 24), Theme::dark(false)).is_empty());
}
#[tokio::test]
async fn ready_can_execute_only_once() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut form = form(calls.clone());
    assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
    let execution = form.take_execution().unwrap();
    for code in [KeyCode::Enter, KeyCode::Left, KeyCode::Enter] {
        press(&mut form, code);
    }
    assert!(form.take_execution().is_none());
    form.complete(execution.await.unwrap());
    assert!(form.take_execution().is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn escaping_a_write_waits_for_its_result() {
    let (release, wait) = tokio::sync::oneshot::channel();
    let prepared =
        PreparedCommand::create_prepared(Note(String::new()), 1000.0, move |_| async move {
            wait.await.unwrap();
            Ok(CommandProgress::Finished { home: false })
        })
        .unwrap();
    let mut form = Forms::create_forms(path("note"), prepared);
    press(&mut form, KeyCode::Enter);
    let execution = tokio::spawn(form.take_execution().unwrap());
    press(&mut form, KeyCode::Esc);
    assert_eq!(form.phase(), Phase::Executing);
    assert!(!execution.is_finished());
    release.send(()).unwrap();
    form.complete(execution.await.unwrap().unwrap());
    assert_eq!(form.phase(), Phase::Result);
    assert!(form.go_home());
}

struct Complete;
impl FormModel for Complete {
    type Output = ();
    fn route(&self) -> Vec<ViewId> {
        Vec::new()
    }
    fn create_view(&self, _: &ViewId, _: f64) -> Result<View> {
        panic!("nothing to ask")
    }
    fn accept(&mut self, _: &ViewId, _: FormValue, _: f64) -> Result<()> {
        panic!("nothing to answer")
    }
    fn forward(&self, _: &ViewId) -> Option<ViewId> {
        None
    }
    fn complete(&self) -> Result<()> {
        Ok(())
    }
}
#[tokio::test]
async fn complete_inputs_execute_without_an_artificial_question() {
    let prepared = PreparedCommand::create_prepared(Complete, 1000.0, |()| async {
        Ok(CommandProgress::Finished { home: true })
    })
    .unwrap();
    let mut form = Forms::create_forms(path("cancel"), prepared);
    let execution = form.take_execution().expect("no unanswered fields");
    assert!(form.take_execution().is_none());
    form.complete(execution.await.unwrap());
    assert!(form.go_home());
}

#[test]
fn summary_cancel_restores_focus_without_discarding_a_suspended_note() {
    use app::tui::{App, input::route_with_draft, state::Panel};
    let mut form = form(Arc::new(AtomicUsize::new(0)));
    form.apply(Input::Pasted("unfinished note".into()), 1000.0, (80, 24));
    press(&mut form, KeyCode::Left);
    let mut suspended = Some(form);
    let mut app = App::new();
    for (code, modifiers) in [
        (KeyCode::Right, KeyModifiers::ALT),
        (KeyCode::Char('c'), KeyModifiers::CONTROL),
    ] {
        route_with_draft(
            &mut app,
            None,
            &mut suspended,
            Input::Key(KeyEvent::new(code, modifiers)),
            20,
            1000.0,
        );
    }
    assert_eq!(app.focus, Panel::Menu);
    let mut resumed = suspended.expect("Summary's cancel changes focus only");
    assert!(resumed.resume(&path("note")));
    assert!(
        resumed
            .render(1000.0, (80, 24), Theme::dark(false))
            .join("\n")
            .contains("unfinished note")
    );
}

#[test]
fn home_and_menu_cancel_still_discard_suspended_drafts() {
    use app::tui::{App, input::route_with_draft, state::Panel};
    for (focus, code, modifiers) in [
        (Panel::Summary, KeyCode::Esc, KeyModifiers::NONE),
        (Panel::Menu, KeyCode::Esc, KeyModifiers::NONE),
        (Panel::Menu, KeyCode::Char('c'), KeyModifiers::CONTROL),
    ] {
        let mut form = form(Arc::new(AtomicUsize::new(0)));
        press(&mut form, KeyCode::Left);
        let mut suspended = Some(form);
        let mut app = App::new();
        app.focus = focus;
        route_with_draft(
            &mut app,
            None,
            &mut suspended,
            Input::Key(KeyEvent::new(code, modifiers)),
            20,
            1000.0,
        );
        assert!(suspended.is_none());
    }
}
