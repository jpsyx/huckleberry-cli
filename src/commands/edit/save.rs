//! Saving history details while retaining unmodified wire values.
use crate::{
    cli::{Amount, Colour, Consistency, PottyOutcome},
    edit::Draft,
};
use anyhow::Result;
use huckleberry_api::models::{
    diaper::{DiaperMode, PooColor, PooConsistency},
    feed::BottleType,
    solids::SolidsReaction,
};
use huckleberry_api::{DiaperDetails, Huckleberry, RowRef};
/// Writes the draft back to the row it came from.
pub(super) async fn run(
    client: &Huckleberry,
    cid: &str,
    at: &RowRef,
    draft: &Draft,
    original: &super::preserve::Original,
) -> Result<()> {
    match draft {
        Draft::Diaper(diaper) => {
            let details = details(diaper, original);
            client
                .update_diaper_entry(
                    cid,
                    at,
                    DiaperMode::from_wire(
                        &original
                            .value("mode", diaper.mode.to_api().as_str())
                            .unwrap_or_else(|| diaper.mode.to_api().as_str().into()),
                    ),
                    &details,
                    diaper.how.map(PottyOutcome::to_api),
                )
                .await?;
        }
        Draft::Bottle(bottle) => {
            client
                .update_bottle_entry(
                    cid,
                    at,
                    bottle.amount,
                    BottleType::from_wire(
                        &original
                            .value("type", bottle.kind.to_api().as_str())
                            .unwrap_or_else(|| bottle.kind.to_api().as_str().into()),
                    ),
                    bottle.units.to_api(),
                    bottle.notes.as_deref(),
                )
                .await?;
        }
        Draft::Nursing(nursing) => {
            client
                .update_nursing_entry(
                    cid,
                    at,
                    nursing.left_minutes * 60.0,
                    nursing.right_minutes * 60.0,
                    nursing.notes.as_deref(),
                )
                .await?;
        }
        Draft::Solids(meal) => {
            // The family's own foods, so that correcting a meal does not turn
            // a food that is on their list into one that is not.
            let known = client.custom_foods(cid, false).await.unwrap_or_default();
            let references: Vec<_> = meal
                .foods
                .iter()
                .map(|food| crate::commands::feed::match_food(food, &known, &meal.amount))
                .collect();
            client
                .update_solids_entry(
                    cid,
                    at,
                    &references,
                    original
                        .value(
                            "reaction",
                            meal.reaction
                                .map(crate::cli::Reaction::to_api)
                                .as_ref()
                                .map_or("", |value| value.as_str()),
                        )
                        .as_deref()
                        .map(SolidsReaction::from_wire),
                    meal.notes.as_deref(),
                )
                .await?;
        }
        Draft::Sleep(sleep) => {
            client
                .update_sleep_entry(cid, at, sleep.minutes * 60.0, sleep.notes.as_deref())
                .await?;
        }
    }
    Ok(())
}

fn details(
    diaper: &crate::edit::DiaperDraft,
    original: &super::preserve::Original,
) -> DiaperDetails {
    DiaperDetails {
        pee_amount: diaper.pee.map(Amount::to_api),
        poo_amount: diaper.poo.map(Amount::to_api),
        color: original
            .value(
                "color",
                diaper
                    .color
                    .map(Colour::to_api)
                    .as_ref()
                    .map_or("", |value| value.as_str()),
            )
            .as_deref()
            .map(PooColor::from_wire),
        consistency: original
            .value(
                "consistency",
                diaper
                    .consistency
                    .map(Consistency::to_api)
                    .as_ref()
                    .map_or("", |value| value.as_str()),
            )
            .as_deref()
            .map(PooConsistency::from_wire),
        rash: diaper.rash,
        notes: diaper.notes.clone(),
    }
}
