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
        } => bottle(context, &client, &cid, *amount, *bottle_type, *units).await,
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
                amount,
                *reaction,
                notes.as_deref(),
            )
            .await
        }
    };
    super::persist_session(context, &client).await?;
    outcome
}

/// Records a bottle, asking for whatever was not given.
async fn bottle(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    amount: Option<f64>,
    bottle_type: Option<BottleKind>,
    units: Option<Units>,
) -> Result<()> {
    let units = units.unwrap_or_else(|| Units::from_setting(&context.config.units));
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

    client
        .log_bottle(cid, amount, kind.to_api(), units.to_api())
        .await?;
    context.report(&format!(
        "Recorded {amount} {} of {}.",
        units.as_str(),
        kind.to_api()
    ));
    Ok(())
}

/// Offers the last bottle's amount as the default, which is almost always the
/// right answer and saves the typing that matters at 3am.
async fn ask_for_amount(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    units: Units,
) -> Result<f64> {
    let last = client
        .feed_document(cid)
        .await
        .ok()
        .flatten()
        .and_then(|document| document.prefs)
        .and_then(|prefs| prefs.bottle_amount)
        .map(|amount| format!("{}", amount.as_f64()));

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

/// Records a meal.
async fn solids(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    foods: &[String],
    amount: &str,
    reaction: Option<Reaction>,
    notes: Option<&str>,
) -> Result<()> {
    let named = if foods.is_empty() {
        vec![ask_for_food(context, client, cid).await?]
    } else {
        foods.to_vec()
    };

    let known = client.custom_foods(cid, false).await.unwrap_or_default();
    let references: Vec<FoodReference> = named
        .iter()
        .map(|name| match_food(name, &known, amount))
        .collect();

    client
        .log_solids(
            cid,
            &references,
            notes,
            reaction.map(Reaction::to_api),
            None,
        )
        .await?;
    context.report(&format!("Recorded {}.", named.join(", ")));
    Ok(())
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
