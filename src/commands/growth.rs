//! `growth`: weight, length and head circumference.

use anyhow::{Result, bail};
use huckleberry_api::GrowthMeasurements;

use crate::cli::System;
use crate::prompt::{self, Choice, Question};
use crate::session::Context;

/// Records a measurement, asking for everything that was not given.
///
/// Every measurement the app takes, in its own order: which system, then the
/// weight, the length and the head. Only the weight has to be answered, and
/// only when nothing at all was passed: a parent who came to record a weight
/// should not have to produce a tape measure.
pub async fn run(
    context: &Context,
    weight: Option<f64>,
    height: Option<f64>,
    head: Option<f64>,
    units: Option<System>,
) -> Result<()> {
    let (client, cid) = super::client_and_child(context).await?;
    let nothing_given = weight.is_none() && height.is_none() && head.is_none();
    let configured = System::from_setting(&context.config.measurements);
    // The `measurements` setting is the default, not the answer.
    let system = match units {
        Some(given) => given,
        None => ask_for_system(context, configured)?,
    };

    let mut measurements = GrowthMeasurements {
        weight,
        height,
        head,
    };
    if nothing_given {
        measurements.weight = Some(ask_for_weight(context, system)?);
    }
    if measurements.height.is_none() {
        measurements.height = ask_for_length(context, system, "Length?", "--height <NUMBER>")?;
    }
    if measurements.head.is_none() {
        measurements.head =
            ask_for_length(context, system, "Head circumference?", "--head <NUMBER>")?;
    }
    validate(&measurements)?;

    client
        .log_growth(&cid, &measurements, system.to_api())
        .await?;
    super::persist_session(context, &client).await?;
    let (weight_unit, length_unit) = match system {
        System::Metric => ("kg", "cm"),
        System::Imperial => ("lb", "in"),
    };
    let fields: Vec<(&str, String)> = [
        ("Weight", measurements.weight, weight_unit),
        ("Length", measurements.height, length_unit),
        ("Head circumference", measurements.head, length_unit),
    ]
    .into_iter()
    .filter_map(|(label, amount, unit)| amount.map(|amount| (label, format!("{amount} {unit}"))))
    .collect();
    context.receipt("📏 Growth recorded", &fields, &[]);
    Ok(())
}

/// Which system the numbers are in.
fn ask_for_system(context: &Context, configured: System) -> Result<System> {
    const CHOICES: [Choice<'static>; 2] = [
        Choice {
            value: "metric",
            hint: "Kilograms and centimetres",
        },
        Choice {
            value: "imperial",
            hint: "Pounds and inches",
        },
    ];
    let question = Question::new("units", "In which units?", "--units <SYSTEM>")
        .with_choices(&CHOICES)
        .with_default(configured.as_str());
    Ok(System::from_setting(&prompt::ask(
        &question,
        context.theme,
    )?))
}

/// A length, which is optional: most visits weigh and nothing else.
fn ask_for_length(
    context: &Context,
    system: System,
    label: &str,
    flag: &str,
) -> Result<Option<f64>> {
    let unit = match system {
        System::Metric => "centimetres",
        System::Imperial => "inches",
    };
    let label = format!("{label} In {unit}.");
    let question = Question::new("length", &label, flag).optional();
    let Some(answer) = prompt::ask_optional(&question, context.theme)? else {
        return Ok(None);
    };
    answer
        .trim()
        .parse()
        .map(Some)
        .map_err(|_| anyhow::anyhow!("`{answer}` is not a number"))
}

/// Refuses a measurement that cannot be right.
///
/// Not a health judgement: these bounds only catch a decimal point in the
/// wrong place or pounds typed where kilograms were expected, which is a
/// mistake worth catching before it is in the record.
fn validate(measurements: &GrowthMeasurements) -> Result<()> {
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

fn ask_for_weight(context: &Context, system: System) -> Result<f64> {
    let unit = match system {
        System::Metric => "kilograms",
        System::Imperial => "pounds",
    };
    let label = format!("Weight, in {unit}?");
    let question = Question::new("weight", &label, "--weight <NUMBER>");
    let answer = prompt::ask(&question, context.theme)?;
    answer
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("`{answer}` is not a number"))
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
