//! First-use setup: signing in, and the settings that have no default.
//!
//! Every command runs through here before it does anything, because a command
//! that quietly uses the wrong definition of "today" is worse than one that
//! stops and asks. `--help` and `--version` never reach it: clap answers those
//! and exits before this program starts.
//!
//! The two-audiences rule from [`rules/cli-ux.md`](../docs/rules/cli-ux.md)
//! holds exactly as it does everywhere else. A person at a terminal is asked.
//! An agent, a pipe or a CI job is not: it fails naming the command that would
//! have answered, so nothing ever blocks on a question nobody can see.

pub mod run;

use anyhow::{Result, bail};

use crate::cli::Command;
use crate::config::Config;
use crate::domain::today::DayMode;

/// What a command needs in place before it can answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Needs {
    /// Somebody to sign in as.
    pub credentials: bool,
    /// The settings that have no default worth using.
    pub settings: bool,
}

impl Needs {
    /// Nothing: the command is part of getting set up.
    const NOTHING: Self = Self {
        credentials: false,
        settings: false,
    };
}

/// What this command needs, given where its data is coming from.
///
/// Three families of command are exempt, and all for the same reason: they are
/// how somebody gets out of an unconfigured state. `auth` signs in, `config`
/// sets the settings, and `info` says where both of those live. A setup gate
/// in front of them would be a locked door with the key behind it.
#[must_use]
pub const fn needs(command: &Command, offline: bool) -> Needs {
    match command {
        Command::Auth { .. } | Command::Config { .. } | Command::Info => Needs::NOTHING,
        // A snapshot is a different source, deliberately chosen, and nothing
        // under it opens a socket. There is nobody to be signed in as.
        _ if offline => Needs {
            credentials: false,
            settings: true,
        },
        _ => Needs {
            credentials: true,
            settings: true,
        },
    }
}

/// Settings with no default worth using, in the order setup asks for them.
///
/// `day_start` and `night_start` are only asked of a family counting discrete
/// days: a rolling day has no hour to begin at, so asking would be asking for
/// something that will never be read.
#[must_use]
pub fn missing(config: &Config) -> Vec<&'static str> {
    let Some(mode) = config.day_mode() else {
        return vec!["day_mode"];
    };
    if mode == DayMode::Continuous {
        return Vec::new();
    }
    ["day_start", "night_start"]
        .into_iter()
        .filter(|key| config.get(key).is_none_or(|value| value.trim().is_empty()))
        .collect()
}

/// The refusal a command gets when settings are missing and nobody can be
/// asked, naming every one of them and the command that answers it.
///
/// All of them at once rather than one per run, because the caller that gets
/// this is a script or an agent, and making it discover them one failure at a
/// time is making it run four times to learn four things.
#[must_use]
pub fn missing_message(keys: &[&str]) -> String {
    let program = crate::program_name();
    let mut lines = String::new();
    for key in keys {
        use core::fmt::Write as _;
        let _ = write!(
            lines,
            "\n  {program} config set {key} <VALUE>   # {}",
            Config::describe(key).unwrap_or("see `config show`")
        );
    }
    let subject = if keys.len() == 1 {
        format!("`{}` is not configured", keys[0])
    } else {
        format!(
            "{} settings are not configured: {}",
            keys.len(),
            keys.join(", ")
        )
    };
    format!("{subject}. On a terminal this is asked for; here, set it:{lines}")
}

/// Which mode a baby of this age gets offered first.
///
/// A newborn's day has no shape, so the only honest window is a rolling
/// twenty-four hours. By twelve weeks most babies have a day with a beginning,
/// and "how much has she eaten today" starts to mean since she woke.
///
/// An unknown age is offered discrete days, which is what the word "today"
/// means to everybody who has not thought about it.
#[must_use]
pub const fn default_mode(age_in_days: Option<i64>) -> DayMode {
    match age_in_days {
        Some(days) if days <= 12 * 7 => DayMode::Continuous,
        _ => DayMode::Discrete,
    }
}

/// Refuses on behalf of a command that cannot ask.
pub fn refuse(keys: &[&str]) -> Result<()> {
    bail!(missing_message(keys))
}

/// The hours a day is offered as beginning at, earliest first.
///
/// A menu rather than a typed time, because this is answered at the end of a
/// sign-in with one hand. Anything outside it is still reachable with
/// `config set day_start`, which the menu also offers.
pub const DAY_START_HOURS: [&str; 6] = ["04:00", "05:00", "06:00", "07:00", "08:00", "09:00"];

/// The hours night is offered as beginning at.
pub const NIGHT_START_HOURS: [&str; 7] = [
    "17:00", "18:00", "19:00", "19:30", "20:00", "21:00", "22:00",
];

/// What a setting is offered as, and what it opens on.
///
/// The defaults are Huckleberry's own: a day from seven and a night from
/// eight are what its profile starts every family on, so a parent who does not
/// care can press Enter twice and have what the app already assumed.
#[must_use]
pub fn choices_for(key: &str) -> (&'static [&'static str], &'static str) {
    match key {
        "day_start" => (&DAY_START_HOURS, "07:00"),
        _ => (&NIGHT_START_HOURS, "20:00"),
    }
}

#[cfg(test)]
mod gate {
    use super::*;
    use crate::cli::Cli;
    use clap::Parser;

    fn command(words: &str) -> Command {
        Cli::try_parse_from(std::iter::once("h").chain(words.split_whitespace()))
            .unwrap_or_else(|error| panic!("{words}: {error}"))
            .command
            .unwrap_or_else(|| panic!("{words} parsed to no command"))
    }

    #[test]
    fn the_commands_that_get_you_out_of_an_unconfigured_state_are_never_gated() {
        for words in [
            "auth login",
            "auth status",
            "auth logout",
            "config show",
            "config set",
            "config path",
            "info",
        ] {
            assert_eq!(
                needs(&command(words), false),
                Needs::NOTHING,
                "`{words}` is how somebody gets set up, so it cannot need setup"
            );
        }
    }

    #[test]
    fn every_other_command_needs_an_account_and_the_settings() {
        for words in ["now", "diaper", "summary", "log", "edit", "feed bottle"] {
            let needs = needs(&command(words), false);
            assert!(needs.credentials, "{words}");
            assert!(needs.settings, "{words}");
        }
    }

    #[test]
    fn a_snapshot_needs_the_settings_but_nobody_to_sign_in_as() {
        let needs = needs(&command("now"), true);
        assert!(!needs.credentials, "nothing under --offline opens a socket");
        assert!(needs.settings, "a snapshot still has to be counted somehow");
    }

    #[test]
    fn nothing_is_missing_once_a_family_has_said_how_they_count_a_day() {
        let mut config = Config::default();
        assert_eq!(missing(&config), ["day_mode"]);
        config.set("day_mode", "continuous").expect("a mode");
        assert!(
            missing(&config).is_empty(),
            "a rolling day has no hour to begin at, so there is nothing to ask"
        );
        config.set("day_mode", "discrete").expect("a mode");
        assert_eq!(missing(&config), ["day_start", "night_start"]);
        config.set("day_start", "6am").expect("a time");
        assert_eq!(missing(&config), ["night_start"]);
        config.set("night_start", "7:30pm").expect("a time");
        assert!(missing(&config).is_empty());
    }

    #[test]
    fn a_newborn_is_offered_a_rolling_day_and_an_older_baby_a_real_one() {
        assert_eq!(default_mode(Some(0)), DayMode::Continuous);
        assert_eq!(default_mode(Some(83)), DayMode::Continuous);
        assert_eq!(default_mode(Some(84)), DayMode::Continuous, "twelve weeks");
        assert_eq!(default_mode(Some(85)), DayMode::Discrete);
        assert_eq!(default_mode(Some(400)), DayMode::Discrete);
        assert_eq!(
            default_mode(None),
            DayMode::Discrete,
            "an unknown age gets the word's ordinary meaning"
        );
    }

    #[test]
    fn a_refusal_names_the_command_that_would_have_answered() {
        let message = missing_message(&["day_mode"]);
        assert!(message.contains("config set day_mode"), "{message}");
        assert!(message.contains("continuous"), "{message}");
    }

    #[test]
    fn an_answer_is_read_back_in_the_words_it_was_asked_in() {
        assert_eq!(
            run::confirmation("day_start", "06:00"),
            "The day starts at 6:00 am.",
            "stored as 24-hour, confirmed as somebody says it"
        );
        assert_eq!(
            run::confirmation("night_start", "19:30"),
            "Night starts at 7:30 pm."
        );
        assert!(
            run::confirmation("day_mode", "continuous").contains("newborn"),
            "the confirmation says what was chosen, not just its name"
        );
    }

    #[test]
    fn a_refusal_names_every_missing_setting_rather_than_one_per_run() {
        let message = missing_message(&["day_start", "night_start"]);
        assert!(message.contains("config set day_start"), "{message}");
        assert!(message.contains("config set night_start"), "{message}");
        assert!(message.contains("2 settings"), "{message}");
    }
}
