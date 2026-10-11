//! `growth`: weight, length and head circumference.

use anyhow::{Result, bail};
use huckleberry_api::GrowthMeasurements;

use super::growth_form::{GrowthForm, GrowthInput, GrowthSubmission};
use super::prepared::{CommandProgress, PreparedCommand};
use crate::cli::System;
use crate::prompt::{self, flow::runner};
use crate::session::Context;

/// Records a measurement, asking for everything that was not given.
///
/// Ask when first, then every measurement the app takes: which system, then the
/// weight, the length and the head. Only the weight has to be answered, and
/// only when nothing at all was passed: a parent who came to record a weight
/// should not have to produce a tape measure.
pub async fn run(
    context: &Context,
    weight: Option<f64>,
    height: Option<f64>,
    head: Option<f64>,
    units: Option<System>,
    at: Option<&str>,
) -> Result<()> {
    let (client, cid) = super::client_and_child(context).await?;
    let input = create_input(context, weight, height, head, units, at)?;
    let answer = runner::read(
        create_model(context, input)?,
        huckleberry_api::client::now_seconds(),
        context.theme,
    )?;
    execute(context, &client, &cid, answer).await?;
    super::persist_session(context, &client).await
}

pub(crate) fn create_input(
    context: &Context,
    weight: Option<f64>,
    height: Option<f64>,
    head: Option<f64>,
    system: Option<System>,
    at: Option<&str>,
) -> Result<GrowthInput> {
    Ok(GrowthInput {
        at: prompt::time::resolve_at(context, at, prompt::available() || prompt::host::hosted())?,
        measurements: GrowthMeasurements {
            weight,
            height,
            head,
        },
        system,
    })
}
fn create_model(context: &Context, input: GrowthInput) -> Result<GrowthForm> {
    GrowthForm::create_growth(
        context.calendar()?,
        input,
        System::from_setting(&context.config.measurements),
    )
}
/// Prepares one measurement operation without writing before completion.
pub(crate) fn prepare_with_client(
    context: Context,
    client: huckleberry_api::Huckleberry,
    cid: String,
    input: GrowthInput,
    now: f64,
) -> Result<PreparedCommand> {
    PreparedCommand::create_prepared(
        create_model(&context, input)?,
        now,
        move |answer| async move {
            execute(&context, &client, &cid, answer).await?;
            super::persist_session(&context, &client).await?;
            Ok(CommandProgress::Finished { home: true })
        },
    )
}
async fn execute(
    context: &Context,
    client: &huckleberry_api::Huckleberry,
    cid: &str,
    answer: GrowthSubmission,
) -> Result<()> {
    let GrowthSubmission {
        at,
        measurements,
        system,
    } = answer;
    client
        .log_growth_at(cid, &measurements, system.to_api(), at)
        .await?;
    let (weight_unit, length_unit) = match system {
        System::Metric => ("kg", "cm"),
        System::Imperial => ("lb", "in"),
    };
    let mut fields: Vec<(&str, String)> = [
        ("Weight", measurements.weight, weight_unit),
        ("Length", measurements.height, length_unit),
        ("Head circumference", measurements.head, length_unit),
    ]
    .into_iter()
    .filter_map(|(label, amount, unit)| amount.map(|amount| (label, format!("{amount} {unit}"))))
    .collect();
    fields.push((
        "When",
        crate::render::format::date_time(at, &context.calendar()?),
    ));
    context.receipt("📏 Growth recorded", &fields, &[]);
    Ok(())
}

/// Refuses a measurement that cannot be right.
///
/// Not a health judgement: these bounds only catch a decimal point in the
/// wrong place or pounds typed where kilograms were expected, which is a
/// mistake worth catching before it is in the record.
pub(super) fn validate(measurements: &GrowthMeasurements) -> Result<()> {
    for (name, value, limit) in [
        ("weight", measurements.weight, 200.0),
        ("height", measurements.height, 300.0),
        ("head", measurements.head, 300.0),
    ] {
        let Some(value) = value else { continue };
        if value.partial_cmp(&0.0) != Some(core::cmp::Ordering::Greater) || value > limit {
            bail!("`{value}` is not a plausible {name}");
        }
    }
    Ok(())
}

/// What was recorded, as a person reads it.
#[must_use]
pub fn describe(measurements: &GrowthMeasurements, system: System) -> String {
    let (weight, length) = match system {
        System::Metric => ("kg", "cm"),
        System::Imperial => ("lb", "in"),
    };
    let parts: Vec<String> = [
        measurements.weight.map(|value| format!("{value} {weight}")),
        measurements
            .height
            .map(|value| format!("{value} {length} long")),
        measurements
            .head
            .map(|value| format!("{value} {length} head")),
    ]
    .into_iter()
    .flatten()
    .collect();
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_was_measured_is_described_in_the_system_it_was_measured_in() {
        let measurements = GrowthMeasurements {
            weight: Some(3.4),
            height: Some(52.0),
            head: None,
        };
        assert_eq!(
            describe(&measurements, System::Metric),
            "3.4 kg, 52 cm long"
        );
        assert_eq!(
            describe(&measurements, System::Imperial),
            "3.4 lb, 52 in long"
        );
    }

    #[test]
    fn a_measurement_with_the_decimal_point_in_the_wrong_place_is_refused() {
        assert!(
            validate(&GrowthMeasurements {
                weight: Some(340.0),
                ..GrowthMeasurements::default()
            })
            .is_err()
        );
    }

    #[test]
    fn a_measurement_of_nothing_is_refused() {
        assert!(
            validate(&GrowthMeasurements {
                weight: Some(0.0),
                ..GrowthMeasurements::default()
            })
            .is_err()
        );
        assert!(
            validate(&GrowthMeasurements {
                head: Some(-1.0),
                ..GrowthMeasurements::default()
            })
            .is_err()
        );
    }

    #[test]
    fn a_plausible_measurement_is_accepted() {
        assert!(
            validate(&GrowthMeasurements {
                weight: Some(3.4),
                height: Some(52.0),
                head: Some(35.0),
            })
            .is_ok()
        );
    }
}
