//! Entry identity stays attached to drafts and destructive confirmations.
use app::{
    commands::{
        delete_form::{DeleteForm, DeleteTarget},
        edit::retained::{EditForm, EditTarget},
    },
    domain::Calendar,
    edit::{Draft, SleepDraft},
    listing::{Column, Role, Row},
    prompt::{
        flow::{FormEvent, FormModel, FormState, FormValue, fields, listing::ListingControl},
        host::Input,
    },
    theme::Theme,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use huckleberry_api::RowRef;
const COLUMNS: &[Column] = &[Column::new("Entry", Role::Value)];
fn rows() -> Vec<Row> {
    vec![
        Row::new("sleep/a", ["A".into()]),
        Row::new("sleep/b", ["B".into()]),
    ]
}
fn press<Form: FormModel>(form: &mut FormState<Form>, code: KeyCode) -> FormEvent {
    form.apply(
        Input::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        10000.0,
        (80, 24),
    )
}
fn editor() -> FormState<EditForm> {
    let targets = [("a", "A"), ("b", "B")]
        .into_iter()
        .map(|(id, note)| EditTarget {
            token: format!("sleep/{id}"),
            started: 1000.25,
            draft: Some(Draft::Sleep(SleepDraft {
                minutes: 10.0,
                notes: Some(note.into()),
            })),
            timer: None,
        })
        .collect();
    FormState::create_form(
        EditForm::create_edit(Calendar::new("UTC").unwrap(), rows(), targets, None, vec![])
            .unwrap(),
        10000.0,
    )
    .unwrap()
}
fn edit_note(form: &mut FormState<EditForm>) {
    press(form, KeyCode::Char('4'));
    press(form, KeyCode::Enter); // notes
    press(form, KeyCode::Char('2'));
    press(form, KeyCode::Enter); // change
}
#[test]
fn back_from_field_keeps_other_changes() {
    let mut form = editor();
    press(&mut form, KeyCode::Enter);
    edit_note(&mut form);
    form.apply(Input::Pasted(" changed".into()), 10000.0, (80, 24));
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Char('2'));
    press(&mut form, KeyCode::Enter); // time
    press(&mut form, KeyCode::Left); // absolute begins hours
    press(&mut form, KeyCode::Char('1'));
    assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
    assert!(
        form.complete()
            .unwrap()
            .changes
            .contains(&"notes=A changed".into())
    );
}
#[test]
fn back_to_picker_keeps_selection() {
    let mut form = editor();
    press(&mut form, KeyCode::Down);
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Left);
    press(&mut form, KeyCode::Enter);
    assert_eq!(form.complete().unwrap().target.token, "sleep/b");
}
#[test]
fn another_entry_has_another_draft() {
    let mut form = editor();
    press(&mut form, KeyCode::Enter);
    edit_note(&mut form);
    form.apply(Input::Pasted(" changed".into()), 10000.0, (80, 24));
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Left);
    press(&mut form, KeyCode::Down);
    press(&mut form, KeyCode::Enter);
    edit_note(&mut form);
    let shown = form
        .render(10000.0, (80, 24), Theme::dark(false))
        .join("\n");
    assert!(shown.contains("> B"), "{shown}");
    assert!(!shown.contains("A changed"));
}
#[test]
fn deleted_row_is_not_retargeted() {
    let mut listing = ListingControl::create_listing("Entries", COLUMNS, rows(), None);
    listing.refresh_rows(vec![Row::new("sleep/b", ["B".into()])]);
    assert!(listing.value().is_err());
    listing.apply(
        Input::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
        (80, 24),
    );
    assert!(matches!(listing.value().unwrap(),FormValue::Picked(value) if value=="sleep/b"));
}
#[test]
fn delete_confirmation_is_bound_to_entry() {
    let targets = ["a", "b"]
        .into_iter()
        .map(|id| DeleteTarget {
            at: RowRef::loose("sleep", id),
            started: 1000.0,
            description: format!("entry {id}"),
        })
        .collect();
    let mut model = DeleteForm::create_delete(rows(), COLUMNS, targets, None, false);
    model
        .accept(
            &fields::create_id("entry"),
            FormValue::Picked("sleep/a".into()),
            10000.0,
        )
        .unwrap();
    model
        .accept(
            &fields::create_id("confirm"),
            FormValue::Choice("yes".into()),
            10000.0,
        )
        .unwrap();
    model
        .accept(
            &fields::create_id("entry"),
            FormValue::Picked("sleep/b".into()),
            10000.0,
        )
        .unwrap();
    assert!(model.complete().is_err());
    let view = model
        .create_view(model.route().last().unwrap(), 10000.0)
        .unwrap();
    assert!(matches!(view.value(10000.0).unwrap(),FormValue::Choice(value) if value=="no"));
}

#[test]
fn accepting_an_unchanged_bottle_amount_has_no_explicit_changes() {
    let target = EditTarget {
        token: "feed/a".into(),
        started: 1000.0,
        draft: Some(Draft::Bottle(app::edit::BottleDraft {
            amount: 90.125,
            units: app::cli::Units::Ml,
            kind: app::cli::BottleKind::Formula,
            notes: None,
        })),
        timer: None,
    };
    let mut form = FormState::create_form(
        EditForm::create_edit(
            Calendar::new("UTC").unwrap(),
            vec![],
            vec![target],
            Some("feed/a".into()),
            vec![],
        )
        .unwrap(),
        10000.0,
    )
    .unwrap();
    press(&mut form, KeyCode::Char('3'));
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Enter);
    assert!(form.complete().unwrap().changes.is_empty());
}
#[test]
fn unchanged_future_stored_time_is_not_rejected() {
    let target = EditTarget {
        token: "sleep/a".into(),
        started: 11000.25,
        draft: None,
        timer: None,
    };
    let mut form = FormState::create_form(
        EditForm::create_edit(
            Calendar::new("UTC").unwrap(),
            vec![],
            vec![target],
            Some("sleep/a".into()),
            vec![],
        )
        .unwrap(),
        10000.0,
    )
    .unwrap();
    press(&mut form, KeyCode::Char('2'));
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Enter);
    assert!(
        form.render(10000.0, (80, 24), Theme::dark(false))
            .join("\n")
            .contains("Fields to change")
    );
    assert_eq!(form.complete().unwrap().started, None);
}

#[test]
fn edited_foods_keep_their_nested_name_buffer() {
    let target = EditTarget {
        token: "feed/a".into(),
        started: 1000.0,
        draft: Some(Draft::Solids(app::edit::SolidsDraft {
            foods: vec!["Avocado".into()],
            amount: "some".into(),
            reaction: None,
            notes: None,
        })),
        timer: None,
    };
    let mut form = FormState::create_form(
        EditForm::create_edit(
            Calendar::new("UTC").unwrap(),
            vec![],
            vec![target],
            Some("feed/a".into()),
            vec![],
        )
        .unwrap(),
        10000.0,
    )
    .unwrap();
    press(&mut form, KeyCode::Char('3'));
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Char('2'));
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Char('2'));
    press(&mut form, KeyCode::Enter);
    form.apply(Input::Pasted("Pear".into()), 10000.0, (80, 24));
    press(&mut form, KeyCode::Left);
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Enter);
    press(&mut form, KeyCode::Char('1'));
    press(&mut form, KeyCode::Enter);
    assert!(
        form.complete()
            .unwrap()
            .changes
            .contains(&"foods=Avocado, Pear".into())
    );
}

#[test]
fn picker_scroll_does_not_jump_when_moving_back_within_the_visible_page() {
    let rows = (0..30)
        .map(|index| Row::new(index.to_string(), [format!("entry{index:02}")]))
        .collect();
    let mut list = ListingControl::create_listing("Entries", COLUMNS, rows, None);
    let key = |code| Input::Key(KeyEvent::new(code, KeyModifiers::NONE));
    for _ in 0..15 {
        list.apply(key(KeyCode::Down), (80, 10));
    }
    let before = list.render((80, 10), Theme::dark(false));
    list.apply(key(KeyCode::Up), (80, 10));
    let after = list.render((80, 10), Theme::dark(false));
    assert_eq!(before[3], after[3]);
}
