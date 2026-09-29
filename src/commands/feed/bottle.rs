//! Bottle recording and defaults.
use super::{
    BottleInput, BottleKind, Choice, Context, Huckleberry, Question, Result, Units, ask_for_notes,
    bail, format, prompt,
};
/// Records a bottle, asking for everything that was not given.
///
/// Every question a bottle has, starting with when, then which units,
/// how much, what was in it, and anything to note. A flag answers its own
/// question and no others, so the fast path stays one line and the slow path
/// never leaves a field unrecordable.
pub(super) async fn bottle(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    input: BottleInput<'_>,
) -> Result<()> {
    let BottleInput {
        amount,
        bottle_type,
        units,
        notes,
        at,
    } = input;
    let at = prompt::time::read_at(context, at)?;
    // The `units` setting is the default, not the answer: somebody who mostly
    // records in ounces still gives the odd bottle in millilitres.
    let configured = Units::from_setting(&context.config.units);
    let units = match units {
        Some(given) => given,
        None => ask_for_units(context, configured)?,
    };
    let amount = match amount {
        Some(given) => given,
        None => ask_for_amount(context, client, cid, units).await?,
    };
    if amount.partial_cmp(&0.0) != Some(core::cmp::Ordering::Greater) {
        bail!("a bottle needs an amount greater than zero, not {amount}");
    }
    let kind = match bottle_type {
        Some(given) => given,
        None => ask_for_bottle_type(context, client, cid).await?,
    };
    let notes = match notes {
        Some(given) => Some(given.to_owned()),
        None => ask_for_notes(context)?,
    };

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

/// Which units this bottle is being recorded in.
///
/// Asked every time, because the answer is about this bottle and not about the
/// family: the `units` setting is what Enter takes, and
/// `h config set units oz` is how somebody changes what Enter takes.
fn ask_for_units(context: &Context, configured: Units) -> Result<Units> {
    const CHOICES: [Choice<'static>; 2] = [
        Choice {
            value: "ml",
            hint: "Millilitres",
        },
        Choice {
            value: "oz",
            hint: "Fluid ounces",
        },
    ];
    let question = Question::new("units", "In which units?", "--units <UNITS>")
        .with_choices(&CHOICES)
        .with_default(configured.as_str());
    Ok(Units::from_setting(&prompt::ask(&question, context.theme)?))
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

/// Offers the last bottle's amount as the default, which is almost always the
/// right answer and saves the typing that matters at 3am.
async fn ask_for_amount(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    units: Units,
) -> Result<f64> {
    let prefs = client
        .feed_document(cid)
        .await
        .ok()
        .flatten()
        .and_then(|document| document.prefs);
    let last = offered_amount(
        prefs
            .as_ref()
            .and_then(|prefs| prefs.bottle_amount)
            .map(huckleberry_api::models::common::Number::as_f64),
        prefs.as_ref().and_then(|prefs| prefs.bottle_units.as_ref()),
        units,
    )
    .map(|amount| format::amount_in(amount, units));

    let label = format!("How much, in {}?", units.as_str());
    let mut question = Question::new("amount", &label, "--amount <NUMBER>");
    if let Some(default) = &last {
        question = question.with_default(default);
    }
    let answer = prompt::ask(&question, context.theme)?;
    answer
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("`{answer}` is not a number"))
}

/// Offers the bottle kinds, with the last one used first.
async fn ask_for_bottle_type(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
) -> Result<BottleKind> {
    const CHOICES: [Choice<'static>; 4] = [
        Choice {
            value: "formula",
            hint: "Formula",
        },
        Choice {
            value: "breast-milk",
            hint: "Expressed breast milk",
        },
        Choice {
            value: "cow-milk",
            hint: "Cow milk",
        },
        Choice {
            value: "other",
            hint: "Something else",
        },
    ];

    let last = client
        .feed_document(cid)
        .await
        .ok()
        .flatten()
        .and_then(|document| document.prefs)
        .and_then(|prefs| prefs.bottle_type)
        .map(|kind| kind.as_str().to_owned());

    let default = match last.as_deref() {
        Some("Breast Milk") => Some("breast-milk"),
        Some("Cow Milk") => Some("cow-milk"),
        Some("Formula") => Some("formula"),
        _ => None,
    };
    let mut question =
        Question::new("bottle type", "What was in it?", "--type <KIND>").with_choices(&CHOICES);
    if let Some(value) = default {
        question = question.with_default(value);
    }
    let answer = prompt::ask(&question, context.theme)?;
    Ok(match answer.as_str() {
        "breast-milk" => BottleKind::BreastMilk,
        "cow-milk" => BottleKind::CowMilk,
        "other" => BottleKind::Other,
        _ => BottleKind::Formula,
    })
}
