//! `feed`: bottles, the nursing timer, and meals.

use anyhow::{Result, bail};
use huckleberry_api::client::now_seconds;
use huckleberry_api::models::feed::FeedSide;
use huckleberry_api::models::solids::FoodReference;
use huckleberry_api::{Huckleberry, TimerChange};

use crate::cli::{BottleKind, FeedAction, NursingAction, Reaction, Side, Units};
use crate::domain::time::format_duration;
use crate::prompt::{self, Choice, Question};
use crate::render::format;
use crate::session::Context;

/// Runs the chosen action.
pub async fn run(context: &Context, action: &FeedAction) -> Result<()> {
    let (client, cid) = super::client_and_child(context).await?;
    let outcome = match action {
        FeedAction::Bottle {
            amount,
            bottle_type,
            units,
            notes,
        } => {
            bottle(
                context,
                &client,
                &cid,
                *amount,
                *bottle_type,
                *units,
                notes.as_deref(),
            )
            .await
        }
        FeedAction::Nursing { action } => nursing(context, &client, &cid, action).await,
        FeedAction::Solids {
            foods,
            amount,
            reaction,
            notes,
        } => {
            solids(
                context,
                &client,
                &cid,
                foods,
                amount.as_deref(),
                *reaction,
                notes.as_deref(),
            )
            .await
        }
    };
    super::persist_session(context, &client).await?;
    outcome
}

/// Records a bottle, asking for everything that was not given.
///
/// Every question a bottle has, in the order the app asks them: which units,
/// how much, what was in it, and anything to note. A flag answers its own
/// question and no others, so the fast path stays one line and the slow path
/// never leaves a field unrecordable.
async fn bottle(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    amount: Option<f64>,
    bottle_type: Option<BottleKind>,
    units: Option<Units>,
    notes: Option<&str>,
) -> Result<()> {
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
        .log_bottle(cid, amount, kind.to_api(), units.to_api(), notes.as_deref())
        .await?;
    context.report(&format!(
        "Recorded {} {} of {}.",
        tidy(amount, units),
        units.as_str(),
        kind.to_api()
    ));
    Ok(())
}

/// Which units this bottle is being recorded in.
///
/// Asked every time, because the answer is about this bottle and not about the
/// family: the `units` setting is what Enter takes, and
/// `hb config set units oz` is how somebody changes what Enter takes.
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

/// Anything worth writing down, which every entry can carry.
fn ask_for_notes(context: &Context) -> Result<Option<String>> {
    let question = Question::new("notes", "Anything to note?", "--notes <TEXT>").optional();
    Ok(prompt::ask_optional(&question, context.theme)?
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty()))
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
    let millilitres = last? * recorded_in.map_or(1.0, |units| units.to_millilitres(1.0));
    Some(wanted.to_api().from_millilitres(millilitres))
}

/// An amount as that unit is read: millilitres whole, ounces to two places
/// with nothing trailing.
#[must_use]
pub fn tidy(amount: f64, units: Units) -> String {
    match units {
        Units::Ml => format!("{amount:.0}"),
        Units::Oz => {
            let text = format!("{amount:.2}");
            text.trim_end_matches('0').trim_end_matches('.').to_owned()
        }
    }
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
    .map(|amount| tidy(amount, units));

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

/// Runs the nursing timer.
async fn nursing(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    action: &NursingAction,
) -> Result<()> {
    match action {
        NursingAction::Start { side } => {
            let side = match side {
                Some(given) => given.to_api(),
                None => suggested_side(context, client, cid).await?,
            };
            client.start_nursing(cid, side.clone()).await?;
            context.report(&format!("Nursing started on the {side}."));
            Ok(())
        }
        NursingAction::Pause => {
            report(
                context,
                client.pause_nursing(cid).await?,
                "Paused.",
                "paused",
            );
            Ok(())
        }
        NursingAction::Resume { side } => {
            report(
                context,
                client.resume_nursing(cid, side.map(Side::to_api)).await?,
                "Resumed.",
                "running",
            );
            Ok(())
        }
        NursingAction::Switch => {
            report(
                context,
                client.switch_nursing_side(cid).await?,
                "Switched sides.",
                "running",
            );
            Ok(())
        }
        NursingAction::Cancel => {
            report(
                context,
                client.cancel_nursing(cid).await?,
                "Thrown away. Nothing was recorded.",
                "not running",
            );
            Ok(())
        }
        NursingAction::Stop => stop_nursing(context, client, cid).await,
        NursingAction::Status => nursing_status(context, client, cid).await,
    }
}

/// Which side to offer: the one opposite the last feed, which is what the app
/// suggests and what a parent half asleep wants pre-filled.
async fn suggested_side(context: &Context, client: &Huckleberry, cid: &str) -> Result<FeedSide> {
    const CHOICES: [Choice<'static>; 2] = [
        Choice {
            value: "left",
            hint: "The left side",
        },
        Choice {
            value: "right",
            hint: "The right side",
        },
    ];

    let last = client
        .feed_document(cid)
        .await
        .ok()
        .flatten()
        .and_then(|document| document.prefs)
        .and_then(|prefs| prefs.last_side)
        .map(|side| side.last_side);

    // The app suggests the side opposite the last feed, and so does this.
    let suggested = if last == Some(FeedSide::Left) {
        "right"
    } else {
        "left"
    };
    let question = Question::new("side", "Which side?", "--side <SIDE>")
        .with_choices(&CHOICES)
        .with_default(suggested);
    let answer = prompt::ask(&question, context.theme)?;
    Ok(if answer == "right" {
        FeedSide::Right
    } else {
        FeedSide::Left
    })
}

async fn stop_nursing(context: &Context, client: &Huckleberry, cid: &str) -> Result<()> {
    match client.complete_nursing(cid).await? {
        Some(completed) => {
            context.report(&format!(
                "Nursed {} · L {} · R {}",
                format_duration(completed.total_seconds()),
                format_duration(completed.left_seconds),
                format_duration(completed.right_seconds)
            ));
            println!("total_seconds\t{:.0}", completed.total_seconds());
            println!("left_seconds\t{:.0}", completed.left_seconds);
            println!("right_seconds\t{:.0}", completed.right_seconds);
        }
        None => context.warn("No nursing session was running."),
    }
    Ok(())
}

async fn nursing_status(context: &Context, client: &Huckleberry, cid: &str) -> Result<()> {
    let document = client.feed_document(cid).await?;
    let at = now_seconds();
    let calendar = context.calendar()?;

    let Some(timer) = document
        .as_ref()
        .and_then(huckleberry_api::models::FeedDocument::running_timer)
    else {
        println!("running\tfalse");
        context.detail("no nursing session in progress");
        return Ok(());
    };

    let (left, right) = timer.totals(at);
    println!("running\ttrue");
    println!("paused\t{}", timer.paused);
    println!("side\t{}", timer.current_side());
    println!("left_seconds\t{left:.0}");
    println!("right_seconds\t{right:.0}");
    if let Some(started) = timer.feed_start_time {
        println!(
            "started_clock\t{}",
            format::clock(started.as_f64(), &calendar)
        );
    }
    let paused = if timer.paused { " (paused)" } else { "" };
    context.report(&format!(
        "Nursing {} on the {}{paused} · L {} · R {}",
        format_duration(left + right),
        timer.current_side(),
        format_duration(left),
        format_duration(right)
    ));
    Ok(())
}

/// Records a meal, asking for everything that was not given.
async fn solids(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    foods: &[String],
    amount: Option<&str>,
    reaction: Option<Reaction>,
    notes: Option<&str>,
) -> Result<()> {
    let named = if foods.is_empty() {
        vec![ask_for_food(context, client, cid).await?]
    } else {
        foods.to_vec()
    };
    let amount = match amount {
        Some(given) => given.to_owned(),
        None => ask_for_helping(context)?,
    };
    let reaction = match reaction {
        Some(given) => Some(given),
        None => ask_for_reaction(context)?,
    };
    let notes = match notes {
        Some(given) => Some(given.to_owned()),
        None => ask_for_notes(context)?,
    };

    let known = client.custom_foods(cid, false).await.unwrap_or_default();
    let references: Vec<FoodReference> = named
        .iter()
        .map(|name| match_food(name, &known, &amount))
        .collect();

    client
        .log_solids(
            cid,
            &references,
            notes.as_deref(),
            reaction.map(Reaction::to_api),
            None,
        )
        .await?;
    context.report(&format!("Recorded {}.", named.join(", ")));
    Ok(())
}

/// How much of it, in whatever words suit. "some" is what the app writes when
/// nobody says, so it is what Enter takes.
fn ask_for_helping(context: &Context) -> Result<String> {
    let question = Question::new("amount", "How much?", "--amount <TEXT>").with_default("some");
    prompt::ask(&question, context.theme)
}

/// How it went, which is optional: a meal nobody had an opinion about is a
/// meal, and an invented reaction is worse than none.
fn ask_for_reaction(context: &Context) -> Result<Option<Reaction>> {
    const CHOICES: [Choice<'static>; 4] = [
        Choice {
            value: "loved",
            hint: "Loved it",
        },
        Choice {
            value: "meh",
            hint: "Took it or left it",
        },
        Choice {
            value: "hated",
            hint: "Hated it",
        },
        Choice {
            value: "allergic",
            hint: "Reacted badly",
        },
    ];
    let question = Question::new("reaction", "How did it go?", "--reaction <REACTION>")
        .with_choices(&CHOICES)
        .optional();
    Ok(
        prompt::ask_optional(&question, context.theme)?.map(|answer| match answer.as_str() {
            "meh" => Reaction::Meh,
            "hated" => Reaction::Hated,
            "allergic" => Reaction::Allergic,
            _ => Reaction::Loved,
        }),
    )
}

/// Offers the family's own foods, and takes a new name as well.
async fn ask_for_food(context: &Context, client: &Huckleberry, cid: &str) -> Result<String> {
    let known = client.custom_foods(cid, false).await.unwrap_or_default();
    if known.is_empty() {
        return prompt::ask(
            &Question::new("food", "What did they eat?", "--food <NAME>"),
            context.theme,
        );
    }
    let choices: Vec<Choice<'_>> = known
        .iter()
        .map(|food| Choice {
            value: &food.name,
            hint: "one of your foods",
        })
        .collect();
    // Free text is still accepted: a food not on the list is a food, and the
    // prompt takes anything typed that is not one of the numbered choices.
    let question = Question::new("food", "What did they eat?", "--food <NAME>");
    let answer = prompt::ask(&question.with_choices(&choices), context.theme);
    match answer {
        Ok(chosen) => Ok(chosen),
        Err(failure) => Err(failure),
    }
}

/// Turns a typed name into a food reference, matching the family's own foods
/// by name so that logging "Avocado" twice does not create two foods.
#[must_use]
pub fn match_food(
    name: &str,
    known: &[huckleberry_api::models::solids::CustomFood],
    amount: &str,
) -> FoodReference {
    let wanted = name.trim();
    known
        .iter()
        .find(|food| food.name.eq_ignore_ascii_case(wanted))
        .map_or_else(
            // A food nobody has added yet is referenced by its own name as the
            // id. The entry still reads correctly in the app; it is simply not
            // linked to a row in the family's food list.
            || FoodReference::custom(wanted, wanted, amount),
            |food| FoodReference::custom(&food.id, &food.name, amount),
        )
}

/// Turns a timer change into the line a person reads.
fn report(context: &Context, change: TimerChange, done: &str, already: &str) {
    match change {
        TimerChange::Applied => context.report(done),
        TimerChange::NotRunning => context.warn("No nursing session is running."),
        TimerChange::Unchanged => context.warn(&format!("The session is already {already}.")),
    }
}

#[cfg(test)]
mod amounts {
    use super::*;
    use huckleberry_api::models::feed::VolumeUnits;

    #[test]
    fn the_last_amount_is_offered_in_the_units_being_recorded_in() {
        // The bug this exists for: the app stores the last amount in whatever
        // units it was recorded in, so a family who records in ounces was
        // offered "1.15" under a question reading "How much, in ml?".
        let offered =
            offered_amount(Some(1.15), Some(&VolumeUnits::Ounces), Units::Ml).expect("an amount");
        assert!((offered - 34.0).abs() < 0.5, "{offered}");
    }

    #[test]
    fn an_amount_already_in_the_right_units_is_offered_as_it_stands() {
        let offered = offered_amount(Some(90.0), Some(&VolumeUnits::Millilitres), Units::Ml)
            .expect("an amount");
        assert!((offered - 90.0).abs() < f64::EPSILON);
    }

    #[test]
    fn units_nobody_recorded_are_taken_to_be_the_ones_being_recorded_in() {
        let offered = offered_amount(Some(90.0), None, Units::Ml).expect("an amount");
        assert!((offered - 90.0).abs() < f64::EPSILON);
    }

    #[test]
    fn nothing_recorded_means_nothing_to_offer() {
        assert_eq!(
            offered_amount(None, Some(&VolumeUnits::Ounces), Units::Ml),
            None
        );
    }

    #[test]
    fn an_offered_amount_is_written_the_way_that_unit_is_read() {
        assert_eq!(tidy(33.999, Units::Ml), "34");
        assert_eq!(tidy(1.15, Units::Oz), "1.15");
        assert_eq!(tidy(4.0, Units::Oz), "4", "no trailing zeros to delete");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use huckleberry_api::models::solids::{CustomFood, SolidsFoodSource};

    fn avocado() -> CustomFood {
        CustomFood {
            created_at: "2025-09-01T00:00:00.000Z".to_owned(),
            updated_at: "2025-09-01T00:00:00.000Z".to_owned(),
            name: "Avocado".to_owned(),
            archived: false,
            id: "food-1".to_owned(),
            kind: "solids".to_owned(),
            image: String::new(),
            source: SolidsFoodSource::Custom,
        }
    }

    #[test]
    fn a_known_food_is_referenced_by_its_id_so_it_is_not_duplicated() {
        let reference = match_food("avocado", &[avocado()], "some");
        assert_eq!(reference.id, "food-1");
        assert_eq!(reference.name, "Avocado", "the stored spelling wins");
    }

    #[test]
    fn a_food_nobody_has_added_is_still_recorded() {
        let reference = match_food("Pear", &[avocado()], "half");
        assert_eq!(reference.id, "Pear");
        assert_eq!(reference.name, "Pear");
    }

    #[test]
    fn a_name_is_trimmed_before_it_is_matched() {
        assert_eq!(match_food("  Avocado  ", &[avocado()], "some").id, "food-1");
    }
}
