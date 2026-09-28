use app::interactive::{
    catalog::{self, CommandPath},
    draft::CommandDraft,
    options::{self, OptionControl},
    session::SessionOptions,
};
use std::collections::BTreeSet;

#[test]
fn every_clap_argument_has_a_control() {
    for path in catalog::command_paths() {
        let command = catalog::metadata(&path).unwrap();
        let arguments = command
            .get_arguments()
            .map(|arg| arg.get_id().to_string())
            .collect::<BTreeSet<_>>();
        let bindings = options::bindings(&path);
        assert_eq!(
            arguments,
            bindings
                .iter()
                .map(|binding| binding.argument_id.clone())
                .collect(),
            "{path:?}"
        );
        for binding in bindings {
            if binding.argument_id == "password" {
                assert_eq!(binding.control, OptionControl::Secret);
            }
        }
    }
}

#[test]
fn conflicting_food_sources_are_replaced_not_combined() {
    let mut draft = CommandDraft::new(CommandPath(vec!["foods".into(), "list".into()]));
    options::set_value(&mut draft, "custom", vec!["true".into()]);
    options::set_value(&mut draft, "curated", vec!["true".into()]);
    assert!(!draft.values.contains_key("custom"));
    assert!(draft.resolve(&SessionOptions::default()).is_ok());
}

#[test]
fn passwords_are_hidden_in_previews_and_parser_errors() {
    let mut draft = CommandDraft::new(CommandPath(vec!["auth".into(), "login".into()]));
    draft.set("password", vec!["SECRET-VALUE".into()]);
    assert!(!format!("{:?}", app::interactive::draft::preview(&draft)).contains("SECRET-VALUE"));
    draft.set("missing", vec!["SECRET-VALUE".into()]);
    assert!(
        !draft
            .resolve(&SessionOptions::default())
            .unwrap_err()
            .to_string()
            .contains("SECRET-VALUE")
    );
}

#[test]
fn missing_positional_key_never_reinterprets_its_value() {
    let mut draft = CommandDraft::new(CommandPath(vec!["config".into(), "set".into()]));
    draft.set("value", vec!["oz".into()]);
    assert!(draft.resolve(&SessionOptions::default()).is_err());
}

#[test]
fn edit_field_choices_preserve_app_values() {
    let choices = options::fields::choices("pee");
    assert_eq!(
        choices
            .iter()
            .map(|pair| pair.0.as_str())
            .collect::<Vec<_>>(),
        ["little", "medium", "big"]
    );
    assert!(
        options::fields::choices("type")
            .iter()
            .any(|pair| pair.0 == "Breast Milk")
    );
    assert!(
        options::fields::choices("how")
            .iter()
            .any(|pair| pair.0 == "wentPotty")
    );
}

#[test]
fn keeping_a_field_discards_its_pending_override() {
    let mut changes = vec!["notes=hello".to_owned(), "rash=true".to_owned()];
    app::interactive::options::fields::update_change(&mut changes, "notes", None);
    assert_eq!(changes, ["rash=true"]);
}

#[test]
fn raw_tracker_rows_can_be_selected_without_typing_id() {
    let at = huckleberry_api::RowRef::loose("health", "original-id");
    let rows = vec![(
        at.clone(),
        serde_json::json!({"start": 1_700_000_000, "weight": 4.2}),
    )];
    let calendar = app::domain::Calendar::new("UTC").unwrap();
    let shown = app::commands::delete::create_tracker_rows(&rows, &calendar);
    assert_eq!(shown[0].key, app::edit::token_for(&at));
    assert!(shown[0].selectable);
    assert!(shown[0].cells[1].contains("4.2"));
}

#[test]
fn session_cancel_preserves_all_overrides() {
    let original = SessionOptions {
        child: Some("baby".into()),
        verbose: true,
        ..Default::default()
    };
    let mut pending = original.clone();
    pending.child = None;
    pending.verbose = false;
    assert_eq!(
        app::interactive::session::finish_edit(&original, pending, false),
        original
    );
}

#[test]
fn offline_choice_can_return_to_live() {
    let original = SessionOptions {
        offline: Some("snapshot.json".into()),
        ..Default::default()
    };
    let mut pending = original.clone();
    pending.offline = None;
    let saved = app::interactive::session::finish_edit(&original, pending, true);
    assert_eq!(saved.to_cli(Some(app::cli::Command::Info)).offline, None);
}
