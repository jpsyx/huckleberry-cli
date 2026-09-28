//! `diaper` and `potty`: the nappy tracker.
//!
//! Both write to the same collection, so they share this module and most of
//! their prompting. The interactive path is deliberately short: at 3am the
//! question is what was in it, and everything else is optional and only asked
//! for when the answer implies there is something to describe.

use anyhow::Result;
use huckleberry_api::DiaperDetails;

use crate::cli::actions::NappyFlags;
use crate::cli::{Amount, Colour, Consistency, NappyKind, PottyOutcome};
use crate::prompt::{self, Choice, Question};
use crate::session::Context;

/// Records a nappy change.
pub async fn nappy(
    context: &Context,
    mode: Option<NappyKind>,
    flags: NappyFlags,
    notes: Option<&str>,
) -> Result<()> {
    let (client, cid) = super::client_and_child(context).await?;
    let mode = match mode {
        Some(given) => given,
        None => ask_for_mode(context, "What was in it?")?,
    };
    let details = details(context, mode, flags, notes)?;

    client.log_diaper(&cid, mode.to_api(), &details).await?;
    super::persist_session(context, &client).await?;
    context.report(&format!("Recorded a {} nappy.", mode.to_api()));
    Ok(())
}

/// Records a potty trip.
pub async fn potty(
    context: &Context,
    mode: Option<NappyKind>,
    how: Option<PottyOutcome>,
    color: Option<Colour>,
    consistency: Option<Consistency>,
    notes: Option<&str>,
) -> Result<()> {
    let (client, cid) = super::client_and_child(context).await?;
    let mode = match mode {
        Some(given) => given,
        None => ask_for_mode(context, "What happened?")?,
    };
    let how = match how {
        Some(given) => given,
        None => ask_for_outcome(context)?,
    };
    let details = DiaperDetails {
        pee_amount: None,
        poo_amount: None,
        color: color.map(Colour::to_api),
        consistency: consistency.map(Consistency::to_api),
        rash: false,
        notes: notes.map(ToOwned::to_owned),
    };

    client
        .log_potty(&cid, mode.to_api(), how.to_api(), &details)
        .await?;
    super::persist_session(context, &client).await?;
    context.report(&format!("Recorded a potty trip: {}.", how.to_api()));
    Ok(())
}

/// Everything beyond the mode, asked for only when it is worth asking.
///
/// A person who passed any detail flag has said what they want recorded and is
/// not asked for the rest: the flags are the fast path, and interrupting them
/// with questions would make the fast path slower than the slow one.
fn details(
    context: &Context,
    mode: NappyKind,
    flags: NappyFlags,
    notes: Option<&str>,
) -> Result<DiaperDetails> {
    let mut details = DiaperDetails {
        pee_amount: flags.pee.map(Amount::to_api),
        poo_amount: flags.poo.map(Amount::to_api),
        color: flags.color.map(Colour::to_api),
        consistency: flags.consistency.map(Consistency::to_api),
        rash: flags.rash,
        notes: notes.map(ToOwned::to_owned),
    };

    let worth_asking = flags.is_bare()
        && notes.is_none()
        && NappyFlags::describes_dirt(mode)
        && std::io::IsTerminal::is_terminal(&std::io::stdin());
    if !worth_asking {
        return Ok(details);
    }
    // Only for a dirty nappy, only when nothing was said, and only ever
    // optional: an empty answer moves on.
    if prompt::confirm("Note the colour and consistency?", false, context.theme)? {
        details.color = Some(ask_for_colour(context)?.to_api());
        details.consistency = Some(ask_for_consistency(context)?.to_api());
    }
    Ok(details)
}

fn ask_for_mode(context: &Context, label: &str) -> Result<NappyKind> {
    const CHOICES: [Choice<'static>; 4] = [
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
    let question = Question::new("what was in it", label, "--mode <MODE>").with_choices(&CHOICES);
    Ok(match prompt::ask(&question, context.theme)?.as_str() {
        "poo" => NappyKind::Poo,
        "both" => NappyKind::Both,
        "dry" => NappyKind::Dry,
        _ => NappyKind::Pee,
    })
}

fn ask_for_outcome(context: &Context) -> Result<PottyOutcome> {
    const CHOICES: [Choice<'static>; 3] = [
        Choice {
            value: "went-potty",
            hint: "Went",
        },
        Choice {
            value: "sat-but-dry",
            hint: "Sat, but nothing happened",
        },
        Choice {
            value: "accident",
            hint: "Did not make it",
        },
    ];
    let question =
        Question::new("how it went", "How did it go?", "--how <OUTCOME>").with_choices(&CHOICES);
    Ok(match prompt::ask(&question, context.theme)?.as_str() {
        "sat-but-dry" => PottyOutcome::SatButDry,
        "accident" => PottyOutcome::Accident,
        _ => PottyOutcome::WentPotty,
    })
}

fn ask_for_colour(context: &Context) -> Result<Colour> {
    const CHOICES: [Choice<'static>; 6] = [
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
    let question = Question::new("colour", "What colour?", "--color <COLOUR>")
        .with_choices(&CHOICES)
        .with_default("yellow");
    Ok(match prompt::ask(&question, context.theme)?.as_str() {
        "brown" => Colour::Brown,
        "green" => Colour::Green,
        "black" => Colour::Black,
        "red" => Colour::Red,
        "gray" => Colour::Gray,
        _ => Colour::Yellow,
    })
}

fn ask_for_consistency(context: &Context) -> Result<Consistency> {
    const CHOICES: [Choice<'static>; 7] = [
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
    let question = Question::new(
        "consistency",
        "What consistency?",
        "--consistency <TEXTURE>",
    )
    .with_choices(&CHOICES)
    .with_default("loose");
    Ok(match prompt::ask(&question, context.theme)?.as_str() {
        "runny" => Consistency::Runny,
        "solid" => Consistency::Solid,
        "mucousy" => Consistency::Mucousy,
        "hard" => Consistency::Hard,
        "pebbles" => Consistency::Pebbles,
        "diarrhea" => Consistency::Diarrhea,
        _ => Consistency::Loose,
    })
}
