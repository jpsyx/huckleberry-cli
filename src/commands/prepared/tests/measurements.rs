//! Meal and growth navigation collects everything before their history write.
use super::support::{self, Fixture};
use crate::{
    cli::FeedAction,
    commands::{feed::prepare, growth, growth_form::GrowthInput, prepared::PreparedCommand},
    prompt::{flow::FormEvent, host::Input},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use huckleberry_api::GrowthMeasurements;
use serde_json::json;
fn press(form: &mut PreparedCommand, code: KeyCode) -> FormEvent {
    form.apply(
        Input::Key(KeyEvent::new(code, KeyModifiers::NONE)),
        10000.0,
        (80, 24),
    )
}
#[test]
fn meal_catalog_is_read_once_and_back_keeps_the_custom_helping() {
    let (requests, _) = support::hosted(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let fixture = Fixture::create_fixture("meal");
            let stub = support::api::Stub::start(vec![
                json!({"documents":[]}),
                json!({"fields":{}}),
                json!({}),
                json!({}),
            ])
            .await;
            let mut form = prepare::prepare_with_client(
                fixture.context(),
                support::client(&stub),
                "test-child".into(),
                FeedAction::Solids {
                    at: None,
                    foods: vec!["Avocado".into(), "Pear".into()],
                    amount: None,
                    reaction: None,
                    notes: None,
                },
                10000.0,
            )
            .await?;
            press(&mut form, KeyCode::Enter);
            press(&mut form, KeyCode::Down);
            press(&mut form, KeyCode::Enter);
            form.apply(Input::Pasted("half".into()), 10000.0, (80, 24));
            press(&mut form, KeyCode::Left);
            press(&mut form, KeyCode::Enter);
            press(&mut form, KeyCode::Enter);
            press(&mut form, KeyCode::Enter);
            assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
            assert_eq!(stub.requests().await.len(), 1);
            form.execute().await?;
            Ok(stub.requests().await)
        })
    });
    assert_eq!(
        requests
            .iter()
            .filter(|request| request.path.contains("/intervals/"))
            .count(),
        1
    );
    let entry = requests
        .iter()
        .find(|request| request.path.contains("/intervals/"))
        .unwrap()
        .document();
    assert_eq!(entry["foods"]["Avocado"]["amount"], "half");
}
#[test]
fn growth_records_converted_quantities_only_once() {
    let (requests, _) = support::hosted(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let fixture = Fixture::create_fixture("growth");
            let stub =
                support::api::Stub::start(vec![json!({"fields":{}}), json!({}), json!({})]).await;
            let mut form = growth::prepare_with_client(
                fixture.context(),
                support::client(&stub),
                "test-child".into(),
                GrowthInput {
                    at: None,
                    measurements: GrowthMeasurements::default(),
                    system: None,
                },
                10000.0,
            )?;
            press(&mut form, KeyCode::Enter);
            press(&mut form, KeyCode::Enter);
            form.apply(Input::Pasted("0.45359237".into()), 10000.0, (80, 24));
            press(&mut form, KeyCode::Enter);
            press(&mut form, KeyCode::Left);
            press(&mut form, KeyCode::Left);
            press(&mut form, KeyCode::Down);
            press(&mut form, KeyCode::Enter);
            press(&mut form, KeyCode::Enter);
            press(&mut form, KeyCode::Enter);
            assert_eq!(press(&mut form, KeyCode::Enter), FormEvent::Ready);
            assert!(stub.requests().await.is_empty());
            form.execute().await?;
            Ok(stub.requests().await)
        })
    });
    let entries = requests
        .iter()
        .filter(|request| request.document().get("weight").is_some())
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].document()["weight"], 1.0);
}
