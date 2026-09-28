//! The subcommand trees that go deeper than one level.

use clap::Subcommand;

use super::values::{Amount, BottleKind, Colour, Consistency, NappyKind, Reaction, Side, Units};

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
    /// Start a sleep.
    Start,
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

        /// What was in it. Defaults to the `units` setting's usual partner:
        /// left out on a terminal, you are asked.
        #[arg(short = 't', long = "type", value_enum, value_name = "KIND")]
        bottle_type: Option<BottleKind>,

        /// Which units the amount is in. Defaults to the `units` setting.
        #[arg(short, long, value_enum)]
        units: Option<Units>,
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

        /// How much of each, in whatever words suit.
        #[arg(short, long, default_value = "some", value_name = "TEXT")]
        amount: String,

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
    /// Whether the flags describe anything beyond the mode, which is what
    /// decides whether a person is offered the detail questions at all.
    #[must_use]
    pub const fn is_bare(&self) -> bool {
        self.pee.is_none()
            && self.poo.is_none()
            && self.color.is_none()
            && self.consistency.is_none()
            && !self.rash
    }

    /// Whether the mode implies there is anything dirty to describe.
    #[must_use]
    pub const fn describes_dirt(mode: NappyKind) -> bool {
        matches!(mode, NappyKind::Poo | NappyKind::Both)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_flags_are_recognised_as_bare() {
        let bare = NappyFlags {
            pee: None,
            poo: None,
            color: None,
            consistency: None,
            rash: false,
        };
        assert!(bare.is_bare());
        assert!(!NappyFlags { rash: true, ..bare }.is_bare());
        assert!(
            !NappyFlags {
                color: Some(Colour::Yellow),
                ..bare
            }
            .is_bare()
        );
    }

    #[test]
    fn only_a_dirty_nappy_has_a_colour_worth_asking_about() {
        assert!(NappyFlags::describes_dirt(NappyKind::Poo));
        assert!(NappyFlags::describes_dirt(NappyKind::Both));
        assert!(!NappyFlags::describes_dirt(NappyKind::Pee));
        assert!(!NappyFlags::describes_dirt(NappyKind::Dry));
    }
}
