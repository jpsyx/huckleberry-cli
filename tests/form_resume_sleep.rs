//! Resume is an explicit action attached only to the eligible completed sleep.
use app::{
    commands::edit::retained::{EditForm, EditTarget},
    domain::Calendar,
    edit::{Draft, SleepDraft},
    listing::Row,
    prompt::flow::{FormModel, FormValue, fields},
};

fn editor(explicit: Vec<String>) -> EditForm {
    let targets = ["latest", "older"].map(|id| EditTarget {
        token: format!("sleep/{id}"),
        started: 1000.0,
        draft: Some(Draft::Sleep(SleepDraft {
            minutes: 10.0,
            notes: Some("keep".into()),
        })),
        timer: None,
    });
    EditForm::create_edit(
        Calendar::new("UTC").unwrap(),
        vec![
            Row::new("sleep/latest", ["Latest".into()]),
            Row::new("sleep/older", ["Older".into()]),
        ],
        targets.into(),
        None,
        explicit,
    )
    .unwrap()
    .with_resumable_sleep(Some("sleep/latest"))
}
fn choose(model: &mut EditForm, entry: &str) {
    model
        .accept(
            &fields::create_id("entry"),
            FormValue::Picked(format!("sleep/{entry}")),
            2000.0,
        )
        .unwrap();
}
fn menu(model: &EditForm) -> app::prompt::flow::ViewId {
    model
        .route()
        .into_iter()
        .find(|id| id.field == "fields")
        .unwrap()
}
#[test]
fn resume_choice_submits_only_the_eligible_entry() {
    let mut model = editor(vec![]);
    choose(&mut model, "latest");
    let id = menu(&model);
    model
        .accept(&id, FormValue::Choice("resume".into()), 2000.0)
        .unwrap();
    assert!(model.forward(&id).is_none());
    let answer = model.complete().unwrap();
    assert_eq!(answer.changes, ["resume=true"]);
    assert_eq!(answer.target.token, "sleep/latest");
    assert!(answer.started.is_none());
}
#[test]
fn older_entries_cannot_accept_the_resume_action() {
    let mut model = editor(vec![]);
    choose(&mut model, "older");
    assert!(
        model
            .accept(&menu(&model), FormValue::Choice("resume".into()), 2000.0)
            .is_err()
    );
}
#[test]
fn default_done_and_back_navigation_do_not_resume() {
    let mut model = editor(vec![]);
    choose(&mut model, "latest");
    model
        .accept(&menu(&model), FormValue::Choice("done".into()), 2000.0)
        .unwrap();
    assert!(model.complete().unwrap().changes.is_empty());
}
#[test]
fn pending_edits_cannot_be_silently_discarded_by_resuming() {
    let mut model = editor(vec![]);
    choose(&mut model, "latest");
    let id = menu(&model);
    model
        .accept(&id, FormValue::Choice("at".into()), 2000.0)
        .unwrap();
    let field = model.forward(&id).unwrap();
    model
        .accept(&field, FormValue::Instant(950.0), 2000.0)
        .unwrap();
    assert!(
        model
            .accept(&id, FormValue::Choice("resume".into()), 2000.0)
            .is_err()
    );
}
#[test]
fn explicit_resume_is_scriptable_but_cannot_mix_with_edits() {
    let mut model = editor(vec!["resume=true".into()]);
    choose(&mut model, "latest");
    assert_eq!(model.complete().unwrap().changes, ["resume=true"]);
    let mut mixed = editor(vec!["resume=true".into(), "notes=changed".into()]);
    choose(&mut mixed, "latest");
    assert!(mixed.complete().is_err());
}
