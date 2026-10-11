//! A draft's entry identity is the one addressed by the eventual write.
use super::support::{self, Fixture};
use crate::{
    commands::{
        delete,
        delete_form::{DeleteForm, DeleteTarget},
        edit::{
            prepare,
            retained::{EditForm, EditTarget},
        },
        prepared::PreparedCommand,
    },
    domain::Calendar,
    edit::{Draft, SleepDraft},
    listing::{Column, Role, Row},
    prompt::{flow::FormEvent, host::Input},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use huckleberry_api::RowRef;
use serde_json::json;
const COLUMNS: &[Column] = &[Column::new("Entry", Role::Value)];
fn rows() -> Vec<Row> {
    ["a", "b"]
        .into_iter()
        .map(|id| Row::new(format!("sleep/{id}"), [id.into()]))
        .collect()
}
fn press(form: &mut PreparedCommand, code: KeyCode) -> FormEvent {
    form.apply(
        Input::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        10000.0,
        (80, 24),
    )
}
#[test]
fn returning_to_an_edited_entry_saves_only_its_note_once() {
    let (requests, _) = support::hosted(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let fixture = Fixture::create_fixture("edit");
            let stub = support::api::Stub::start(vec![json!({})]).await;
            let targets = ["a", "b"]
                .into_iter()
                .map(|id| EditTarget {
                    token: format!("sleep/{id}"),
                    started: 1000.125,
                    draft: Some(Draft::Sleep(SleepDraft {
                        minutes: 10.0,
                        notes: Some(id.into()),
                    })),
                    timer: None,
                })
                .collect();
            let model =
                EditForm::create_edit(Calendar::new("UTC")?, rows(), targets, None, vec![])?;
            let mut form = prepare::prepare_model(
                fixture.context(),
                support::client(&stub),
                "test-child".into(),
                model,
                10000.0,
            )?;
            press(&mut form, KeyCode::Enter);
            press(&mut form, KeyCode::Char('4'));
            press(&mut form, KeyCode::Enter);
            press(&mut form, KeyCode::Char('2'));
            press(&mut form, KeyCode::Enter);
            form.apply(Input::Pasted(" changed".into()), 10000.0, (80, 24));
            press(&mut form, KeyCode::Enter);
            press(&mut form, KeyCode::Left);
            press(&mut form, KeyCode::Down);
            press(&mut form, KeyCode::Enter);
            press(&mut form, KeyCode::Left);
            press(&mut form, KeyCode::Up);
            press(&mut form, KeyCode::Enter);
            press(&mut form, KeyCode::Char('1'));
            assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
            assert!(stub.requests().await.is_empty());
            form.execute().await?;
            Ok(stub.requests().await)
        })
    });
    assert_eq!(requests.len(), 1);
    assert!(requests[0].path.ends_with("/a"));
    assert_eq!(requests[0].update_mask(), ["details.notes", "lastUpdated"]);
    assert_eq!(requests[0].document()["details"]["notes"], "a changed");
}
#[test]
fn deletion_confirms_the_reselected_target_before_removing_it() {
    let (requests, _) = support::hosted(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let fixture = Fixture::create_fixture("delete");
            let stub = support::api::Stub::start(vec![
                json!({}),
                json!({"fields":{}}),
                json!({"documents":[]}),
            ])
            .await;
            let targets = ["a", "b"]
                .into_iter()
                .map(|id| DeleteTarget {
                    at: RowRef::loose("sleep", id),
                    started: 1000.0,
                    description: id.into(),
                })
                .collect();
            let model = DeleteForm::create_delete(rows(), COLUMNS, targets, None, false);
            let mut form = delete::prepare_model(
                fixture.context(),
                support::client(&stub),
                "test-child".into(),
                model,
                10000.0,
            )?;
            press(&mut form, KeyCode::Enter);
            press(&mut form, KeyCode::Down);
            press(&mut form, KeyCode::Left);
            press(&mut form, KeyCode::Down);
            press(&mut form, KeyCode::Enter);
            assert!(stub.requests().await.is_empty());
            press(&mut form, KeyCode::Down);
            assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
            form.execute().await?;
            Ok(stub.requests().await)
        })
    });
    let removals = requests
        .iter()
        .filter(|request| request.method == "DELETE")
        .collect::<Vec<_>>();
    assert_eq!(removals.len(), 1);
    assert!(removals[0].path.ends_with("/b"));
}
