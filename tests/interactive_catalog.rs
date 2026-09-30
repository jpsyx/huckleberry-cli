use app::cli::{Cli, Command};
use app::interactive::{catalog, draft::CommandDraft, session::SessionOptions};
use clap::CommandFactory;
use std::collections::BTreeSet;

fn leaves(command: &clap::Command, prefix: &[String], output: &mut BTreeSet<Vec<String>>) {
    for child in command
        .get_subcommands()
        .filter(|child| child.get_name() != "help")
    {
        let mut path = prefix.to_vec();
        path.push(child.get_name().into());
        if child.has_subcommands() {
            leaves(child, &path, output);
        } else {
            output.insert(path);
        }
    }
}
#[test]
fn home_order_and_all_commands_are_reachable() {
    let home = catalog::entries(catalog::MenuId::Home);
    assert_eq!(
        home.iter()
            .map(|entry| entry.label.as_str())
            .collect::<Vec<_>>(),
        [
            "Log a diaper",
            "Log a feed",
            "Log sleep",
            "Edit",
            "Visualizations",
            "View logs",
            "Other logging",
            "Delete",
            "More"
        ]
    );
    let mut actual = BTreeSet::new();
    leaves(&Cli::command(), &[], &mut actual);
    assert_eq!(
        catalog::command_paths()
            .into_iter()
            .map(|path| path.0)
            .collect::<BTreeSet<_>>(),
        actual
    );
}

#[test]
fn first_home_item_resolves_to_a_diaper() {
    let home = catalog::entries(catalog::MenuId::Home);
    let catalog::MenuTarget::Command(path) = &home[0].target else {
        panic!("first home item must run a command");
    };
    let command = CommandDraft::new(path.clone())
        .resolve(&SessionOptions::default())
        .unwrap()
        .command;
    assert!(matches!(command, Some(Command::Diaper { .. })));
}
#[test]
fn drafts_resolve_repeated_values_and_positionals() {
    let mut draft = CommandDraft::new(catalog::CommandPath(vec!["feed".into(), "solids".into()]));
    draft.set("foods", vec!["Avocado".into(), "Banana".into()]);
    draft.set("at", vec!["40 minutes ago.".into()]);
    let cli = draft.resolve(&SessionOptions::default()).unwrap();
    let Command::Feed {
        action: app::cli::FeedAction::Solids { foods, at, .. },
    } = cli.command.unwrap()
    else {
        panic!("meal");
    };
    assert_eq!(foods, ["Avocado", "Banana"]);
    assert_eq!(at.as_deref(), Some("40 minutes ago."));
    let mut draft = CommandDraft::new(catalog::CommandPath(vec!["config".into(), "set".into()]));
    draft.set("key", vec!["units".into()]);
    draft.set("value", vec!["oz".into()]);
    assert!(matches!(
        draft
            .resolve(&SessionOptions::default())
            .unwrap()
            .command
            .unwrap(),
        Command::Config {
            action: app::cli::ConfigAction::Set {
                key: Some(_),
                value: Some(_)
            }
        }
    ));
}
#[test]
fn draft_aliases_and_invalid_choices_use_real_clap_validation() {
    let end = CommandDraft::new(catalog::CommandPath(vec!["sleep".into(), "end".into()]));
    let stop = CommandDraft::new(catalog::CommandPath(vec!["sleep".into(), "stop".into()]));
    assert_eq!(
        end.resolve(&SessionOptions::default())
            .unwrap()
            .command
            .unwrap(),
        stop.resolve(&SessionOptions::default())
            .unwrap()
            .command
            .unwrap()
    );
    let mut diaper = CommandDraft::new(catalog::CommandPath(vec!["diaper".into()]));
    diaper.set("mode", vec!["nonsense".into()]);
    assert!(diaper.resolve(&SessionOptions::default()).is_err());
}
