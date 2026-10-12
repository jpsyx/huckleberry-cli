//! Every clap leaf has an intentional collection/execution boundary.
use super::support::Fixture;
use crate::{
    commands::prepared,
    interactive::{catalog, draft::CommandDraft, session::SessionOptions},
};
const READERS: &[&str] = &[
    "now",
    "dash",
    "summary",
    "trends",
    "stripes",
    "log",
    "info",
    "auth status",
    "child list",
    "child show",
    "config show",
    "config path",
    "foods list",
    "sleep status",
    "pump status",
    "feed nursing status",
];

const IMMEDIATE: &[&str] = &[
    "auth logout",
    "export",
    "sleep cancel",
    "pump cancel",
    "feed nursing cancel",
];

const FORMS: &[&str] = &[
    "diaper",
    "potty",
    "growth",
    "edit",
    "delete",
    "auth login",
    "child use",
    "config set",
    "foods add",
    "feed bottle",
    "feed solids",
    "feed nursing start",
    "feed nursing pause",
    "feed nursing resume",
    "feed nursing stop",
    "feed nursing switch",
    "pump log",
    "pump start",
    "pump pause",
    "pump resume",
    "pump stop",
    "sleep start",
    "sleep manual",
    "sleep pause",
    "sleep resume",
    "sleep stop",
];

#[tokio::test]
async fn every_prompting_command_has_a_retained_form() {
    let fixture = Fixture::create_fixture("catalog");
    let mut paths = catalog::command_paths();
    // Summary is permanent in the shell, but remains a standalone command.
    paths.push(catalog::CommandPath(vec!["summary".into()]));
    assert_eq!(paths.len(), READERS.len() + IMMEDIATE.len() + FORMS.len());
    for path in paths {
        let name = path.0.join(" ");
        assert!(
            READERS.contains(&name.as_str())
                || IMMEDIATE.contains(&name.as_str())
                || FORMS.contains(&name.as_str()),
            "unclassified command: {name}"
        );
        let command = CommandDraft::new(path)
            .resolve(&SessionOptions::default())
            .unwrap()
            .command
            .unwrap();
        let mut context = fixture.context();
        context.offline = Some("unused-catalog-snapshot".into());
        for (key, value) in [
            ("day_start", "07:00"),
            ("day_end", "20:00"),
            ("day_mode", "discrete"),
        ] {
            context.config.set(key, value).unwrap();
        }
        // Child selection uses its account reader, so an invalid local zone also
        // prevents opening a real connection if test runners supply credentials.
        if name == "child use" {
            context.config.timezone = "Invalid/Test".into();
        }
        let prepared = prepared::prepare(context, command, 10000.0).await;
        if READERS.contains(&name.as_str()) {
            assert!(
                matches!(prepared, Ok(None)),
                "reader should use its existing viewer: {name}"
            );
        } else {
            match prepared {
                Ok(Some(command)) => {
                    if ["auth logout", "export"].contains(&name.as_str()) {
                        assert!(command.is_ready(), "{name}");
                    } else {
                        assert!(!command.is_ready(), "{name}");
                    }
                }
                // Live FORMS must reach their preparer's client boundary, never legacy dispatch.
                Err(error) => assert!(
                    name == "child use" || error.to_string().contains("nothing to write to"),
                    "{name}: {error:#}"
                ),
                Ok(None) => panic!("{name} bypasses retained preparation"),
            }
        }
    }
}
