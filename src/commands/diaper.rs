//! `diaper` and `potty`: the diaper tracker.
//!
//! Both write to the same collection, so they share this module and most of
//! their prompting. Every question the mode implies is asked, in order, and
//! every one of them takes Enter for "leave it out": a person who answered
//! "wet" is still asked how much, because a detail that is never asked for is
//! a detail that cannot be recorded. Which questions a mode implies is
//! [`crate::cli::actions::unanswered`], which is pure and tested.

use anyhow::Result;
use huckleberry_api::DiaperDetails;

use crate::cli::actions::{DiaperFlags, DiaperQuestion, unanswered};
use crate::cli::{Amount, Colour, Consistency, DiaperKind, PottyOutcome};
use crate::prompt::{self, Choice, Question};
use crate::render::output;
use crate::session::Context;

/// Records a diaper change.
pub async fn diaper(
    context: &Context,
    mode: Option<DiaperKind>,
    flags: DiaperFlags,
    notes: Option<&str>,
) -> Result<()> {
    let (client, cid) = super::client_and_child(context).await?;
    let mode = match mode {
        Some(given) => given,
        None => ask_for_mode(context, "What was in it?")?,
    };
    let details = details(context, Record::Diaper, mode, flags, notes)?;

    client.log_diaper(&cid, mode.to_api(), &details).await?;
    super::persist_session(context, &client).await?;
    context.receipt("🧷 Diaper recorded", &receipt_fields(mode, &details), &[]);
    Ok(())
}

/// Records a potty trip.
pub async fn potty(
    context: &Context,
    mode: Option<DiaperKind>,
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
    let details = details(
        context,
        Record::Potty,
        mode,
        DiaperFlags {
            pee: None,
            poo: None,
            color,
            consistency,
            rash: false,
        },
        notes,
    )?;

    client
        .log_potty(&cid, mode.to_api(), how.to_api(), &details)
        .await?;
    super::persist_session(context, &client).await?;
    let mut fields = receipt_fields(mode, &details);
    fields.push(("Outcome", output::words(how.to_api().as_str())));
    context.receipt("🚽 Potty trip recorded", &fields, &[]);
    Ok(())
}

/// The details a parent just recorded, using the words from the prompts.
fn receipt_fields(mode: DiaperKind, details: &DiaperDetails) -> output::Fields {
    let contents = match mode {
        DiaperKind::Pee => "Wet",
        DiaperKind::Poo => "Dirty",
        DiaperKind::Both => "Wet and dirty",
        DiaperKind::Dry => "Dry",
    };
    let mut fields = vec![("Contents", contents.into())];
    if let Some(value) = &details.pee_amount {
        fields.push(("Wet amount", output::words(&value.to_string())));
    }
    if let Some(value) = &details.poo_amount {
        fields.push(("Dirty amount", output::words(&value.to_string())));
    }
    if let Some(value) = &details.color {
        fields.push(("Colour", output::words(value.as_str())));
    }
    if let Some(value) = &details.consistency {
        fields.push(("Consistency", output::words(value.as_str())));
    }
    if details.rash {
        fields.push(("Rash", "Recorded".into()));
    }
    if let Some(notes) = &details.notes {
        fields.push(("Notes", notes.clone()));
    }
    fields
}

/// Everything beyond the mode, asked for one question at a time.
///
/// A flag answers its own question and no others, so `--pee big` still leaves
/// the colour worth asking about. Nothing here is required: an empty answer
/// leaves the field out, and with no terminal nothing is asked at all.
fn details(
    context: &Context,
    record: Record,
    mode: DiaperKind,
    flags: DiaperFlags,
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

    for detail in unanswered(mode, &flags, notes)
        .into_iter()
        .filter(|detail| record.asks(*detail))
    {
        match detail {
            DiaperQuestion::PeeAmount => {
                details.pee_amount =
                    ask_for_amount(context, "How much wet?", "--pee <AMOUNT>")?.map(Amount::to_api);
            }
            DiaperQuestion::PooAmount => {
                details.poo_amount = ask_for_amount(context, "How much dirty?", "--poo <AMOUNT>")?
                    .map(Amount::to_api);
            }
            DiaperQuestion::Colour => {
                details.color = ask_for_colour(context)?.map(Colour::to_api);
            }
            DiaperQuestion::Consistency => {
                details.consistency = ask_for_consistency(context)?.map(Consistency::to_api);
            }
            DiaperQuestion::Rash => {
                details.rash = prompt::confirm("Any rash?", false, context.theme)?;
            }
            DiaperQuestion::Notes => details.notes = ask_for_notes(context)?,
        }
    }
    Ok(details)
}

/// Which of the two records is being described.
///
/// The same questions, minus the ones the record has no field for: the app has
/// no rash toggle on a potty trip, so asking about one would offer to record
/// something Huckleberry will not show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Record {
    /// A diaper change.
    Diaper,
    /// A potty trip.
    Potty,
}

impl Record {
    /// Whether this record has a field for that detail.
    const fn asks(self, detail: DiaperQuestion) -> bool {
        !matches!(self, Self::Potty) || !matches!(detail, DiaperQuestion::Rash)
    }
}

fn ask_for_mode(context: &Context, label: &str) -> Result<DiaperKind> {
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
        "poo" => DiaperKind::Poo,
        "both" => DiaperKind::Both,
        "dry" => DiaperKind::Dry,
        _ => DiaperKind::Pee,
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

fn ask_for_colour(context: &Context) -> Result<Option<Colour>> {
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
        .optional();
    Ok(
        prompt::ask_optional(&question, context.theme)?.map(|answer| match answer.as_str() {
            "brown" => Colour::Brown,
            "green" => Colour::Green,
            "black" => Colour::Black,
            "red" => Colour::Red,
            "gray" => Colour::Gray,
            _ => Colour::Yellow,
        }),
    )
}

fn ask_for_consistency(context: &Context) -> Result<Option<Consistency>> {
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
    .optional();
    Ok(
        prompt::ask_optional(&question, context.theme)?.map(|answer| match answer.as_str() {
            "runny" => Consistency::Runny,
            "solid" => Consistency::Solid,
            "mucousy" => Consistency::Mucousy,
            "hard" => Consistency::Hard,
            "pebbles" => Consistency::Pebbles,
            "diarrhea" => Consistency::Diarrhea,
            _ => Consistency::Loose,
        }),
    )
}

/// How much, as the app's three buttons. Asked for the wet half and for the
/// dirty half, which is why the label and the flag are arguments.
fn ask_for_amount(context: &Context, label: &str, flag: &str) -> Result<Option<Amount>> {
    const CHOICES: [Choice<'static>; 3] = [
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
    let question = Question::new("amount", label, flag)
        .with_choices(&CHOICES)
        .optional();
    Ok(
        prompt::ask_optional(&question, context.theme)?.map(|answer| match answer.as_str() {
            "little" => Amount::Little,
            "big" => Amount::Big,
            _ => Amount::Medium,
        }),
    )
}

/// Anything worth writing down, which every entry can carry.
fn ask_for_notes(context: &Context) -> Result<Option<String>> {
    let question = Question::new("notes", "Anything to note?", "--notes <TEXT>").optional();
    Ok(prompt::ask_optional(&question, context.theme)?
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty()))
}
