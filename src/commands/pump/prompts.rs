//! Shared pumping questions and the decisions behind their defaults.

use anyhow::{Result, bail};
use huckleberry_api::models::pump::{LastPump, PumpAmounts};
use huckleberry_api::{Huckleberry, models::Number};

use crate::cli::{PumpValues, Units};
use crate::prompt::{self, Choice, Question};
use crate::render::format;
use crate::session::Context;
use crate::theme::Theme;

/// Whether to ask for each side instead of one combined amount.
fn separate_sides(values: &PumpValues, theme: Theme) -> Result<bool> {
    if values.amount.is_some() {
        return Ok(false);
    }
    if values.left.is_some() || values.right.is_some() {
        return Ok(true);
    }
    let question = Question::new(
        "pump amounts",
        "Record which amounts?",
        "--amount <NUMBER> or --left <NUMBER> --right <NUMBER>",
    )
    .with_choices(&[
        Choice {
            value: "total",
            hint: "Total",
        },
        Choice {
            value: "leftright",
            hint: "Left and right",
        },
    ])
    .with_default("total");
    let answer = if prompt::host::hosted() {
        prompt::select::answer(&question, theme)?.ok_or(prompt::Cancelled)?
    } else {
        prompt::ask(&question, theme)?
    };
    Ok(answer == "leftright")
}

/// Converts entered minutes to the seconds stored by the API.
fn duration_seconds(minutes: f64) -> Result<f64> {
    let seconds = minutes * 60.0;
    if !seconds.is_finite() || minutes < 0.0 {
        bail!("pump duration must be a finite number of minutes at least zero");
    }
    Ok(seconds)
}

/// Resolved volumes with one common storage unit.
pub(super) struct Volumes {
    /// The selected entry mode and quantities.
    pub amounts: PumpAmounts,
    /// Units shared by every recorded quantity.
    pub units: Units,
}

/// Reads defaults only when an interactive amount question needs them.
pub(super) async fn volumes(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    values: &PumpValues,
) -> Result<Volumes> {
    let is_separate = separate_sides(values, context.theme)?;
    let has_amounts = values.amount.is_some() || (values.left.is_some() && values.right.is_some());
    let last = if !has_amounts && (prompt::host::hosted() || prompt::available()) {
        client.latest_pump(cid).await.ok().flatten()
    } else {
        None
    };
    resolve_amounts(context, values, last.as_ref(), is_separate)
}

/// Answers the amount questions using the already-read last pump.
fn resolve_amounts(
    context: &Context,
    values: &PumpValues,
    last: Option<&LastPump>,
    is_separate: bool,
) -> Result<Volumes> {
    let configured = recording_units(context, values.units)?;
    if !is_separate {
        let (chosen, amount) = amount(context, values.amount, configured, last, Part::Total)?;
        let units = values.units.unwrap_or(chosen);
        return Ok(Volumes {
            amounts: PumpAmounts::Total(format::convert(amount, chosen, units)),
            units,
        });
    }
    let (left_units, left) = amount(context, values.left, configured, last, Part::Left)?;
    let (right_units, right) = amount(context, values.right, configured, last, Part::Right)?;
    let units = values.units.unwrap_or(left_units);
    Ok(Volumes {
        amounts: PumpAmounts::LeftRight {
            left: format::convert(left, left_units, units),
            right: format::convert(right, right_units, units),
        },
        units,
    })
}

/// The quantity being asked for, including its scriptable flag.
#[derive(Clone, Copy)]
enum Part {
    Total,
    Left,
    Right,
}

/// Offers the matching last amount in the units the new dial starts on.
fn offered(last: Option<&LastPump>, part: Part, units: Units) -> Option<f64> {
    let last = last?;
    let left = last.left_amount.map(Number::as_f64);
    let right = last.right_amount.map(Number::as_f64);
    let amount = match part {
        Part::Total if left.is_some() || right.is_some() => {
            Some(left.unwrap_or(0.0) + right.unwrap_or(0.0))
        }
        Part::Total => None,
        Part::Left => left,
        Part::Right => right,
    }?;
    let previous_units = last
        .units
        .as_ref()
        .and_then(|unit| Units::from_stored(unit.as_str()))
        .unwrap_or(units);
    let converted = format::convert(amount, previous_units, units);
    (converted.is_finite() && converted >= 0.0).then_some(converted)
}

/// Reads one quantity with the shared zero-inclusive volume dial.
fn amount(
    context: &Context,
    given: Option<f64>,
    units: Units,
    last: Option<&LastPump>,
    part: Part,
) -> Result<(Units, f64)> {
    if let Some(amount) = given {
        return Ok((units, amount));
    }
    let (label, flag) = match part {
        Part::Total => ("Total amount?", "--amount <NUMBER>"),
        Part::Left => ("Left amount?", "--left <NUMBER>"),
        Part::Right => ("Right amount?", "--right <NUMBER>"),
    };
    if prompt::host::hosted() {
        return prompt::dial::volume::ask_nonnegative(
            label,
            units,
            offered(last, part, units),
            context.theme,
        );
    }
    let label = format!("{label} In {}.", units.as_str());
    let default = offered(last, part, units).map(|amount| format::amount_in(amount, units));
    let mut question = Question::new("pump amount", &label, flag);
    if let Some(default) = default.as_deref() {
        question = question.with_default(default);
    }
    let answer = prompt::ask(&question, context.theme)?;
    let amount = answer
        .parse()
        .map_err(|_| anyhow::anyhow!("`{answer}` is not a number"))?;
    Ok((units, amount))
}

/// On a bare terminal, units precede the typed amount, as for a bottle.
fn recording_units(context: &Context, given: Option<Units>) -> Result<Units> {
    let configured = Units::from_setting(&context.config.units);
    if let Some(units) = given {
        return Ok(units);
    }
    if prompt::host::hosted() || !prompt::available() {
        return Ok(configured);
    }
    let question = Question::new("units", "In which units?", "--units <UNITS>")
        .with_choices(&[
            Choice {
                value: "ml",
                hint: "Millilitres",
            },
            Choice {
                value: "oz",
                hint: "Fluid ounces",
            },
        ])
        .with_default(configured.as_str());
    Ok(Units::from_setting(&prompt::ask(&question, context.theme)?))
}

/// Resolves an optional note through the same prompt used by other recordings.
pub(super) fn notes(context: &Context, given: Option<&str>) -> Result<Option<String>> {
    if let Some(notes) = given {
        return Ok(Some(notes.to_owned()));
    }
    let question = Question::new("notes", "Anything to note?", "--notes <TEXT>").optional();
    prompt::ask_optional(&question, context.theme)
}

/// Reads optional minutes while retaining the API's seconds as the result.
pub(super) fn duration(context: &Context, given: Option<f64>) -> Result<Option<f64>> {
    if let Some(minutes) = given {
        return duration_seconds(minutes).map(Some);
    }
    if !prompt::host::hosted() && !prompt::available() {
        return Ok(None);
    }
    let presets = [0.0, 5.0, 10.0, 15.0, 20.0, 30.0];
    let items = std::iter::once("Skip".to_owned())
        .chain(presets.iter().map(|minutes| format!("{minutes} minutes")))
        .chain(std::iter::once("Enter minutes".to_owned()))
        .map(|label| prompt::select::MenuItem {
            label,
            detail: None,
        })
        .collect::<Vec<_>>();
    let chosen = prompt::select::choose("How long?", &items, 0, context.theme)?;
    if chosen == 0 {
        return Ok(None);
    }
    let minutes = if let Some(minutes) = presets.get(chosen - 1) {
        Some(*minutes)
    } else {
        custom_minutes(context)?
    };
    minutes.map(duration_seconds).transpose()
}

/// Uses the shared optional text field for a duration outside the presets.
fn custom_minutes(context: &Context) -> Result<Option<f64>> {
    let question =
        Question::new("duration", "How many minutes?", "--duration <MINUTES>").optional();
    prompt::ask_optional(&question, context.theme)?
        .map(|typed| {
            typed
                .parse()
                .map_err(|_| anyhow::anyhow!("`{typed}` is not a number of minutes"))
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompt::host::{self, Input, Reply, Request};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn one_supplied_side_selects_separate_amounts() {
        let values = PumpValues {
            left: Some(0.0),
            ..PumpValues::default()
        };
        assert!(separate_sides(&values, Theme::dark(false)).expect("a mode"));
    }

    #[test]
    fn fractional_minutes_become_seconds_and_zero_is_valid() {
        assert!((duration_seconds(1.25).expect("a duration") - 75.0).abs() < f64::EPSILON);
        assert!(duration_seconds(0.0).expect("zero duration").abs() < f64::EPSILON);
    }

    #[test]
    fn invalid_minutes_never_reach_the_api() {
        for minutes in [-1.0, f64::INFINITY, f64::NAN, f64::MAX] {
            assert!(duration_seconds(minutes).is_err(), "accepted {minutes}");
        }
    }

    #[test]
    fn the_shell_offers_total_or_separate_sides_before_amounts() {
        let _serial = host::one_at_a_time();
        let channel = host::install();
        let asking =
            std::thread::spawn(|| separate_sides(&PumpValues::default(), Theme::dark(false)));
        let request = channel
            .requests
            .recv_timeout(std::time::Duration::from_secs(1));
        let Ok((Request::Frame(lines), reply)) = request else {
            host::remove();
            panic!("the shell must offer the amount mode");
        };
        assert!(lines.iter().any(|line| line.contains("Total")), "{lines:?}");
        assert!(
            lines.iter().any(|line| line.contains("Left and right")),
            "{lines:?}"
        );
        reply
            .send(Reply::Input(Input::Key(KeyEvent::new(
                KeyCode::Down,
                KeyModifiers::NONE,
            ))))
            .expect("listening");
        let (_, reply) = channel.requests.recv().expect("a frame");
        reply
            .send(Reply::Input(Input::Key(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::NONE,
            ))))
            .expect("listening");
        let result = asking.join().expect("ends");
        host::remove();
        assert!(result.expect("the side mode"));
    }

    fn context() -> Context {
        Context {
            config: crate::config::Config::default(),
            config_path: "unused-config.toml".into(),
            credentials_path: "unused-credentials.toml".into(),
            theme: Theme::dark(false),
            verbose: false,
            child_override: None,
            offline: None,
        }
    }

    #[test]
    fn explicit_amounts_keep_zero_and_use_configured_units_without_questions() {
        let values = PumpValues {
            left: Some(0.0),
            right: Some(2.5),
            ..PumpValues::default()
        };
        let mut context = context();
        context.config.units = "oz".into();
        let resolved = resolve_amounts(&context, &values, None, true).expect("resolved");
        assert_eq!(
            resolved.amounts,
            PumpAmounts::LeftRight {
                left: 0.0,
                right: 2.5
            }
        );
        assert_eq!(resolved.units, Units::Oz);
    }

    #[test]
    fn last_total_combines_both_stored_halves_and_converts_to_configured_units() {
        use huckleberry_api::models::{Number, feed::VolumeUnits};
        let _serial = host::one_at_a_time();
        let channel = host::install();
        let asking = std::thread::spawn(|| {
            let last = LastPump {
                left_amount: Some(Number::Float(1.0)),
                right_amount: Some(Number::Float(1.0)),
                units: Some(VolumeUnits::Ounces),
                ..LastPump::default()
            };
            resolve_amounts(&context(), &PumpValues::default(), Some(&last), false)
        });
        let request = channel
            .requests
            .recv_timeout(std::time::Duration::from_secs(1));
        let Ok((Request::Frame(_), reply)) = request else {
            host::remove();
            panic!("the shell must offer a total amount");
        };
        reply
            .send(Reply::Input(Input::Key(KeyEvent::new(
                KeyCode::Enter,
                KeyModifiers::NONE,
            ))))
            .expect("listening");
        let result = asking.join().expect("ends");
        host::remove();
        let resolved = result.expect("a volume");
        assert_eq!(resolved.units, Units::Ml);
        assert_eq!(resolved.amounts, PumpAmounts::Total(59.0));
    }

    #[test]
    fn a_side_dial_can_change_units_without_changing_the_other_sides_quantity() {
        let _serial = host::one_at_a_time();
        let channel = host::install();
        let asking = std::thread::spawn(|| {
            let values = PumpValues {
                left: Some(0.0),
                units: Some(Units::Ml),
                ..PumpValues::default()
            };
            resolve_amounts(&context(), &values, None, true)
        });
        for key in [KeyCode::Right, KeyCode::Down, KeyCode::Enter] {
            answer_dial_frame(&channel.requests, key);
        }
        let result = asking.join().expect("ends");
        host::remove();
        let resolved = result.expect("side amounts");
        let PumpAmounts::LeftRight { left, right } = resolved.amounts else {
            panic!("separate amounts");
        };
        assert_eq!(resolved.units, Units::Ml);
        assert!(left.abs() < f64::EPSILON);
        assert!((right - 59.147_059).abs() < 0.001, "{right}");
    }

    fn answer_dial_frame(
        requests: &std::sync::mpsc::Receiver<(Request, std::sync::mpsc::SyncSender<Reply>)>,
        key: KeyCode,
    ) {
        loop {
            let request = requests.recv_timeout(std::time::Duration::from_secs(1));
            let Ok((request, reply)) = request else {
                host::remove();
                panic!("the missing side needs its volume dial");
            };
            match request {
                Request::Frame(_) => {
                    reply
                        .send(Reply::Input(Input::Key(KeyEvent::new(
                            key,
                            KeyModifiers::NONE,
                        ))))
                        .expect("listening");
                    break;
                }
                Request::Step(_) => {
                    reply
                        .send(Reply::Stepped { interrupted: true })
                        .expect("animating");
                }
                Request::Show(_) => {
                    reply.send(Reply::Shown).expect("showing");
                }
            }
        }
    }
}
