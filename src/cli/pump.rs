//! Pump recording and timer arguments, shared by flags and the shell.

use clap::{Args, Subcommand};

use super::Units;

/// Recorded quantities for a manual pump or a completed timer.
#[derive(Debug, Clone, PartialEq, Default, Args)]
pub struct PumpValues {
    /// Total expressed milk. Use this or separate --left and --right amounts.
    #[arg(short, long, alias = "total", conflicts_with_all = ["left", "right"], value_name = "NUMBER")]
    pub amount: Option<f64>,

    /// Milk expressed on the left, including zero. Omitted amounts are asked for.
    #[arg(long, value_name = "NUMBER")]
    pub left: Option<f64>,

    /// Milk expressed on the right, including zero. Omitted amounts are asked for.
    #[arg(long, value_name = "NUMBER")]
    pub right: Option<f64>,

    /// Units for all amounts. The configured units are offered by the prompt.
    #[arg(short, long, value_enum)]
    pub units: Option<Units>,

    /// Anything worth writing down.
    #[arg(short, long, value_name = "TEXT")]
    pub notes: Option<String>,
}

/// A completed pumping session supplied manually.
#[derive(Debug, Clone, PartialEq, Default, Args)]
pub struct PumpLogOptions {
    /// When it began: `now`, `358 am`, or `32 mins ago`.
    /// Asked first on a terminal; otherwise defaults to now.
    #[arg(long, alias = "start", value_name = "TIME")]
    pub at: Option<String>,

    /// Session duration in minutes, including fractional minutes. Optional.
    #[arg(short, long, value_name = "MINUTES")]
    pub duration: Option<f64>,

    /// The total or the separate side amounts, units, and notes.
    #[command(flatten)]
    pub values: PumpValues,
}

/// Everything `pump` can do.
#[derive(Debug, Clone, PartialEq, Subcommand)]
pub enum PumpAction {
    /// Record a completed pumping session.
    #[command(visible_alias = "manual")]
    Log {
        /// When, how long, and how much was expressed.
        #[command(flatten)]
        options: PumpLogOptions,
    },
    /// Start a pumping timer.
    Start {
        /// When it began. Asked on a terminal; otherwise defaults to now.
        #[arg(short, long, alias = "at", value_name = "TIME")]
        start: Option<String>,
    },
    /// Pause the running pump timer.
    Pause {
        /// When it paused. Asked on a terminal; otherwise defaults to now.
        #[arg(long, value_name = "TIME")]
        at: Option<String>,
    },
    /// Resume the paused timer, counting the pause in the session duration.
    Resume {
        /// When it resumed. Asked on a terminal; otherwise defaults to now.
        #[arg(long, value_name = "TIME")]
        at: Option<String>,
    },
    /// Finish the timer and record the expressed amounts.
    #[command(visible_alias = "end")]
    Stop {
        /// When it ended. Asked on a terminal; otherwise defaults to now.
        #[arg(long, value_name = "TIME")]
        at: Option<String>,

        /// Expressed amounts, units, and notes.
        #[command(flatten)]
        values: PumpValues,
    },
    /// Discard the running pump timer without recording it.
    Cancel,
    /// Show the pump timer's state and elapsed time.
    Status,
}
