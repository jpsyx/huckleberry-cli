//! The subcommand trees that go deeper than one level.

use clap::Subcommand;

use super::values::{
    Amount, BottleKind, Colour, Consistency, DiaperKind, Overlap, Reaction, Side, Units,
};

/// What `auth` can be asked to do.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum AuthAction {
    /// Sign in and save the session.
    Login {
        /// The account's email address. Left out on a terminal, you are asked.
        #[arg(short, long, value_name = "ADDRESS")]
        email: Option<String>,

        /// The account's password. Left out on a terminal, you are asked
        /// without an echo. Prefer HUCKLEBERRY_PASSWORD, or the prompt, over
        /// putting a password in your shell history.
        #[arg(short, long, value_name = "PASSWORD")]
        password: Option<String>,

        /// The family's IANA timezone, e.g. America/New_York.
        #[arg(short, long, value_name = "ZONE")]
        timezone: Option<String>,
    },
    /// Say whether there is a session and how long it has left.
    Status,
    /// Forget the saved session and credentials.
    Logout,
}

/// What `child` can be asked to do.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum ChildAction {
    /// List the children on the account.
    List,
    /// Choose the child every command acts on.
    Use {
        /// Which child. Left out on a terminal, you are asked.
        #[arg(value_name = "CID")]
        cid: Option<String>,
    },
    /// Show one child's profile.
    Show,
}

/// What `sleep` can be asked to do.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum SleepAction {
    /// Start a sleep, asking when it began on a terminal.
    Start {
        /// When it began: `now`, `358 am`, or `10 minutes ago`.
        /// Left out on a terminal, you are asked; otherwise defaults to now.
        #[arg(short, long, value_name = "TIME")]
        start: Option<String>,
    },
    /// Record a sleep that has already happened.
    Manual {
        /// When it began: `now`, `358 am`, or `32 mins ago`.
        /// Left out on a terminal, you are asked.
        #[arg(short, long, value_name = "TIME")]
        start: Option<String>,

        /// When it ended: `now`, `358 am`, or `32 mins ago`.
        /// Clock times mean the first occurrence after it began, so a sleep
        /// across midnight needs no date. Left out on a terminal, you are asked.
        #[arg(short, long, value_name = "TIME")]
        end: Option<String>,

        /// What to do if it runs into a sleep that is still going. Left out
        /// on a terminal, you are asked; there is no default, because all
        /// three answers throw something away.
        #[arg(long, value_enum)]
        overlap: Option<Overlap>,
    },
    /// Pause the running sleep.
    Pause {
        /// When it happened: `now`, `358 am`, or `32 mins ago`.
        /// Left out on a terminal, you are asked; otherwise defaults to now.
        #[arg(long, value_name = "TIME")]
        at: Option<String>,
    },
    /// Resume a paused sleep.
    Resume {
        /// When it happened: `now`, `358 am`, or `32 mins ago`.
        /// Left out on a terminal, you are asked; otherwise defaults to now.
        #[arg(long, value_name = "TIME")]
        at: Option<String>,
    },
    /// Finish the running sleep and record it, asking when it ended on a terminal.
    #[command(visible_alias = "end")]
    Stop {
        /// When it happened: `now`, `358 am`, or `32 mins ago`.
        /// Left out on a terminal, you are asked; otherwise defaults to now.
        #[arg(long, value_name = "TIME")]
        at: Option<String>,
    },
    /// Throw away the running sleep without recording it.
    Cancel,
    /// Say whether a sleep is running, and for how long.
    Status,
}

/// What `feed` can be asked to do.
#[derive(Debug, Clone, PartialEq, Subcommand)]
pub enum FeedAction {
    /// Record a bottle.
    Bottle {
        /// When it happened: `now`, `358 am`, or `32 mins ago`.
        /// Left out on a terminal, you are asked; otherwise defaults to now.
        #[arg(long, value_name = "TIME")]
        at: Option<String>,
        /// How much. Left out on a terminal, you are asked.
        #[arg(short, long, value_name = "NUMBER")]
        amount: Option<f64>,

        /// What was in it. Left out on a terminal, you are asked, with the
        /// last kind used offered as the default.
        #[arg(short = 't', long = "type", value_enum, value_name = "KIND")]
        bottle_type: Option<BottleKind>,

        /// Which units the amount is in. Left out on a terminal, you are
        /// asked, with the `units` setting offered as the default.
        #[arg(short, long, value_enum)]
        units: Option<Units>,

        /// Anything worth writing down.
        #[arg(short, long, value_name = "TEXT")]
        notes: Option<String>,
    },
    /// Run the nursing timer.
    Nursing {
        /// What to do with the timer.
        #[command(subcommand)]
        action: NursingAction,
    },
    /// Record a meal.
    Solids {
        /// When it happened: `now`, `358 am`, or `32 mins ago`.
        /// Left out on a terminal, you are asked; otherwise defaults to now.
        #[arg(long, value_name = "TIME")]
        at: Option<String>,
        /// A food, by name. Repeat for several. Left out on a terminal, you
        /// are asked, with the family's own foods offered as choices.
        #[arg(short, long = "food", value_name = "NAME")]
        foods: Vec<String>,

        /// How much of each, in whatever words suit. Left out on a terminal,
        /// you are asked, with "some" offered as the default.
        #[arg(short, long, value_name = "TEXT")]
        amount: Option<String>,

        /// How it went.
        #[arg(short, long, value_enum)]
        reaction: Option<Reaction>,

        /// Anything worth writing down.
        #[arg(short, long, value_name = "TEXT")]
        notes: Option<String>,
    },
}

/// What `feed nursing` can be asked to do.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum NursingAction {
    /// Start a nursing session.
    Start {
        /// When it happened: `now`, `358 am`, or `32 mins ago`.
        /// Left out on a terminal, you are asked; otherwise defaults to now.
        #[arg(long, alias = "at", value_name = "TIME")]
        start: Option<String>,
        /// Which side to start on. Left out, the side opposite the last feed
        /// is offered; on a terminal you are asked.
        #[arg(short, long, value_enum)]
        side: Option<Side>,
    },
    /// Pause the running session, banking the side that was running.
    Pause {
        /// When it happened: `now`, `358 am`, or `32 mins ago`.
        /// Left out on a terminal, you are asked; otherwise defaults to now.
        #[arg(long, value_name = "TIME")]
        at: Option<String>,
    },
    /// Resume a paused session.
    Resume {
        /// When it happened: `now`, `358 am`, or `32 mins ago`.
        /// Left out on a terminal, you are asked; otherwise defaults to now.
        #[arg(long, value_name = "TIME")]
        at: Option<String>,
        /// Which side to resume on. Left out, the last one is used.
        #[arg(short, long, value_enum)]
        side: Option<Side>,
    },
    /// Switch sides, banking what the current one has run for.
    Switch {
        /// When it happened: `now`, `358 am`, or `32 mins ago`.
        /// Left out on a terminal, you are asked; otherwise defaults to now.
        #[arg(long, value_name = "TIME")]
        at: Option<String>,
    },
    /// Finish the session and record it.
    Stop {
        /// When it happened: `now`, `358 am`, or `32 mins ago`.
        /// Left out on a terminal, you are asked; otherwise defaults to now.
        #[arg(long, value_name = "TIME")]
        at: Option<String>,
    },
    /// Throw away the session without recording it.
    Cancel,
    /// Say whether a session is running, and for how long on each side.
    Status,
}

/// What `foods` can be asked to do.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum FoodsAction {
    /// List foods.
    List {
        /// Only the family's own foods.
        #[arg(long, conflicts_with = "curated")]
        custom: bool,

        /// Only Huckleberry's curated list.
        #[arg(long, conflicts_with = "custom")]
        curated: bool,

        /// Only foods whose name contains this.
        #[arg(short, long, value_name = "TEXT")]
        search: Option<String>,

        /// Include foods that have been archived.
        #[arg(long)]
        archived: bool,
    },
    /// Add a food to the family's own list.
    Add {
        /// What to call it. Left out on a terminal, you are asked.
        #[arg(value_name = "NAME")]
        name: Option<String>,
    },
}

/// What `config` can be asked to do.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum ConfigAction {
    /// Print the effective configuration, one `key=value` line per setting.
    Show,
    /// Set one setting and save it.
    Set {
        /// Which setting to change. Left out on a terminal, you are asked.
        key: Option<String>,

        /// The new value. Left out on a terminal, you are asked.
        value: Option<String>,
    },
    /// Print where the configuration and credentials files live.
    Path,
}

/// Everything optional about a diaper or a potty trip, as the flags give it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiaperFlags {
    /// How much wet.
    pub pee: Option<Amount>,
    /// How much dirty.
    pub poo: Option<Amount>,
    /// The colour.
    pub color: Option<Colour>,
    /// The consistency.
    pub consistency: Option<Consistency>,
    /// Whether a rash was noted.
    pub rash: bool,
}

impl DiaperFlags {
    /// Whether the mode implies there is anything dirty to describe.
    #[must_use]
    pub const fn describes_dirt(mode: DiaperKind) -> bool {
        matches!(mode, DiaperKind::Poo | DiaperKind::Both)
    }
}

/// One thing a diaper can carry beyond what was in it.
///
/// A list rather than a series of `if`s in the command, so that "which
/// questions does a wet diaper have" is a pure function with a test on it
/// rather than the shape of a function that also talks to a terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiaperQuestion {
    /// How much wet.
    PeeAmount,
    /// How much dirty.
    PooAmount,
    /// The colour.
    Colour,
    /// The consistency.
    Consistency,
    /// Whether a rash was noted.
    Rash,
    /// Whatever the parent wants to write down.
    Notes,
}

/// Everything about a diaper of this mode that the flags left unanswered, in
/// the order a person should be asked.
///
/// Every question a mode implies is asked, and a flag answers only its own:
/// passing `--pee big` is not a statement that there is nothing to say about
/// the colour. Each answer is optional, so the fast path is Enter.
#[must_use]
pub fn unanswered(
    mode: DiaperKind,
    flags: &DiaperFlags,
    notes: Option<&str>,
) -> Vec<DiaperQuestion> {
    let mut asking = Vec::new();
    if matches!(mode, DiaperKind::Pee | DiaperKind::Both) && flags.pee.is_none() {
        asking.push(DiaperQuestion::PeeAmount);
    }
    if DiaperFlags::describes_dirt(mode) {
        if flags.poo.is_none() {
            asking.push(DiaperQuestion::PooAmount);
        }
        if flags.color.is_none() {
            asking.push(DiaperQuestion::Colour);
        }
        if flags.consistency.is_none() {
            asking.push(DiaperQuestion::Consistency);
        }
    }
    if !flags.rash {
        asking.push(DiaperQuestion::Rash);
    }
    if notes.is_none() {
        asking.push(DiaperQuestion::Notes);
    }
    asking
}

#[cfg(test)]
mod tests {
    use super::*;

    const BARE: DiaperFlags = DiaperFlags {
        pee: None,
        poo: None,
        color: None,
        consistency: None,
        rash: false,
    };

    #[test]
    fn only_a_dirty_diaper_has_a_colour_worth_asking_about() {
        assert!(DiaperFlags::describes_dirt(DiaperKind::Poo));
        assert!(DiaperFlags::describes_dirt(DiaperKind::Both));
        assert!(!DiaperFlags::describes_dirt(DiaperKind::Pee));
        assert!(!DiaperFlags::describes_dirt(DiaperKind::Dry));
    }

    #[test]
    fn a_wet_diaper_is_asked_how_much_wet() {
        // The bug this test exists for: picking "pee" used to end the
        // conversation, so a big wet diaper could not be recorded at all.
        let asked = unanswered(DiaperKind::Pee, &BARE, None);
        assert_eq!(
            asked,
            vec![
                DiaperQuestion::PeeAmount,
                DiaperQuestion::Rash,
                DiaperQuestion::Notes
            ]
        );
    }

    #[test]
    fn a_dirty_diaper_is_asked_about_the_dirt_and_nothing_about_wet() {
        let asked = unanswered(DiaperKind::Poo, &BARE, None);
        assert!(asked.contains(&DiaperQuestion::PooAmount));
        assert!(asked.contains(&DiaperQuestion::Colour));
        assert!(asked.contains(&DiaperQuestion::Consistency));
        assert!(!asked.contains(&DiaperQuestion::PeeAmount));
    }

    #[test]
    fn a_diaper_with_both_in_it_is_asked_about_both() {
        let asked = unanswered(DiaperKind::Both, &BARE, None);
        assert!(asked.contains(&DiaperQuestion::PeeAmount));
        assert!(asked.contains(&DiaperQuestion::PooAmount));
    }

    #[test]
    fn a_dry_diaper_is_asked_nothing_about_what_was_not_in_it() {
        let asked = unanswered(DiaperKind::Dry, &BARE, None);
        assert_eq!(asked, vec![DiaperQuestion::Rash, DiaperQuestion::Notes]);
    }

    #[test]
    fn a_flag_answers_its_own_question_and_no_others() {
        let asked = unanswered(
            DiaperKind::Both,
            &DiaperFlags {
                pee: Some(Amount::Big),
                ..BARE
            },
            None,
        );
        assert!(!asked.contains(&DiaperQuestion::PeeAmount));
        assert!(
            asked.contains(&DiaperQuestion::Colour),
            "one flag is not a statement about the rest: {asked:?}"
        );
    }

    #[test]
    fn a_diaper_described_entirely_in_flags_is_asked_nothing() {
        let asked = unanswered(
            DiaperKind::Both,
            &DiaperFlags {
                pee: Some(Amount::Big),
                poo: Some(Amount::Medium),
                color: Some(Colour::Yellow),
                consistency: Some(Consistency::Loose),
                rash: true,
            },
            Some("a bit sore"),
        );
        assert!(asked.is_empty(), "{asked:?}");
    }
}
