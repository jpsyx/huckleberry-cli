//! What an edit is: which row, which fields, and what is in them now.
//!
//! Everything here is pure. A [`Draft`] is one entry as it stands, in this
//! tool's own words: the form in [`crate::commands::edit`] fills it in by
//! asking, `--set key=value` fills it in without asking, and both hand the
//! same value to the same write. That is what keeps the two paths honest with
//! each other, and it is why "what does `--set mode=both` do to a nappy" is a
//! test rather than something you find out against somebody's real record.
//!
//! An edit changes what was recorded, never when it happened. A row's id in
//! Huckleberry leads with its own millisecond timestamp, so moving the moment
//! would leave history ordered by a time the row no longer claims.

use anyhow::{Result, bail};
use huckleberry_api::RowRef;

use crate::cli::{
    Amount, BottleKind, Colour, Consistency, NappyKind, PottyOutcome, Reaction, Units,
};
use crate::domain::types::{Dataset, DiaperEvent, FeedEvent, SleepEvent};

/// How a batched row's key is spelled inside a token.
const BATCH: char = '#';

/// The name one row goes by on the command line.
///
/// `diaper/1758572400000-3f2a` is a row of its own; a `#` names an entry
/// inside one of the packed batches. It is the whole address, so an agent that
/// listed the entries yesterday can still edit one today.
#[must_use]
pub fn token_for(at: &RowRef) -> String {
    at.batch_key.as_ref().map_or_else(
        || format!("{}/{}", at.tracker, at.document_id),
        |key| format!("{}/{}{BATCH}{key}", at.tracker, at.document_id),
    )
}

/// Reads a token back.
///
/// # Errors
///
/// When it is not one: a token nobody can act on is better refused here than
/// turned into a write at a path that means something else.
pub fn parse_token(text: &str) -> Result<RowRef> {
    let text = text.trim();
    let Some((tracker, rest)) = text.split_once('/') else {
        bail!("`{text}` is not an entry: they look like `diaper/1758572400000-3f2a`");
    };
    if tracker.is_empty() || rest.is_empty() {
        bail!("`{text}` is not an entry: they look like `diaper/1758572400000-3f2a`");
    }
    Ok(match rest.split_once(BATCH) {
        None => RowRef::loose(tracker, rest),
        Some((document_id, key)) => RowRef::batched(tracker, document_id, key),
    })
}

/// A nappy or a potty trip, as it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NappyDraft {
    /// Whether this is a potty trip rather than a nappy.
    pub potty: bool,
    /// What was in it.
    pub mode: NappyKind,
    /// How much wet.
    pub pee: Option<Amount>,
    /// How much dirty.
    pub poo: Option<Amount>,
    /// The colour.
    pub color: Option<Colour>,
    /// The consistency.
    pub consistency: Option<Consistency>,
    /// Whether a rash was noted. Nappies only.
    pub rash: bool,
    /// How the potty trip went. Potty trips only.
    pub how: Option<PottyOutcome>,
    /// Whatever the parent typed.
    pub notes: Option<String>,
}

/// A bottle, as it stands, in the units it is being shown in.
#[derive(Debug, Clone, PartialEq)]
pub struct BottleDraft {
    /// How much, in `units`.
    pub amount: f64,
    /// Which units the amount is in.
    pub units: Units,
    /// What was in it.
    pub kind: BottleKind,
    /// Whatever the parent typed.
    pub notes: Option<String>,
}

/// A nursing session, as it stands.
#[derive(Debug, Clone, PartialEq)]
pub struct NursingDraft {
    /// Minutes on the left.
    pub left_minutes: f64,
    /// Minutes on the right.
    pub right_minutes: f64,
    /// Whatever the parent typed.
    pub notes: Option<String>,
}

/// A meal, as it stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolidsDraft {
    /// What was eaten, by name.
    pub foods: Vec<String>,
    /// How much of each, in whatever words suit.
    pub amount: String,
    /// How it went.
    pub reaction: Option<Reaction>,
    /// Whatever the parent typed.
    pub notes: Option<String>,
}

/// A sleep, as it stands.
#[derive(Debug, Clone, PartialEq)]
pub struct SleepDraft {
    /// How long it lasted, in minutes.
    pub minutes: f64,
    /// Whatever the parent typed.
    pub notes: Option<String>,
}

/// One entry, in the words this tool asks about it with.
#[derive(Debug, Clone, PartialEq)]
pub enum Draft {
    /// A nappy or a potty trip.
    Nappy(NappyDraft),
    /// A bottle.
    Bottle(BottleDraft),
    /// A nursing session.
    Nursing(NursingDraft),
    /// A meal.
    Solids(SolidsDraft),
    /// A sleep.
    Sleep(SleepDraft),
}

impl Draft {
    /// What this entry is, as a sentence calls it.
    #[must_use]
    pub const fn what(&self) -> &'static str {
        match self {
            Self::Nappy(nappy) if nappy.potty => "potty trip",
            Self::Nappy(_) => "nappy",
            Self::Bottle(_) => "bottle",
            Self::Nursing(_) => "nursing session",
            Self::Solids(_) => "meal",
            Self::Sleep(_) => "sleep",
        }
    }

    /// The fields `--set` can name on this entry.
    #[must_use]
    pub const fn fields(&self) -> &'static [&'static str] {
        match self {
            Self::Nappy(nappy) if nappy.potty => &["mode", "how", "color", "consistency", "notes"],
            Self::Nappy(_) => &[
                "mode",
                "pee",
                "poo",
                "color",
                "consistency",
                "rash",
                "notes",
            ],
            Self::Bottle(_) => &["amount", "type", "units", "notes"],
            Self::Nursing(_) => &["left", "right", "notes"],
            Self::Solids(_) => &["foods", "amount", "reaction", "notes"],
            Self::Sleep(_) => &["duration", "notes"],
        }
    }

    /// Changes one field, as `--set key=value` names it.
    ///
    /// An empty value clears a field that can be empty, which is the only way
    /// to say "there is no colour after all" without a flag per field.
    ///
    /// # Errors
    ///
    /// When the field is not one this entry has, or the value is not one the
    /// field takes. Both name what would have worked.
    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        let key = key.trim().to_lowercase();
        let value = value.trim();
        let known = self.fields();
        if !known.contains(&key.as_str()) {
            bail!(
                "a {} has no `{key}`: it takes {}",
                self.what(),
                known.join(", ")
            );
        }
        match self {
            Self::Nappy(nappy) => set_on_nappy(nappy, &key, value),
            Self::Bottle(bottle) => set_on_bottle(bottle, &key, value),
            Self::Nursing(nursing) => set_on_nursing(nursing, &key, value),
            Self::Solids(meal) => set_on_solids(meal, &key, value),
            Self::Sleep(sleep) => set_on_sleep(sleep, &key, value),
        }
    }

    /// Applies every `key=value` pair in turn.
    ///
    /// # Errors
    ///
    /// As [`Draft::set`], and when a pair has no `=` in it.
    pub fn apply(&mut self, pairs: &[String]) -> Result<()> {
        for pair in pairs {
            let Some((key, value)) = pair.split_once('=') else {
                bail!("`{pair}` is not a change: they look like `--set color=yellow`");
            };
            self.set(key, value)?;
        }
        Ok(())
    }
}

/// What a draft says, in one line, for the sentence that confirms the change.
///
/// The words a person picked, not the words the app stores: somebody who
/// answered "big" should read "big" back.
#[must_use]
pub fn summary(draft: &Draft) -> String {
    match draft {
        Draft::Nappy(nappy) => {
            let mut said = vec![format!("{:?}", nappy.mode).to_lowercase()];
            if let Some(outcome) = nappy.how {
                said.push(spelling(&format!("{outcome:?}")));
            }
            for (label, amount) in [("wet", nappy.pee), ("dirty", nappy.poo)] {
                if let Some(amount) = amount {
                    said.push(format!("{} {label}", format!("{amount:?}").to_lowercase()));
                }
            }
            if let Some(colour) = nappy.color {
                said.push(format!("{colour:?}").to_lowercase());
            }
            if let Some(texture) = nappy.consistency {
                said.push(format!("{texture:?}").to_lowercase());
            }
            if nappy.rash {
                said.push("rash noted".to_owned());
            }
            said.join(" · ")
        }
        Draft::Bottle(bottle) => format!(
            "{:.0} {} of {}",
            bottle.amount,
            bottle.units.as_str(),
            bottle.kind.to_api()
        ),
        Draft::Nursing(nursing) => format!(
            "L {:.0}m · R {:.0}m",
            nursing.left_minutes, nursing.right_minutes
        ),
        Draft::Solids(meal) => {
            let mut said = meal.foods.join(", ");
            if let Some(reaction) = meal.reaction {
                use core::fmt::Write as _;
                let _ = write!(said, " · {}", format!("{reaction:?}").to_lowercase());
            }
            said
        }
        Draft::Sleep(sleep) => format!("{:.0} minutes", sleep.minutes),
    }
}

/// A variant name as a person spells it: `SatButDry` becomes `sat but dry`.
fn spelling(name: &str) -> String {
    let mut words = String::new();
    for (position, character) in name.char_indices() {
        if character.is_uppercase() && position > 0 {
            words.push(' ');
        }
        words.extend(character.to_lowercase());
    }
    words
}

/// The draft for the row at `at`, when the dataset holds it.
///
/// The one place that knows which of a dataset's collections a reference can
/// be in, and the reason an edit does not need a second read: whatever was
/// shown is what is being changed.
#[must_use]
pub fn draft_for(dataset: &Dataset, at: &RowRef, units: Units) -> Option<Draft> {
    if let Some(nappy) = dataset
        .diapers
        .iter()
        .find(|event| is_at(event.at.as_ref(), at))
    {
        return Some(Draft::Nappy(nappy_draft(nappy)));
    }
    if let Some(feed) = dataset.feeds.iter().find(|event| is_at(event.at(), at)) {
        return Some(feed_draft(feed, units));
    }
    dataset
        .sleep
        .iter()
        .find(|event| is_at(event.at.as_ref(), at))
        .map(|sleep| Draft::Sleep(sleep_draft(sleep)))
}

/// Whether an event's reference is the one being looked for.
fn is_at(candidate: Option<&RowRef>, wanted: &RowRef) -> bool {
    candidate == Some(wanted)
}

fn nappy_draft(nappy: &DiaperEvent) -> NappyDraft {
    NappyDraft {
        potty: nappy.potty,
        // A mode this tool has no word for is shown as what it counts as
        // rather than refusing the edit outright.
        mode: NappyKind::from_stored(&nappy.mode).unwrap_or(match (nappy.wet, nappy.dirty) {
            (true, true) => NappyKind::Both,
            (false, true) => NappyKind::Poo,
            (true, false) => NappyKind::Pee,
            (false, false) => NappyKind::Dry,
        }),
        pee: nappy.pee_size.map(Amount::from_size),
        poo: nappy.poo_size.map(Amount::from_size),
        color: nappy.color.as_deref().and_then(Colour::from_stored),
        consistency: nappy
            .consistency
            .as_deref()
            .and_then(Consistency::from_stored),
        rash: nappy.rash,
        how: None,
        notes: nappy.notes.clone(),
    }
}

fn feed_draft(feed: &FeedEvent, units: Units) -> Draft {
    match feed {
        FeedEvent::Bottle {
            amount_ml,
            bottle_type,
            notes,
            ..
        } => Draft::Bottle(BottleDraft {
            amount: units.to_api().from_millilitres(amount_ml.unwrap_or(0.0)),
            units,
            kind: bottle_type
                .as_deref()
                .and_then(BottleKind::from_stored)
                .unwrap_or(BottleKind::Formula),
            notes: notes.clone(),
        }),
        FeedEvent::Nursing {
            left_seconds,
            right_seconds,
            notes,
            ..
        } => Draft::Nursing(NursingDraft {
            left_minutes: left_seconds / 60.0,
            right_minutes: right_seconds / 60.0,
            notes: notes.clone(),
        }),
        FeedEvent::Solids {
            foods,
            reaction,
            notes,
            ..
        } => Draft::Solids(SolidsDraft {
            foods: foods.clone(),
            amount: "some".to_owned(),
            reaction: reaction.as_deref().and_then(Reaction::from_stored),
            notes: notes.clone(),
        }),
    }
}

fn sleep_draft(sleep: &SleepEvent) -> SleepDraft {
    SleepDraft {
        minutes: sleep.duration / 60.0,
        notes: sleep.notes.clone(),
    }
}

/// What a typed word means for a field that can also be empty.
fn optional<T>(value: &str, read: impl Fn(&str) -> Option<T>, field: &str) -> Result<Option<T>> {
    if value.is_empty() {
        return Ok(None);
    }
    match read(value) {
        Some(parsed) => Ok(Some(parsed)),
        None => bail!("`{value}` is not a {field} this tool knows"),
    }
}

/// A number, with the field named when it is not one.
fn number(value: &str, field: &str) -> Result<f64> {
    value
        .parse()
        .map_err(|_| anyhow::anyhow!("`{value}` is not a number, and {field} is one"))
}

/// A note as it is stored: what was typed, or nothing when it was blank.
fn note(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

fn set_on_nappy(nappy: &mut NappyDraft, key: &str, value: &str) -> Result<()> {
    match key {
        "mode" => {
            nappy.mode = NappyKind::from_stored(value)
                .ok_or_else(|| anyhow::anyhow!("`{value}` is not pee, poo, both or dry"))?;
        }
        "pee" => nappy.pee = optional(value, Amount::from_stored, "amount")?,
        "poo" => nappy.poo = optional(value, Amount::from_stored, "amount")?,
        "color" => nappy.color = optional(value, Colour::from_stored, "colour")?,
        "consistency" => {
            nappy.consistency = optional(value, Consistency::from_stored, "consistency")?;
        }
        "rash" => nappy.rash = truth(value)?,
        "how" => {
            nappy.how = optional(
                value,
                |word| PottyOutcome::from_stored(word).or_else(|| spoken_outcome(word)),
                "outcome",
            )?;
        }
        _ => nappy.notes = note(value),
    }
    Ok(())
}

/// The potty outcomes as a person spells them on a command line, where the
/// app's own `wentPotty` would be a surprise to type.
fn spoken_outcome(word: &str) -> Option<PottyOutcome> {
    match word {
        "went-potty" | "went" => Some(PottyOutcome::WentPotty),
        "sat-but-dry" | "sat" => Some(PottyOutcome::SatButDry),
        "accident" => Some(PottyOutcome::Accident),
        _ => None,
    }
}

fn set_on_bottle(bottle: &mut BottleDraft, key: &str, value: &str) -> Result<()> {
    match key {
        "amount" => bottle.amount = number(value, "an amount")?,
        "type" => {
            bottle.kind = BottleKind::from_stored(value)
                .or_else(|| spoken_bottle(value))
                .ok_or_else(|| anyhow::anyhow!("`{value}` is not a kind of bottle"))?;
        }
        "units" => {
            bottle.units = Units::from_stored(value)
                .ok_or_else(|| anyhow::anyhow!("`{value}` is not ml or oz"))?;
        }
        _ => bottle.notes = note(value),
    }
    Ok(())
}

/// The bottle kinds as a person spells them, where the app capitalises.
fn spoken_bottle(word: &str) -> Option<BottleKind> {
    use clap::ValueEnum as _;
    BottleKind::from_str(word, true).ok()
}

fn set_on_nursing(nursing: &mut NursingDraft, key: &str, value: &str) -> Result<()> {
    match key {
        "left" => nursing.left_minutes = number(value, "a number of minutes")?,
        "right" => nursing.right_minutes = number(value, "a number of minutes")?,
        _ => nursing.notes = note(value),
    }
    Ok(())
}

fn set_on_solids(meal: &mut SolidsDraft, key: &str, value: &str) -> Result<()> {
    match key {
        "foods" => {
            let named: Vec<String> = value
                .split(',')
                .map(|food| food.trim().to_owned())
                .filter(|food| !food.is_empty())
                .collect();
            if named.is_empty() {
                bail!("a meal needs at least one food");
            }
            meal.foods = named;
        }
        "amount" => value.clone_into(&mut meal.amount),
        "reaction" => {
            meal.reaction = optional(
                value,
                |word| {
                    use clap::ValueEnum as _;
                    Reaction::from_stored(word).or_else(|| Reaction::from_str(word, true).ok())
                },
                "reaction",
            )?;
        }
        _ => meal.notes = note(value),
    }
    Ok(())
}

fn set_on_sleep(sleep: &mut SleepDraft, key: &str, value: &str) -> Result<()> {
    match key {
        "duration" => sleep.minutes = number(value, "a number of minutes")?,
        _ => sleep.notes = note(value),
    }
    Ok(())
}

/// A yes or a no, for the fields that are one.
fn truth(value: &str) -> Result<bool> {
    match value.to_lowercase().as_str() {
        "true" | "yes" | "y" | "1" => Ok(true),
        "false" | "no" | "n" | "0" | "" => Ok(false),
        other => bail!("`{other}` is not a yes or a no"),
    }
}

#[cfg(test)]
mod tokens {
    use super::*;

    #[test]
    fn a_row_of_its_own_is_named_by_its_tracker_and_its_id() {
        let at = RowRef::loose("diaper", "1758572400000-3f2a");
        assert_eq!(token_for(&at), "diaper/1758572400000-3f2a");
        assert_eq!(parse_token("diaper/1758572400000-3f2a").expect("a row"), at);
    }

    #[test]
    fn a_row_inside_a_batch_carries_its_key_too() {
        let at = RowRef::batched("feed", "pack-one", "inner");
        assert_eq!(token_for(&at), "feed/pack-one#inner");
        assert_eq!(parse_token("feed/pack-one#inner").expect("a row"), at);
    }

    #[test]
    fn every_token_reads_back_as_the_row_it_names() {
        for at in [
            RowRef::loose("sleep", "abc"),
            RowRef::batched("health", "pack", "a-b_c"),
        ] {
            assert_eq!(parse_token(&token_for(&at)).expect("a row"), at);
        }
    }

    #[test]
    fn something_that_is_not_a_token_is_refused_with_an_example() {
        for text in ["", "diaper", "/abc", "diaper/"] {
            let failure = parse_token(text).expect_err("refused");
            assert!(
                format!("{failure}").contains("1758572400000"),
                "{text}: {failure}"
            );
        }
    }
}

#[cfg(test)]
mod drafts {
    use super::*;
    use crate::domain::fixtures::{AFTERNOON, dataset, nappy};
    use crate::domain::types::Size;

    fn wet_nappy() -> Dataset {
        let mut data = dataset();
        let mut event = nappy(AFTERNOON, true, false);
        event.at = Some(RowRef::loose("diaper", "row-one"));
        event.mode = "pee".to_owned();
        event.pee_size = Some(Size::Large);
        data.diapers = vec![event];
        data
    }

    #[test]
    fn a_nappy_starts_from_what_is_on_the_record() {
        let data = wet_nappy();
        let Some(Draft::Nappy(draft)) =
            draft_for(&data, &RowRef::loose("diaper", "row-one"), Units::Ml)
        else {
            panic!("a nappy");
        };
        assert_eq!(draft.mode, NappyKind::Pee);
        assert_eq!(draft.pee, Some(Amount::Big));
        assert!(!draft.potty);
    }

    #[test]
    fn a_row_the_dataset_does_not_hold_has_no_draft() {
        assert!(
            draft_for(
                &wet_nappy(),
                &RowRef::loose("diaper", "elsewhere"),
                Units::Ml
            )
            .is_none()
        );
    }

    #[test]
    fn a_change_names_the_field_and_the_value() {
        let mut draft = Draft::Nappy(NappyDraft {
            potty: false,
            mode: NappyKind::Pee,
            pee: None,
            poo: None,
            color: None,
            consistency: None,
            rash: false,
            how: None,
            notes: None,
        });
        draft
            .apply(&[
                "mode=both".to_owned(),
                "poo=medium".to_owned(),
                "color=yellow".to_owned(),
                "notes=a big one".to_owned(),
            ])
            .expect("the changes");
        let Draft::Nappy(nappy) = &draft else {
            panic!("a nappy")
        };
        assert_eq!(nappy.mode, NappyKind::Both);
        assert_eq!(nappy.poo, Some(Amount::Medium));
        assert_eq!(nappy.color, Some(Colour::Yellow));
        assert_eq!(nappy.notes.as_deref(), Some("a big one"));
    }

    #[test]
    fn an_empty_value_clears_the_field_rather_than_leaving_it() {
        let mut draft = Draft::Nappy(NappyDraft {
            potty: false,
            mode: NappyKind::Poo,
            pee: None,
            poo: Some(Amount::Big),
            color: Some(Colour::Green),
            consistency: None,
            rash: true,
            how: None,
            notes: Some("sore".to_owned()),
        });
        draft
            .apply(&[
                "color=".to_owned(),
                "notes=".to_owned(),
                "rash=no".to_owned(),
            ])
            .expect("the changes");
        let Draft::Nappy(nappy) = &draft else {
            panic!("a nappy")
        };
        assert_eq!(nappy.color, None);
        assert_eq!(nappy.notes, None);
        assert!(!nappy.rash);
        assert_eq!(nappy.poo, Some(Amount::Big), "and nothing else moved");
    }

    #[test]
    fn a_field_this_entry_does_not_have_is_refused_with_the_ones_it_does() {
        let mut draft = Draft::Sleep(SleepDraft {
            minutes: 45.0,
            notes: None,
        });
        let failure = draft
            .set("color", "yellow")
            .expect_err("a sleep has no colour");
        let message = format!("{failure}");
        assert!(message.contains("duration"), "{message}");
        assert!(message.contains("notes"), "{message}");
    }

    #[test]
    fn a_value_the_field_does_not_take_names_what_would_have_worked() {
        let mut draft = Draft::Bottle(BottleDraft {
            amount: 90.0,
            units: Units::Ml,
            kind: BottleKind::Formula,
            notes: None,
        });
        assert!(draft.set("amount", "a lot").is_err());
        assert!(draft.set("units", "litres").is_err());
        draft.set("units", "oz").expect("a unit it does take");
    }

    #[test]
    fn a_pair_with_no_equals_in_it_is_refused() {
        let mut draft = Draft::Sleep(SleepDraft {
            minutes: 45.0,
            notes: None,
        });
        assert!(draft.apply(&["duration".to_owned()]).is_err());
    }

    #[test]
    fn the_sentence_that_confirms_a_change_is_in_the_words_that_were_picked() {
        let said = summary(&Draft::Nappy(NappyDraft {
            potty: false,
            mode: NappyKind::Both,
            pee: Some(Amount::Big),
            poo: Some(Amount::Little),
            color: Some(Colour::Yellow),
            consistency: None,
            rash: true,
            how: None,
            notes: None,
        }));
        assert!(said.contains("both"), "{said}");
        assert!(said.contains("big wet"), "{said}");
        assert!(said.contains("little dirty"), "{said}");
        assert!(said.contains("yellow"), "{said}");
        assert!(said.contains("rash noted"), "{said}");
    }

    #[test]
    fn a_potty_trips_outcome_is_spelled_as_a_person_says_it() {
        let said = summary(&Draft::Nappy(NappyDraft {
            potty: true,
            mode: NappyKind::Pee,
            pee: None,
            poo: None,
            color: None,
            consistency: None,
            rash: false,
            how: Some(PottyOutcome::SatButDry),
            notes: None,
        }));
        assert!(said.contains("sat but dry"), "{said}");
    }

    #[test]
    fn a_bottle_reads_back_in_the_units_it_was_shown_in() {
        let said = summary(&Draft::Bottle(BottleDraft {
            amount: 4.0,
            units: Units::Oz,
            kind: BottleKind::BreastMilk,
            notes: None,
        }));
        assert_eq!(said, "4 oz of Breast Milk");
    }

    #[test]
    fn a_potty_trip_is_asked_about_how_it_went_and_never_about_a_rash() {
        let draft = Draft::Nappy(NappyDraft {
            potty: true,
            mode: NappyKind::Pee,
            pee: None,
            poo: None,
            color: None,
            consistency: None,
            rash: false,
            how: Some(PottyOutcome::WentPotty),
            notes: None,
        });
        assert!(draft.fields().contains(&"how"));
        assert!(!draft.fields().contains(&"rash"));
        assert_eq!(draft.what(), "potty trip");
    }

    #[test]
    fn the_outcome_takes_the_spelling_a_person_types_as_well_as_the_apps() {
        let mut draft = Draft::Nappy(NappyDraft {
            potty: true,
            mode: NappyKind::Pee,
            pee: None,
            poo: None,
            color: None,
            consistency: None,
            rash: false,
            how: None,
            notes: None,
        });
        draft.set("how", "went-potty").expect("a person's spelling");
        let Draft::Nappy(potty) = &draft else {
            panic!("a potty trip")
        };
        assert_eq!(potty.how, Some(PottyOutcome::WentPotty));
        draft.set("how", "wentPotty").expect("the app's spelling");
    }
}
