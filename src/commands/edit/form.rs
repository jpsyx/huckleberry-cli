//! Asking about every field of an entry, with what is already there.
//!
//! The submission process again, one difference: every question arrives with
//! the recorded answer as its default, so Enter keeps what is there and only
//! what is typed changes. A field that can be empty takes `-` for "leave it
//! out", which is the only way to say "there is no colour after all" without a
//! flag per field.
//!
//! With no terminal nothing here is reachable: the failure names
//! `--set key=value`, which is what would have answered.

use anyhow::Result;
use huckleberry_api::Huckleberry;

use crate::cli::{Amount, BottleKind, Colour, Consistency, DiaperKind, PottyOutcome, Reaction};
use crate::edit::{BottleDraft, DiaperDraft, Draft, NursingDraft, SleepDraft, SolidsDraft};
use crate::prompt::{self, Choice, Question};
use crate::render::format;
use crate::session::Context;

/// What clears a field that can be empty.
const CLEAR: &str = "-";

/// Asks about every field of the entry, in the order the tracker's own form
/// asks them.
pub async fn fill(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    draft: &mut Draft,
) -> Result<()> {
    match draft {
        Draft::Diaper(diaper) => diaper_form(context, diaper),
        Draft::Bottle(bottle) => bottle_form(context, bottle),
        Draft::Nursing(nursing) => nursing_form(context, nursing),
        Draft::Solids(meal) => solids_form(context, client, cid, meal).await,
        Draft::Sleep(sleep) => sleep_form(context, sleep),
    }
}

const MODES: [Choice<'static>; 4] = [
    Choice {
        value: "pee",
        hint: "Wet only",
    },
    Choice {
        value: "poo",
        hint: "Dirty only",
    },
    Choice {
        value: "both",
        hint: "Both",
    },
    Choice {
        value: "dry",
        hint: "Neither",
    },
];

const OUTCOMES: [Choice<'static>; 3] = [
    Choice {
        value: "wentPotty",
        hint: "Went",
    },
    Choice {
        value: "satButDry",
        hint: "Sat, but nothing happened",
    },
    Choice {
        value: "accident",
        hint: "Did not make it",
    },
];

const AMOUNTS: [Choice<'static>; 3] = [
    Choice {
        value: "little",
        hint: "A little",
    },
    Choice {
        value: "medium",
        hint: "A medium amount",
    },
    Choice {
        value: "big",
        hint: "A lot",
    },
];

const COLOURS: [Choice<'static>; 6] = [
    Choice {
        value: "yellow",
        hint: "Yellow",
    },
    Choice {
        value: "brown",
        hint: "Brown",
    },
    Choice {
        value: "green",
        hint: "Green",
    },
    Choice {
        value: "black",
        hint: "Black",
    },
    Choice {
        value: "red",
        hint: "Red",
    },
    Choice {
        value: "gray",
        hint: "Grey",
    },
];

const CONSISTENCIES: [Choice<'static>; 7] = [
    Choice {
        value: "loose",
        hint: "Loose",
    },
    Choice {
        value: "runny",
        hint: "Runny",
    },
    Choice {
        value: "solid",
        hint: "Solid",
    },
    Choice {
        value: "mucousy",
        hint: "Mucousy",
    },
    Choice {
        value: "hard",
        hint: "Hard",
    },
    Choice {
        value: "pebbles",
        hint: "Pebbles",
    },
    Choice {
        value: "diarrhea",
        hint: "Diarrhea",
    },
];

const BOTTLES: [Choice<'static>; 4] = [
    Choice {
        value: "Formula",
        hint: "Formula",
    },
    Choice {
        value: "Breast Milk",
        hint: "Expressed breast milk",
    },
    Choice {
        value: "Cow Milk",
        hint: "Cow milk",
    },
    Choice {
        value: "Other",
        hint: "Something else",
    },
];

const UNITS: [Choice<'static>; 2] = [
    Choice {
        value: "ml",
        hint: "Millilitres",
    },
    Choice {
        value: "oz",
        hint: "Fluid ounces",
    },
];

const REACTIONS: [Choice<'static>; 4] = [
    Choice {
        value: "LOVED",
        hint: "Loved it",
    },
    Choice {
        value: "MEH",
        hint: "Took it or left it",
    },
    Choice {
        value: "HATED",
        hint: "Hated it",
    },
    Choice {
        value: "ALLERGIC",
        hint: "Reacted badly",
    },
];

fn diaper_form(context: &Context, diaper: &mut DiaperDraft) -> Result<()> {
    diaper.mode = required(
        context,
        "what was in it",
        "What was in it?",
        "--set mode=...",
        &MODES,
        diaper.mode.to_api().as_str(),
        DiaperKind::from_stored,
    )?;
    if diaper.potty {
        let current = diaper
            .how
            .map(|outcome| outcome.to_api().as_str().to_owned());
        diaper.how = Some(required(
            context,
            "how it went",
            "How did it go?",
            "--set how=...",
            &OUTCOMES,
            current.as_deref().unwrap_or("wentPotty"),
            PottyOutcome::from_stored,
        )?);
    }
    if !diaper.potty {
        if matches!(diaper.mode, DiaperKind::Pee | DiaperKind::Both) {
            diaper.pee = optional(
                context,
                "amount",
                "How much wet?",
                "--set pee=...",
                &AMOUNTS,
                diaper
                    .pee
                    .map(|amount| amount.to_api().as_str().to_owned())
                    .as_deref(),
                Amount::from_stored,
            )?;
        } else {
            diaper.pee = None;
        }
    }
    if matches!(diaper.mode, DiaperKind::Poo | DiaperKind::Both) {
        if !diaper.potty {
            diaper.poo = optional(
                context,
                "amount",
                "How much dirty?",
                "--set poo=...",
                &AMOUNTS,
                diaper
                    .poo
                    .map(|amount| amount.to_api().as_str().to_owned())
                    .as_deref(),
                Amount::from_stored,
            )?;
        }
        diaper.color = optional(
            context,
            "colour",
            "What colour?",
            "--set color=...",
            &COLOURS,
            diaper
                .color
                .map(|shade| shade.to_api().as_str().to_owned())
                .as_deref(),
            Colour::from_stored,
        )?;
        diaper.consistency = optional(
            context,
            "consistency",
            "What consistency?",
            "--set consistency=...",
            &CONSISTENCIES,
            diaper
                .consistency
                .map(|texture| texture.to_api().as_str().to_owned())
                .as_deref(),
            Consistency::from_stored,
        )?;
    } else {
        // What is not in it is not described: a diaper changed from dirty to
        // wet keeps no colour.
        diaper.poo = None;
        diaper.color = None;
        diaper.consistency = None;
    }
    if !diaper.potty {
        diaper.rash = prompt::confirm("Any rash?", diaper.rash, context.theme)?;
    }
    diaper.notes = notes(context, diaper.notes.as_deref())?;
    Ok(())
}

fn bottle_form(context: &Context, bottle: &mut BottleDraft) -> Result<()> {
    let chosen = required(
        context,
        "units",
        "In which units?",
        "--set units=...",
        &UNITS,
        bottle.units.to_api().as_str(),
        crate::cli::Units::from_stored,
    )?;
    // Asking in ounces and then offering the millilitres is how somebody is
    // shown "[30]" under "How much, in oz?".
    if chosen != bottle.units {
        bottle.amount = format::convert(bottle.amount, bottle.units, chosen);
        bottle.units = chosen;
    }
    let label = format!("How much, in {}?", bottle.units.as_str());
    bottle.amount = number(
        context,
        "amount",
        &label,
        "--set amount=...",
        &format::amount_in(bottle.amount, bottle.units),
    )?;
    bottle.kind = required(
        context,
        "bottle type",
        "What was in it?",
        "--set type=...",
        &BOTTLES,
        bottle.kind.to_api().as_str(),
        BottleKind::from_stored,
    )?;
    bottle.notes = notes(context, bottle.notes.as_deref())?;
    Ok(())
}

fn nursing_form(context: &Context, nursing: &mut NursingDraft) -> Result<()> {
    nursing.left_minutes = number(
        context,
        "minutes on the left",
        "How many minutes on the left?",
        "--set left=...",
        &format!("{:.0}", nursing.left_minutes),
    )?;
    nursing.right_minutes = number(
        context,
        "minutes on the right",
        "How many minutes on the right?",
        "--set right=...",
        &format!("{:.0}", nursing.right_minutes),
    )?;
    nursing.notes = notes(context, nursing.notes.as_deref())?;
    Ok(())
}

async fn solids_form(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    meal: &mut SolidsDraft,
) -> Result<()> {
    let known = client.custom_foods(cid, false).await.unwrap_or_default();
    let choices: Vec<Choice<'_>> = known
        .iter()
        .map(|food| Choice {
            value: &food.name,
            hint: "one of your foods",
        })
        .collect();
    let current = meal.foods.join(", ");
    let question = Question::new(
        "food",
        "What did they eat? Separate several with commas.",
        "--set foods=...",
    )
    .with_default(&current);
    // The choices are offered but anything typed is taken: a food nobody has
    // added is still a food, and a comma-separated list is not on any list.
    let answer = prompt::ask(&question.with_choices(&choices), context.theme)?;
    let named: Vec<String> = answer
        .split(',')
        .map(|food| food.trim().to_owned())
        .filter(|food| !food.is_empty())
        .collect();
    if !named.is_empty() {
        meal.foods = named;
    }
    meal.reaction = optional(
        context,
        "reaction",
        "How did it go?",
        "--set reaction=...",
        &REACTIONS,
        meal.reaction
            .map(|taken| taken.to_api().as_str().to_owned())
            .as_deref(),
        Reaction::from_stored,
    )?;
    meal.notes = notes(context, meal.notes.as_deref())?;
    Ok(())
}

fn sleep_form(context: &Context, sleep: &mut SleepDraft) -> Result<()> {
    sleep.minutes = number(
        context,
        "a length in minutes",
        "How many minutes did it last?",
        "--set duration=...",
        &format!("{:.0}", sleep.minutes),
    )?;
    sleep.notes = notes(context, sleep.notes.as_deref())?;
    Ok(())
}

/// A field that has to hold something: Enter keeps what is there.
fn required<T>(
    context: &Context,
    subject: &str,
    label: &str,
    flag: &str,
    choices: &[Choice<'_>],
    current: &str,
    read: impl Fn(&str) -> Option<T>,
) -> Result<T> {
    let question = Question::new(subject, label, flag)
        .with_choices(choices)
        .with_default(current);
    let answer = prompt::ask(&question, context.theme)?;
    read(&answer).ok_or_else(|| anyhow::anyhow!("`{answer}` is not one of the choices"))
}

/// A field that can be empty: Enter keeps what is there, `-` takes it away.
fn optional<T>(
    context: &Context,
    subject: &str,
    label: &str,
    flag: &str,
    choices: &[Choice<'_>],
    current: Option<&str>,
    read: impl Fn(&str) -> Option<T>,
) -> Result<Option<T>> {
    let mut offered: Vec<Choice<'_>> = choices.to_vec();
    if current.is_some() {
        offered.push(Choice {
            value: CLEAR,
            hint: "Leave it out",
        });
    }
    let mut question = Question::new(subject, label, flag)
        .with_choices(&offered)
        .optional();
    if let Some(value) = current {
        question = question.with_default(value);
    }
    let Some(answer) = prompt::ask_optional(&question, context.theme)? else {
        return Ok(None);
    };
    if answer == CLEAR {
        return Ok(None);
    }
    read(&answer)
        .map(Some)
        .ok_or_else(|| anyhow::anyhow!("`{answer}` is not one of the choices"))
}

/// A number, with what is there now as the default.
fn number(context: &Context, subject: &str, label: &str, flag: &str, shown: &str) -> Result<f64> {
    let question = Question::new(subject, label, flag).with_default(shown);
    let answer = prompt::ask(&question, context.theme)?;
    answer
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("`{answer}` is not a number"))
}

/// The note, where `-` is how a note is taken off.
fn notes(context: &Context, current: Option<&str>) -> Result<Option<String>> {
    let label = if current.is_some() {
        format!("Anything to note? ({CLEAR} takes it off)")
    } else {
        "Anything to note?".to_owned()
    };
    let mut question = Question::new("notes", &label, "--set notes=...").optional();
    if let Some(text) = current {
        question = question.with_default(text);
    }
    Ok(prompt::ask_optional(&question, context.theme)?
        .filter(|text| text != CLEAR)
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty()))
}
