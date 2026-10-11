//! The corrected diaper journey reaches exactly one history write.
use super::support::{self, Fixture};
use crate::{
    commands::{diaper, diaper_form::DiaperInput, prepared::PreparedCommand},
    prompt::{flow::FormEvent, host::Input},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;

fn press(form: &mut PreparedCommand, code: KeyCode) -> FormEvent {
    form.apply(
        Input::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        10000.0,
        (80, 24),
    )
}
fn choose(form: &mut PreparedCommand, digit: char) {
    press(form, KeyCode::Char(digit));
    press(form, KeyCode::Enter);
}
fn draft(fixture: &Fixture, stub: &support::api::Stub, potty: bool) -> PreparedCommand {
    diaper::prepare_with_client(
        fixture.context(),
        support::client(stub),
        "test-child".into(),
        DiaperInput {
            potty,
            ..DiaperInput::default()
        },
        10000.0,
    )
    .unwrap()
}

#[test]
fn a_time_correction_records_the_retained_colour_and_note_once() {
    let (requests, shown) = support::hosted(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let fixture = Fixture::create_fixture("diaper");
            let stub =
                support::api::Stub::start(vec![json!({"fields":{}}), json!({}), json!({})]).await;
            let mut form = draft(&fixture, &stub, false);
            press(&mut form, KeyCode::Enter);
            choose(&mut form, '2'); // dirty
            choose(&mut form, '3'); // medium amount
            choose(&mut form, '3'); // brown
            press(&mut form, KeyCode::Enter); // consistency
            press(&mut form, KeyCode::Enter); // rash
            form.apply(Input::Pasted("complete note".into()), 10000.0, (80, 24));
            for _ in 0..6 {
                press(&mut form, KeyCode::Left);
            }
            for _ in 0..20 {
                press(&mut form, KeyCode::Up);
            }
            for _ in 0..6 {
                press(&mut form, KeyCode::Enter);
            }
            assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
            assert!(stub.requests().await.is_empty());
            form.execute().await?;
            Ok(stub.requests().await)
        })
    });
    let entries = requests
        .iter()
        .filter(|request| request.path.contains("/intervals/"))
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1);
    let recorded = entries[0].document();
    assert_eq!(recorded["start"], 8800.0);
    assert_eq!(recorded["color"], "brown");
    assert_eq!(recorded["notes"], "complete note");
    assert!(shown.iter().any(|line| line.contains("Diaper recorded")));
}

#[test]
fn leaving_a_diaper_or_potty_draft_never_mutates_an_entry() {
    let _serial = crate::prompt::host::one_at_a_time();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let fixture = Fixture::create_fixture("diaper-cancel");
        let stub = support::api::Stub::start(vec![]).await;
        for potty in [false, true] {
            for code in [KeyCode::Left, KeyCode::Esc] {
                let mut form = draft(&fixture, &stub, potty);
                // A relative time opens on minutes; Left first selects hours.
                if code == KeyCode::Left {
                    press(&mut form, code);
                }
                assert!(matches!(
                    press(&mut form, code),
                    FormEvent::Suspend | FormEvent::Home
                ));
            }
        }
        assert!(stub.requests().await.is_empty());
    });
}

#[test]
fn potty_records_the_retained_outcome_without_a_rash_field() {
    let (requests, _) = support::hosted(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let fixture = Fixture::create_fixture("potty");
            let stub =
                support::api::Stub::start(vec![json!({"fields":{}}), json!({}), json!({})]).await;
            let mut form = draft(&fixture, &stub, true);
            press(&mut form, KeyCode::Enter);
            choose(&mut form, '2');
            choose(&mut form, '3');
            press(&mut form, KeyCode::Left);
            press(&mut form, KeyCode::Enter);
            for _ in 0..3 {
                press(&mut form, KeyCode::Enter);
            }
            assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
            form.execute().await?;
            Ok(stub.requests().await)
        })
    });
    let entries = requests
        .iter()
        .filter(|request| request.path.contains("/intervals/"))
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1);
    let recorded = entries[0].document();
    assert_eq!(recorded["howItHappened"], "accident");
    assert_eq!(recorded["isPotty"], true);
    assert!(recorded.get("diaperRash").is_none());
}
