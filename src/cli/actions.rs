//! The subcommand trees that go deeper than one level.

use clap::Subcommand;

use super::values::{
    Amount, BottleKind, Colour, Consistency, NappyKind, Overlap, Reaction, Side, Units,
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
        /// When it began, as you would say it: `9pm`, `21:00`, `0357`.
        /// Left out on a terminal, you are asked.
        #[arg(short, long, value_name = "TIME")]
        start: Option<String>,

        /// When it ended, read as the first such time after it began, so a
        /// sleep across midnight needs no date. Left out on a terminal, you
        /// are asked.
        #[arg(short, long, value_name = "TIME")]
        end: Option<String>,

        /// What to do if it runs into a sleep that is still going. Left out
        /// on a terminal, you are asked; there is no default, because all
        /// three answers throw something away.
        #[arg(long, value_enum)]
        overlap: Option<Overlap>,
    },
    /// Pause the running sleep.
    Pause,
    /// Resume a paused sleep.
    Resume,
    /// Finish the running sleep and record it.
    Stop,
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
        /// Which side to start on. Left out, the side opposite the last feed
        /// is offered; on a terminal you are asked.
        #[arg(short, long, value_enum)]
        side: Option<Side>,
    },
    /// Pause the running session, banking the side that was running.
    Pause,
    /// Resume a paused session.
    Resume {
        /// Which side to resume on. Left out, the last one is used.
        #[arg(short, long, value_enum)]
        side: Option<Side>,
    },
    /// Switch sides, banking what the current one has run for.
    Switch,
    /// Finish the session and record it.
    Stop,
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

/// Everything optional about a nappy or a potty trip, as the flags give it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NappyFlags {
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

impl NappyFlags {
    /// Whether the mode implies there is anything dirty to describe.
    #[must_use]
    pub const fn describes_dirt(mode: NappyKind) -> bool {
        matches!(mode, NappyKind::Poo | NappyKind::Both)
    }
}

/// One thing a nappy can carry beyond what was in it.
///
/// A list rather than a series of `if`s in the command, so that "which
/// questions does a wet nappy have" is a pure function with a test on it
/// rather than the shape of a function that also talks to a terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NappyDetail {
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

/// Everything about a nappy of this mode that the flags left unanswered, in
/// the order a person should be asked.
///
/// Every question a mode implies is asked, and a flag answers only its own:
/// passing `--pee big` is not a statement that there is nothing to say about
/// the colour. Each answer is optional, so the fast path is Enter.
#[must_use]
pub fn unanswered(mode: NappyKind, flags: &NappyFlags, notes: Option<&str>) -> Vec<NappyDetail> {
    let mut asking = Vec::new();
    if matches!(mode, NappyKind::Pee | NappyKind::Both) && flags.pee.is_none() {
        asking.push(NappyDetail::PeeAmount);
    }
    if NappyFlags::describes_dirt(mode) {
        if flags.poo.is_none() {
            asking.push(NappyDetail::PooAmount);
        }
        if flags.color.is_none() {
            asking.push(NappyDetail::Colour);
        }
        if flags.consistency.is_none() {
            asking.push(NappyDetail::Consistency);
        }
    }
    if !flags.rash {
        asking.push(NappyDetail::Rash);
    }
    if notes.is_none() {
        asking.push(NappyDetail::Notes);
    }
    asking
}

#[cfg(test)]
mod tests {
    use super::*;

    const BARE: NappyFlags = NappyFlags {
        pee: None,
        poo: None,
        color: None,
        consistency: None,
        rash: false,
    };

    #[test]
    fn only_a_dirty_nappy_has_a_colour_worth_asking_about() {
        assert!(NappyFlags::describes_dirt(NappyKind::Poo));
        assert!(NappyFlags::describes_dirt(NappyKind::Both));
        assert!(!NappyFlags::describes_dirt(NappyKind::Pee));
        assert!(!NappyFlags::describes_dirt(NappyKind::Dry));
    }

    #[test]
    fn a_wet_nappy_is_asked_how_much_wet() {
        // The bug this test exists for: picking "pee" used to end the
        // conversation, so a big wet nappy could not be recorded at all.
        let asked = unanswered(NappyKind::Pee, &BARE, None);
        assert_eq!(
            asked,
            vec![
                NappyDetail::PeeAmount,
                NappyDetail::Rash,
                NappyDetail::Notes
            ]
        );
    }

    #[test]
    fn a_dirty_nappy_is_asked_about_the_dirt_and_nothing_about_wet() {
        let asked = unanswered(NappyKind::Poo, &BARE, None);
        assert!(asked.contains(&NappyDetail::PooAmount));
        assert!(asked.contains(&NappyDetail::Colour));
        assert!(asked.contains(&NappyDetail::Consistency));
        assert!(!asked.contains(&NappyDetail::PeeAmount));
    }

    #[test]
    fn a_nappy_with_both_in_it_is_asked_about_both() {
        let asked = unanswered(NappyKind::Both, &BARE, None);
        assert!(asked.contains(&NappyDetail::PeeAmount));
        assert!(asked.contains(&NappyDetail::PooAmount));
    }

    #[test]
    fn a_dry_nappy_is_asked_nothing_about_what_was_not_in_it() {
        let asked = unanswered(NappyKind::Dry, &BARE, None);
        assert_eq!(asked, vec![NappyDetail::Rash, NappyDetail::Notes]);
    }

    #[test]
    fn a_flag_answers_its_own_question_and_no_others() {
        let asked = unanswered(
            NappyKind::Both,
            &NappyFlags {
                pee: Some(Amount::Big),
                ..BARE
            },
            None,
        );
        assert!(!asked.contains(&NappyDetail::PeeAmount));
        assert!(
            asked.contains(&NappyDetail::Colour),
            "one flag is not a statement about the rest: {asked:?}"
        );
    }

    #[test]
    fn a_nappy_described_entirely_in_flags_is_asked_nothing() {
        let asked = unanswered(
            NappyKind::Both,
            &NappyFlags {
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
