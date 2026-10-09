//! The command-line surface.
//!
//! Every action has a flag or a subcommand, so an agent or a script can drive
//! the tool without ever meeting a prompt. Values a person may omit are
//! `Option`s: the command asks for them when there is a terminal to ask on,
//! and fails with the flag that would have answered when there is not. See
//! `docs/rules/cli-ux.md`.
//!
//! The surface is split in three: this file has the shape (the root flags and
//! the command list), [`values`] has the fixed sets of words the flags accept,
//! and [`actions`] has the subcommand trees that are more than one level deep.

pub mod actions;
pub mod values;

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

pub use actions::{
    AuthAction, ChildAction, ConfigAction, FeedAction, FoodsAction, NursingAction, SleepAction,
};
pub use values::{
    Amount, BottleKind, Colour, Consistency, DiaperKind, LogKind, Overlap, PottyOutcome, Reaction,
    Side, System, TrendMetric, Units,
};

/// A terminal client and dashboard for Huckleberry baby tracking.
#[derive(Debug, Clone, Parser)]
#[command(version, propagate_version = true)]
pub struct Cli {
    /// Print detailed diagnostics to stderr.
    #[arg(short, long, global = true, env = "HUCKLEBERRY_VERBOSE")]
    pub verbose: bool,

    /// Read and write this configuration file instead of the default one.
    /// The credentials file is read from the same directory.
    #[arg(long, global = true, env = "HUCKLEBERRY_CONFIG", value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Which child to act on. Overrides the `child` setting for this run.
    #[arg(long, global = true, env = "HUCKLEBERRY_CHILD", value_name = "CID")]
    pub child: Option<String>,

    /// Read this snapshot instead of Huckleberry. What `export` writes.
    #[arg(long, global = true, value_name = "PATH")]
    pub offline: Option<PathBuf>,

    /// The command to run.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Everything `edit` takes.
///
/// One struct rather than six arguments threaded through the dispatch, which
/// is what keeps the match that routes commands readable as a list.
#[derive(Debug, Clone, PartialEq, Args)]
pub struct EditOptions {
    /// Which entry, as `edit --list` names one. Left out on a terminal, you
    /// pick one off a list. `sleep/current` selects the ongoing sleep.
    #[arg(long, value_name = "ENTRY")]
    pub id: Option<String>,

    /// Change one field without being asked, for example `--set pee=big`.
    /// Every saved entry accepts `--set "at=8am"`; clock-only edits keep its date.
    /// For the ongoing sleep use `--set "start=32 mins ago"`.
    /// Repeat for several. An empty value clears the field. Left out on a
    /// terminal, choose which fields to change from a menu.
    #[arg(long = "set", value_name = "KEY=VALUE")]
    pub set: Vec<String>,

    /// Print one `entry<TAB>kind<TAB>when<TAB>what` line per entry and change
    /// nothing. How a script finds the entry it means.
    #[arg(long)]
    pub list: bool,

    /// How many days to look back.
    #[arg(short, long, value_name = "DAYS")]
    pub days: Option<u32>,

    /// At most this many entries.
    #[arg(short, long, default_value_t = 40, value_name = "COUNT")]
    pub limit: usize,
}

/// Everything `delete` takes.
#[derive(Debug, Clone, PartialEq, Args)]
pub struct DeleteOptions {
    /// Which entry, as `delete --list` names one. Left out on a terminal, you
    /// pick one off a list.
    #[arg(long, value_name = "ENTRY")]
    pub id: Option<String>,

    /// Work on one tracker's own rows rather than the merged stream:
    /// `health`, `pump`, `milestones`. How an entry the stream does not show
    /// is reached.
    #[arg(long, value_name = "NAME")]
    pub tracker: Option<String>,

    /// Print one `entry<TAB>when<TAB>what` line per entry and delete nothing.
    #[arg(long)]
    pub list: bool,

    /// Delete without asking. Required when there is no terminal.
    #[arg(short, long)]
    pub yes: bool,

    /// How many days to look back.
    #[arg(short, long, value_name = "DAYS")]
    pub days: Option<u32>,

    /// At most this many entries.
    #[arg(short, long, default_value_t = 40, value_name = "COUNT")]
    pub limit: usize,
}

/// The things this tool can be asked to do.
#[derive(Debug, Clone, PartialEq, Subcommand)]
pub enum Command {
    /// Sign in, check the session, or sign out.
    Auth {
        /// What to do with the session.
        #[command(subcommand)]
        action: AuthAction,
    },

    /// List the children on the account, or pick one.
    Child {
        /// What to do with the children.
        #[command(subcommand)]
        action: ChildAction,
    },

    /// The four facts that matter at 3am: last feed, last diaper, awake or
    /// asleep, and the longest stretch of the night.
    Now {
        /// Print JSON instead of a screen.
        #[arg(long)]
        json: bool,
    },

    /// A full-screen dashboard: the stripe chart, the day table, and the live
    /// timers. Press q to leave.
    Dash {
        /// How many days to chart.
        #[arg(short, long, value_name = "DAYS")]
        days: Option<u32>,

        /// How often to re-read, in seconds.
        #[arg(short, long, value_name = "SECONDS")]
        refresh: Option<u32>,
    },

    /// One row per day: feeds, milk, sleep and diapers.
    Summary {
        /// Complete family days to summarize, plus today (default: configured days, normally 7).
        #[arg(short, long, value_name = "DAYS")]
        days: Option<u32>,

        /// Print JSON instead of a table.
        #[arg(long)]
        json: bool,
    },

    /// A chart of one number over time.
    Trends {
        /// Which number to chart. Left out on a terminal, you are asked.
        #[arg(short, long, value_enum)]
        metric: Option<TrendMetric>,

        /// How many days to chart.
        #[arg(short, long, value_name = "DAYS")]
        days: Option<u32>,
    },

    /// The 24-hour stripe chart: where sleep actually lands, day by day.
    Stripes {
        /// How many days to chart.
        #[arg(short, long, value_name = "DAYS")]
        days: Option<u32>,
    },

    /// Everything that happened, newest first.
    Log {
        /// Only this kind of entry.
        #[arg(short, long, value_enum)]
        kind: Option<LogKind>,

        /// Only entries matching every word of this. On a terminal you can
        /// also press `/` and type.
        #[arg(short, long, value_name = "TEXT")]
        search: Option<String>,

        /// How many days to read.
        #[arg(short, long, value_name = "DAYS")]
        days: Option<u32>,

        /// At most this many entries.
        #[arg(short, long, default_value_t = 40, value_name = "COUNT")]
        limit: usize,
    },

    /// Change an entry that is already on the record.
    Edit {
        /// Which entry, and what to change about it.
        #[command(flatten)]
        options: EditOptions,
    },

    /// Take an entry off the record. This cannot be undone.
    Delete {
        /// Which entry, and whether to ask first.
        #[command(flatten)]
        options: DeleteOptions,
    },

    /// Start, pause, finish or check a sleep.
    Sleep {
        /// What to do with the sleep timer.
        #[command(subcommand)]
        action: SleepAction,
    },

    /// Record a bottle, run a nursing timer, or record a meal.
    Feed {
        /// What kind of feed.
        #[command(subcommand)]
        action: FeedAction,
    },

    /// Record a diaper change.
    Diaper {
        /// When it happened: `now`, `358 am`, or `32 mins ago`.
        /// Left out on a terminal, you are asked; otherwise defaults to now.
        #[arg(long, value_name = "TIME")]
        at: Option<String>,
        /// What was in it. Left out on a terminal, you are asked.
        #[arg(short, long, value_enum)]
        mode: Option<DiaperKind>,

        /// How much wet.
        #[arg(long, value_enum, value_name = "AMOUNT")]
        pee: Option<Amount>,

        /// How much dirty.
        #[arg(long, value_enum, value_name = "AMOUNT")]
        poo: Option<Amount>,

        /// The colour.
        #[arg(long, value_enum)]
        color: Option<Colour>,

        /// The consistency.
        #[arg(long, value_enum)]
        consistency: Option<Consistency>,

        /// Note a rash.
        #[arg(long)]
        rash: bool,

        /// Anything worth writing down.
        #[arg(short, long, value_name = "TEXT")]
        notes: Option<String>,
    },

    /// Record a potty trip.
    Potty {
        /// When it happened: `now`, `358 am`, or `32 mins ago`.
        /// Left out on a terminal, you are asked; otherwise defaults to now.
        #[arg(long, value_name = "TIME")]
        at: Option<String>,
        /// What happened. Left out on a terminal, you are asked.
        #[arg(short, long, value_enum)]
        mode: Option<DiaperKind>,

        /// How it went. Left out on a terminal, you are asked.
        #[arg(long, value_enum)]
        how: Option<PottyOutcome>,

        /// The colour.
        #[arg(long, value_enum)]
        color: Option<Colour>,

        /// The consistency.
        #[arg(long, value_enum)]
        consistency: Option<Consistency>,

        /// Anything worth writing down.
        #[arg(short, long, value_name = "TEXT")]
        notes: Option<String>,
    },

    /// Record a growth measurement.
    Growth {
        /// When it happened: `now`, `358 am`, or `32 mins ago`.
        /// Left out on a terminal, you are asked; otherwise defaults to now.
        #[arg(long, value_name = "TIME")]
        at: Option<String>,
        /// Weight, in kilograms or pounds.
        #[arg(short, long, value_name = "NUMBER")]
        weight: Option<f64>,

        /// Length, in centimetres or inches.
        #[arg(long, value_name = "NUMBER")]
        height: Option<f64>,

        /// Head circumference, in centimetres or inches.
        #[arg(long, value_name = "NUMBER")]
        head: Option<f64>,

        /// Which system the numbers are in.
        #[arg(short, long, value_enum)]
        units: Option<System>,
    },

    /// The family's own foods and Huckleberry's curated list.
    Foods {
        /// What to do with the foods.
        #[command(subcommand)]
        action: FoodsAction,
    },

    /// Write a snapshot of everything to a JSON file.
    Export {
        /// How many days to pull.
        #[arg(short, long, value_name = "DAYS")]
        days: Option<u32>,

        /// Where to write it. `-` is stdout, which is also the default.
        #[arg(short, long, value_name = "PATH")]
        out: Option<PathBuf>,
    },

    /// Read and change the stored configuration.
    Config {
        /// What to do with the configuration.
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Print what this build is, one `key=value` line per fact.
    Info,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn the_surface_is_well_formed() {
        Cli::command().debug_assert();
    }

    #[test]
    fn a_bare_invocation_has_no_command() {
        assert!(Cli::try_parse_from(["h"]).unwrap().command.is_none());
    }

    #[test]
    fn the_global_flags_are_accepted_after_a_subcommand() {
        let cli = Cli::try_parse_from([
            "huckleberry-cli",
            "now",
            "--verbose",
            "--config",
            "/tmp/a.toml",
            "--child",
            "abc",
        ])
        .expect("global flags parse");
        assert!(cli.verbose);
        assert_eq!(cli.child.as_deref(), Some("abc"));
        assert_eq!(
            cli.config.as_deref(),
            Some(std::path::Path::new("/tmp/a.toml"))
        );
    }

    #[test]
    fn a_diaper_is_fully_describable_without_a_single_prompt() {
        let cli = Cli::try_parse_from([
            "huckleberry-cli",
            "diaper",
            "--mode",
            "both",
            "--pee",
            "medium",
            "--poo",
            "big",
            "--color",
            "yellow",
            "--consistency",
            "runny",
            "--rash",
            "--notes",
            "a bit sore",
        ])
        .expect("a fully specified diaper");
        let Command::Diaper {
            mode,
            pee,
            poo,
            color,
            consistency,
            rash,
            notes,
            ..
        } = cli.command.unwrap()
        else {
            panic!("expected a diaper");
        };
        assert_eq!(mode, Some(DiaperKind::Both));
        assert_eq!(pee, Some(Amount::Medium));
        assert_eq!(poo, Some(Amount::Big));
        assert_eq!(color, Some(Colour::Yellow));
        assert_eq!(consistency, Some(Consistency::Runny));
        assert!(rash);
        assert_eq!(notes.as_deref(), Some("a bit sore"));
    }

    #[test]
    fn every_value_a_person_may_omit_still_parses_when_omitted() {
        for words in [
            vec!["diaper"],
            vec!["potty"],
            vec!["growth"],
            vec!["trends"],
            vec!["summary"],
            vec!["log"],
            vec!["now"],
            vec!["stripes"],
            vec!["export"],
            vec!["feed", "bottle"],
            vec!["feed", "nursing", "start"],
            vec!["feed", "solids"],
            vec!["child", "use"],
            vec!["config", "set"],
            vec!["edit"],
            vec!["delete"],
            vec!["auth", "login"],
            vec!["foods", "add"],
        ] {
            let mut argv = vec!["huckleberry-cli"];
            argv.extend(words.iter().copied());
            assert!(
                Cli::try_parse_from(&argv).is_ok(),
                "`{}` should parse with nothing supplied",
                words.join(" ")
            );
        }
    }

    #[test]
    fn an_entry_can_be_changed_without_a_single_prompt() {
        let cli = Cli::try_parse_from([
            "huckleberry-cli",
            "edit",
            "--id",
            "diaper/1758572400000-3f2a",
            "--set",
            "mode=both",
            "--set",
            "pee=big",
        ])
        .expect("a fully specified edit");
        let Command::Edit { options } = cli.command.unwrap() else {
            panic!("expected an edit");
        };
        assert_eq!(options.id.as_deref(), Some("diaper/1758572400000-3f2a"));
        assert_eq!(
            options.set,
            vec!["mode=both".to_owned(), "pee=big".to_owned()]
        );
        assert!(!options.list);
    }

    #[test]
    fn deleting_without_a_terminal_still_needs_saying_so() {
        let cli = Cli::try_parse_from([
            "huckleberry-cli",
            "delete",
            "--id",
            "health/1758572400000-3f2a",
            "--yes",
        ])
        .expect("a fully specified delete");
        let Command::Delete { options } = cli.command.unwrap() else {
            panic!("expected a delete");
        };
        assert!(options.yes);
        assert_eq!(options.id.as_deref(), Some("health/1758572400000-3f2a"));
    }

    #[test]
    fn an_unknown_subcommand_is_rejected() {
        assert!(Cli::try_parse_from(["huckleberry-cli", "frobnicate"]).is_err());
    }

    #[test]
    fn a_value_outside_the_fixed_set_is_rejected_by_the_parser() {
        assert!(Cli::try_parse_from(["huckleberry-cli", "diaper", "--mode", "damp"]).is_err());
        assert!(
            Cli::try_parse_from(["huckleberry-cli", "feed", "bottle", "--units", "litres"])
                .is_err()
        );
    }

    #[test]
    fn a_snapshot_can_stand_in_for_the_network_on_any_command() {
        let cli = Cli::try_parse_from(["huckleberry-cli", "summary", "--offline", "/tmp/s.json"])
            .expect("offline parses");
        assert_eq!(
            cli.offline.as_deref(),
            Some(std::path::Path::new("/tmp/s.json"))
        );
    }
}
