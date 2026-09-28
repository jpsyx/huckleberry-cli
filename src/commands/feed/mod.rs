//! `feed`: bottles, the nursing timer, and meals.

use anyhow::{Result, bail};
use huckleberry_api::client::now_seconds;
use huckleberry_api::models::feed::FeedSide;
use huckleberry_api::models::solids::FoodReference;
use huckleberry_api::{Huckleberry, TimerChange};

use crate::cli::{BottleKind, FeedAction, NursingAction, Reaction, Side, Units};
use crate::prompt::{self, Choice, Question};
use crate::render::{format, output};
use crate::session::Context;

/// Unanswered bottle fields, including its event time.
struct BottleInput<'a> {
    amount: Option<f64>,
    bottle_type: Option<BottleKind>,
    units: Option<Units>,
    notes: Option<&'a str>,
    at: Option<&'a str>,
}

/// Unanswered meal fields, including its event time.
struct MealInput<'a> {
    foods: &'a [String],
    amount: Option<&'a str>,
    reaction: Option<Reaction>,
    notes: Option<&'a str>,
    at: Option<&'a str>,
}

/// Runs the chosen action.
pub async fn run(context: &Context, action: &FeedAction) -> Result<()> {
    let (client, cid) = super::client_and_child(context).await?;
    let outcome = match action {
        FeedAction::Bottle {
            at,
            amount,
            bottle_type,
            units,
            notes,
        } => {
            bottle::bottle(
                context,
                &client,
                &cid,
                BottleInput {
                    amount: *amount,
                    bottle_type: *bottle_type,
                    units: *units,
                    notes: notes.as_deref(),
                    at: at.as_deref(),
                },
            )
            .await
        }
        FeedAction::Nursing { action } => nursing::nursing(context, &client, &cid, action).await,
        FeedAction::Solids {
            at,
            foods,
            amount,
            reaction,
            notes,
        } => {
            solids::solids(
                context,
                &client,
                &cid,
                MealInput {
                    foods,
                    amount: amount.as_deref(),
                    reaction: *reaction,
                    notes: notes.as_deref(),
                    at: at.as_deref(),
                },
            )
            .await
        }
    };
    super::persist_session(context, &client).await?;
    outcome
}

mod bottle;
mod nursing;
pub mod solids;
pub use bottle::offered_amount;
pub use solids::match_food;
/// Anything worth writing down, which every entry can carry.
fn ask_for_notes(context: &Context) -> Result<Option<String>> {
    let question = Question::new("notes", "Anything to note?", "--notes <TEXT>").optional();
    Ok(prompt::ask_optional(&question, context.theme)?
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty()))
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
