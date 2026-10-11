//! Bottle defaults are read once, before collecting or executing the owned draft.
use super::{
    BottleInput,
    bottle_form::{self, BottleDefaults, BottleForm, BottleSubmission},
};
use crate::{
    cli::{BottleKind, Units},
    prompt::{self, flow::runner},
    render::format,
    session::Context,
};
use anyhow::Result;
use huckleberry_api::Huckleberry;

/// Records a bottle using the same typed form and executor as the shell.
pub(super) async fn bottle(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    input: BottleInput<'_>,
) -> Result<()> {
    let model = create_model(context, client, cid, input).await?;
    let answer = runner::read(model, huckleberry_api::client::now_seconds(), context.theme)?;
    execute(context, client, cid, answer).await
}

pub(super) async fn create_model(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    input: BottleInput<'_>,
) -> Result<BottleForm> {
    let at = crate::prompt::time::resolve_at(
        context,
        input.at,
        prompt::available() || prompt::host::hosted(),
    )?;
    let units = input
        .units
        .unwrap_or_else(|| Units::from_setting(&context.config.units));
    let defaults = read_defaults(
        client,
        cid,
        units,
        input.amount.is_none() || input.bottle_type.is_none(),
    )
    .await;
    BottleForm::create_bottle(
        context.calendar()?,
        bottle_form::BottleInput {
            at,
            amount: input.amount,
            units: input.units,
            kind: input.bottle_type,
            notes: input.notes.map(str::to_owned),
        },
        defaults,
    )
}

async fn read_defaults(
    client: &Huckleberry,
    cid: &str,
    units: Units,
    needs_defaults: bool,
) -> BottleDefaults {
    let prefs = if needs_defaults {
        client
            .feed_document(cid)
            .await
            .ok()
            .flatten()
            .and_then(|document| document.prefs)
    } else {
        None
    };
    let amount = offered_amount(
        prefs
            .as_ref()
            .and_then(|prefs| prefs.bottle_amount)
            .map(huckleberry_api::models::common::Number::as_f64),
        prefs.as_ref().and_then(|prefs| prefs.bottle_units.as_ref()),
        units,
    );
    let kind = match prefs
        .as_ref()
        .and_then(|prefs| prefs.bottle_type.as_ref())
        .map(huckleberry_api::models::BottleType::as_str)
    {
        Some("Breast Milk") => BottleKind::BreastMilk,
        Some("Cow Milk") => BottleKind::CowMilk,
        _ => BottleKind::Formula,
    };
    BottleDefaults {
        units,
        amount,
        kind,
    }
}

pub(super) async fn execute(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    answer: BottleSubmission,
) -> Result<()> {
    let BottleSubmission {
        at,
        amount,
        units,
        kind,
        notes,
    } = answer;
    client
        .log_bottle_at(
            cid,
            amount,
            kind.to_api(),
            units.to_api(),
            notes.as_deref(),
            at,
        )
        .await?;
    context.receipt(
        "🍼 Bottle recorded",
        &[
            ("When", format::date_time(at, &context.calendar()?)),
            (
                "Amount",
                format!("{} {}", format::amount_in(amount, units), units.as_str()),
            ),
            ("Milk", kind.to_api().to_string()),
            ("Notes", notes.unwrap_or_else(|| "None".into())),
        ],
        &[],
    );
    Ok(())
}

/// The last bottle's amount, in the units this one is being recorded in.
///
/// The app stores that amount in whatever units it was recorded in, so
/// offering it as it stands is how a family who records in ounces is offered
/// "1.15" under a question reading "How much, in ml?".
#[must_use]
pub fn offered_amount(
    last: Option<f64>,
    recorded_in: Option<&huckleberry_api::models::feed::VolumeUnits>,
    wanted: Units,
) -> Option<f64> {
    let recorded_in = recorded_in
        .and_then(|units| Units::from_stored(units.as_str()))
        .unwrap_or(wanted);
    last.map(|amount| format::convert(amount, recorded_in, wanted))
}
