//! The configuration file: what it holds, where it lives, how it is read.
//!
//! The settings are one typed struct with defaults, so a new setting is one
//! more field plus one more arm in [`Config::set`]. Everything that decides
//! something (parsing, validating a value, working out the path) is a pure
//! function tested inline; the two functions that touch the disk ([`load`] and
//! [`save`]) hold no decisions at all.
//!
//! The file is TOML because a person is expected to open it in an editor, and
//! it lives under the XDG configuration directory (`$XDG_CONFIG_HOME`, else
//! `~/.config`) so it is where every other command-line tool keeps its own.
//!
//! **No secret is ever written here.** The email, the password and the session
//! token live in `credentials.toml` beside it, with permissions to match: see
//! [`crate::credentials`].

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::domain::today::{DayMode, DayRule};

/// The file name inside this tool's own configuration directory.
const FILE_NAME: &str = "config.toml";

/// How many days of history a command reads when nothing says otherwise.
pub const DEFAULT_DAYS: u32 = 7;

/// How often the dashboard re-reads, in seconds.
pub const DEFAULT_REFRESH_SECONDS: u32 = 20;

/// Everything this tool remembers between runs.
///
/// `deny_unknown_fields` turns a typo in a hand-edited file into an error that
/// names the settings that do exist, rather than a line that silently does
/// nothing. `default` fills in whatever the file leaves out, so a partial file
/// is valid and an empty one is the defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Which child every command acts on. Empty means "ask me", and
    /// `child use` is what fills it in.
    pub child: String,
    /// The family's timezone, as an IANA name. Every day boundary, night
    /// window and offset is computed in it, so a laptop in another zone still
    /// shows the days the phone app shows.
    pub timezone: String,
    /// How many days of history to read when `--days` is left out.
    pub days: u32,
    /// Which volume unit to show amounts in, and to read `--amount` as.
    pub units: String,
    /// Which system growth measurements are taken in.
    pub measurements: String,
    /// How often the dashboard re-reads, in seconds.
    pub refresh: u32,
    /// Print detailed diagnostics without passing `--verbose` every time.
    pub verbose: bool,
    /// How this family counts a day: `continuous` or `discrete`. Empty means
    /// nobody has said, which is what first-use setup asks about.
    pub day_mode: String,
    /// When a day begins, as `HH:MM`, for a family counting discrete days.
    /// Empty means nobody has said.
    pub day_start: String,
    /// When night begins, as `HH:MM`. Empty means use whatever Huckleberry's
    /// own profile says.
    pub night_start: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            child: String::new(),
            // Not the machine's timezone: the family's is what Huckleberry
            // stores offsets against, and guessing it from the host would be
            // wrong for a parent travelling.
            timezone: "UTC".to_owned(),
            days: DEFAULT_DAYS,
            units: "ml".to_owned(),
            measurements: "metric".to_owned(),
            refresh: DEFAULT_REFRESH_SECONDS,
            verbose: false,
            // Empty rather than a default, because there is no default worth
            // having: midnight is wrong for everybody, and which of the two
            // modes is right depends on how old the baby is. Setup asks.
            day_mode: String::new(),
            day_start: String::new(),
            night_start: String::new(),
        }
    }
}

impl Config {
    /// Every setting name, in the order `config show` prints them.
    pub const KEYS: [&'static str; 10] = [
        "child",
        "timezone",
        "days",
        "units",
        "measurements",
        "refresh",
        "verbose",
        "day_mode",
        "day_start",
        "night_start",
    ];

    /// One line saying what a setting is for, or `None` for an unknown key.
    #[must_use]
    pub fn describe(key: &str) -> Option<&'static str> {
        match key {
            "child" => Some("which child commands act on (see `child list`)"),
            "timezone" => Some("the family's IANA timezone, e.g. America/New_York"),
            "days" => Some("how many days of history to read by default"),
            "units" => Some("volume units for amounts: ml or oz"),
            "measurements" => Some("growth measurements: metric or imperial"),
            "refresh" => Some("how often the dashboard re-reads, in seconds"),
            "verbose" => Some("print detailed diagnostics without --verbose"),
            "day_mode" => Some("how a day is counted: continuous (rolling 24h) or discrete"),
            "day_start" => Some("when a day begins, e.g. 6:00 or 6am (discrete days only)"),
            "night_start" => Some("when night begins, e.g. 19:30 or 7:30pm"),
            _ => None,
        }
    }

    /// The effective settings as `key`/`value` pairs, in [`Self::KEYS`] order.
    #[must_use]
    pub fn entries(&self) -> Vec<(&'static str, String)> {
        vec![
            ("child", self.child.clone()),
            ("timezone", self.timezone.clone()),
            ("days", self.days.to_string()),
            ("units", self.units.clone()),
            ("measurements", self.measurements.clone()),
            ("refresh", self.refresh.to_string()),
            ("verbose", self.verbose.to_string()),
            ("day_mode", self.day_mode.clone()),
            ("day_start", self.day_start.clone()),
            ("night_start", self.night_start.clone()),
        ]
    }

    /// The current value of one setting, or `None` for an unknown key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<String> {
        self.entries()
            .into_iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| value)
    }

    /// Changes one setting in place. The failure names what was wrong: an
    /// unknown key lists the keys, a bad value says what shape was expected.
    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        let value = value.trim();
        match key {
            "child" => value.clone_into(&mut self.child),
            "timezone" => {
                // Validated on the way in rather than on the way out: a typo
                // here would otherwise surface as every day boundary being
                // silently wrong.
                crate::domain::Calendar::new(value)?;
                value.clone_into(&mut self.timezone);
            }
            "days" => self.days = parse_days(value)?,
            "units" => self.units = parse_one_of(value, &["ml", "oz"], "units")?,
            "measurements" => {
                self.measurements = parse_one_of(value, &["metric", "imperial"], "measurements")?;
            }
            "refresh" => self.refresh = parse_refresh(value)?,
            "verbose" => self.verbose = parse_bool(value)?,
            "day_mode" => self.day_mode = parse_day_mode(value)?,
            "day_start" => self.day_start = parse_hour(value, "day_start")?,
            "night_start" => self.night_start = parse_hour(value, "night_start")?,
            _ => bail!(unknown_key_message(key)),
        }
        Ok(())
    }

    /// The child every command acts on, or `None` when nobody has said.
    #[must_use]
    pub fn child(&self) -> Option<&str> {
        let child = self.child.trim();
        (!child.is_empty()).then_some(child)
    }

    /// The calendar the family's days are counted in.
    pub fn calendar(&self) -> Result<crate::domain::Calendar> {
        crate::domain::Calendar::new(&self.timezone)
    }

    /// How this family counts a day, or `None` when nobody has said.
    #[must_use]
    pub fn day_mode(&self) -> Option<DayMode> {
        DayMode::from_key(self.day_mode.trim())
    }

    /// When a day begins, as an hour fraction.
    #[must_use]
    pub fn day_start_hour(&self) -> Option<f64> {
        hour_fraction(&self.day_start)
    }

    /// When night begins, as an hour fraction, when the family has said.
    #[must_use]
    pub fn night_start_hour(&self) -> Option<f64> {
        hour_fraction(&self.night_start)
    }

    /// The rule every screen counts days by.
    ///
    /// An unanswered `day_mode` reads as discrete from midnight, which is what
    /// the tool does while setup has not run yet. It is deliberately the
    /// least surprising wrong answer rather than a refusal, so that
    /// `--offline` and a hand-written configuration still draw a screen.
    #[must_use]
    pub fn day_rule(&self) -> DayRule {
        let night = self.night_start_hour();
        match self.day_mode() {
            Some(DayMode::Continuous) => DayRule {
                mode: DayMode::Continuous,
                day_start_hour: None,
                night_start_hour: night,
            },
            _ => DayRule {
                mode: DayMode::Discrete,
                day_start_hour: self.day_start_hour(),
                night_start_hour: night,
            },
        }
    }
}

/// Reads a day-counting mode, naming both spellings when it is neither.
fn parse_day_mode(value: &str) -> Result<String> {
    let lowered = value.to_lowercase();
    if DayMode::from_key(&lowered).is_some() {
        return Ok(lowered);
    }
    bail!("expected day_mode to be continuous or discrete, not `{value}`")
}

/// Reads a time of day, and stores it in one spelling.
///
/// A bare `6` is two different times and this setting is read at 3am, so it is
/// refused rather than guessed at: the refusal says both ways to be clear.
fn parse_hour(value: &str, setting: &str) -> Result<String> {
    match crate::domain::clock::parse(value) {
        Some(crate::domain::clock::Typed::Certain(time)) => {
            Ok(format!("{:02}:{:02}", time.hour, time.minute))
        }
        Some(crate::domain::clock::Typed::Ambiguous { .. }) => bail!(
            "`{value}` could be morning or evening: write {setting} as 24-hour \
             time (`06:00`) or say am or pm (`6am`)"
        ),
        None => bail!("expected {setting} to be a time like 06:00 or 6am, not `{value}`"),
    }
}

/// A stored `HH:MM` as an hour with a fraction.
fn hour_fraction(stored: &str) -> Option<f64> {
    match crate::domain::clock::parse(stored.trim()) {
        Some(crate::domain::clock::Typed::Certain(time)) => {
            Some(f64::from(time.hour) + f64::from(time.minute) / 60.0)
        }
        _ => None,
    }
}

/// Reads a `true`/`false` setting, naming both spellings when it is neither.
fn parse_bool(value: &str) -> Result<bool> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        other => bail!("expected true or false, not `{other}`"),
    }
}

/// Reads a window of days. Zero would be a window containing nothing, and a
/// year and a half of newborn history is more than anybody means to ask for.
fn parse_days(value: &str) -> Result<u32> {
    let days: u32 = value
        .parse()
        .with_context(|| format!("expected a number of days, not `{value}`"))?;
    if !(1..=400).contains(&days) {
        bail!("expected 1 to 400 days, not {days}");
    }
    Ok(days)
}

/// Reads a refresh interval. Anything under a second would be a busy loop
/// against somebody else's server.
fn parse_refresh(value: &str) -> Result<u32> {
    let seconds: u32 = value
        .parse()
        .with_context(|| format!("expected a number of seconds, not `{value}`"))?;
    if !(1..=3600).contains(&seconds) {
        bail!("expected 1 to 3600 seconds, not {seconds}");
    }
    Ok(seconds)
}

/// Reads a setting with a fixed set of spellings.
fn parse_one_of(value: &str, allowed: &[&str], setting: &str) -> Result<String> {
    let lowered = value.to_lowercase();
    if allowed.contains(&lowered.as_str()) {
        return Ok(lowered);
    }
    bail!(
        "expected {setting} to be one of {}, not `{value}`",
        allowed.join(" or ")
    )
}

/// The refusal for a key no setting answers to, listing the ones that exist.
#[must_use]
pub fn unknown_key_message(key: &str) -> String {
    format!("unknown setting `{key}`: try {}", Config::KEYS.join(", "))
}

/// Reads the configuration text. Every field the text leaves out keeps its
/// default, so an empty file and a missing file mean the same thing.
pub fn parse(text: &str) -> Result<Config> {
    toml::from_str(text).context("parsing the configuration")
}

/// Renders the configuration as the TOML that is written back to disk.
pub fn serialize(config: &Config) -> Result<String> {
    toml::to_string_pretty(config).context("serializing the configuration")
}

/// The configuration file inside a configuration directory.
#[must_use]
pub fn path_in(config_home: &Path) -> PathBuf {
    config_home.join(env!("CARGO_PKG_NAME")).join(FILE_NAME)
}

/// The configuration directory to use, given the two environment variables
/// that can name it.
///
/// A relative `XDG_CONFIG_HOME` is ignored, as the XDG specification requires,
/// rather than resolved against the working directory.
#[must_use]
pub fn config_home(xdg_config_home: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    xdg_config_home
        .filter(|path| path.is_absolute())
        .or_else(|| home.map(|home| home.join(".config")))
}

/// Where the configuration lives when `--config` did not say.
pub fn default_path() -> Result<PathBuf> {
    let home = config_home(
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
    )
    .context("no configuration directory: set HOME or XDG_CONFIG_HOME, or pass --config <PATH>")?;
    Ok(path_in(&home))
}

/// Loads the configuration. A file that is not there is not an error: it means
/// nothing has been configured yet, which is the default configuration.
pub fn load(path: &Path) -> Result<Config> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text).with_context(|| format!("reading {}", path.display())),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Config::default()),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

/// Saves the configuration, creating the directory it belongs in.
pub fn save(path: &Path, config: &Config) -> Result<()> {
    let text = serialize(config)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod files {
    use super::*;

    #[test]
    fn an_empty_file_is_the_defaults() {
        assert_eq!(parse("").expect("an empty file parses"), Config::default());
    }

    #[test]
    fn a_partial_file_keeps_the_other_defaults() {
        let config = parse("child = \"abc\"\n").expect("a partial file parses");
        assert_eq!(config.child, "abc");
        assert_eq!(config.days, Config::default().days);
    }

    #[test]
    fn an_unknown_setting_in_the_file_is_reported_by_name() {
        let error = parse("childs = \"abc\"\n").expect_err("a typo is caught");
        assert!(format!("{error:#}").contains("childs"), "{error:#}");
    }

    #[test]
    fn what_is_written_is_what_is_read_back() {
        let mut config = Config::default();
        config.set("child", "abc").expect("a child id");
        config
            .set("timezone", "America/New_York")
            .expect("a timezone");
        config.set("days", "30").expect("a window");
        let text = serialize(&config).expect("serializing");
        assert_eq!(parse(&text).expect("parsing"), config);
    }

    #[test]
    fn the_file_lives_under_the_tools_own_directory() {
        let path = path_in(Path::new("/home/ada/.config"));
        assert!(
            path.ends_with("huckleberry-cli/config.toml"),
            "{}",
            path.display()
        );
    }

    #[test]
    fn a_relative_xdg_directory_is_ignored_as_the_specification_says() {
        let home = config_home(
            Some(PathBuf::from("relative/path")),
            Some(PathBuf::from("/home/ada")),
        );
        assert_eq!(home, Some(PathBuf::from("/home/ada/.config")));
    }

    #[test]
    fn no_secret_has_a_setting_to_be_written_into() {
        for forbidden in ["password", "email", "token", "secret", "session"] {
            assert!(
                !Config::KEYS.contains(&forbidden),
                "`{forbidden}` must not be a configuration setting"
            );
        }
    }
}

#[cfg(test)]
mod settings {
    use super::*;

    #[test]
    fn every_key_is_shown_described_and_readable() {
        let config = Config::default();
        let shown: Vec<&str> = config.entries().into_iter().map(|(key, _)| key).collect();
        assert_eq!(shown, Config::KEYS);
        for key in Config::KEYS {
            assert!(config.get(key).is_some(), "{key} is not readable");
            assert!(Config::describe(key).is_some(), "{key} is not described");
        }
    }

    #[test]
    fn every_key_can_be_set_and_read_back() {
        let mut config = Config::default();
        for (key, value) in [
            ("child", "abc"),
            ("timezone", "Europe/Berlin"),
            ("days", "14"),
            ("units", "oz"),
            ("measurements", "imperial"),
            ("refresh", "30"),
            ("verbose", "true"),
            ("day_mode", "continuous"),
        ] {
            config
                .set(key, value)
                .unwrap_or_else(|error| panic!("{key}: {error:#}"));
            assert_eq!(config.get(key).as_deref(), Some(value), "{key}");
        }
    }

    #[test]
    fn a_timezone_that_does_not_exist_is_refused_when_it_is_set() {
        let mut config = Config::default();
        assert!(config.set("timezone", "Mars/Olympus_Mons").is_err());
        assert_eq!(config.timezone, "UTC", "the old value survives a refusal");
    }

    #[test]
    fn a_unit_the_tool_does_not_know_is_refused_and_names_the_ones_it_does() {
        let mut config = Config::default();
        let error = config.set("units", "litres").expect_err("refused");
        assert!(format!("{error:#}").contains("ml or oz"), "{error:#}");
    }

    #[test]
    fn a_unit_is_accepted_whatever_case_it_is_typed_in() {
        let mut config = Config::default();
        config.set("units", "OZ").expect("a unit");
        assert_eq!(config.units, "oz");
    }

    #[test]
    fn a_window_of_no_days_is_refused() {
        let mut config = Config::default();
        assert!(config.set("days", "0").is_err());
        assert!(config.set("days", "4000").is_err());
        assert!(config.set("days", "a week").is_err());
    }

    #[test]
    fn a_refresh_that_would_be_a_busy_loop_is_refused() {
        let mut config = Config::default();
        assert!(config.set("refresh", "0").is_err());
        assert!(config.set("refresh", "5").is_ok());
    }

    #[test]
    fn an_unset_child_is_absent_rather_than_an_empty_string() {
        let mut config = Config::default();
        assert_eq!(config.child(), None);
        config.set("child", "  abc  ").expect("a child id");
        assert_eq!(config.child(), Some("abc"));
    }

    #[test]
    fn a_day_is_counted_one_of_exactly_two_ways() {
        let mut config = Config::default();
        assert_eq!(config.day_mode(), None, "nobody has said yet");
        config.set("day_mode", "Continuous").expect("a mode");
        assert_eq!(config.day_mode(), Some(DayMode::Continuous));
        let error = config.set("day_mode", "weekly").expect_err("refused");
        assert!(format!("{error:#}").contains("discrete"), "{error:#}");
    }

    #[test]
    fn a_day_start_is_stored_in_one_spelling_however_it_was_typed() {
        for (typed, stored) in [
            ("6am", "06:00"),
            ("06:00", "06:00"),
            ("6:30 am", "06:30"),
            ("19:30", "19:30"),
            ("7:30pm", "19:30"),
        ] {
            let mut config = Config::default();
            config
                .set("day_start", typed)
                .unwrap_or_else(|error| panic!("{typed}: {error:#}"));
            assert_eq!(config.day_start, stored, "{typed}");
        }
    }

    #[test]
    fn a_time_that_could_be_either_half_of_the_day_is_refused_not_guessed() {
        let mut config = Config::default();
        let error = config.set("day_start", "6").expect_err("refused");
        assert!(format!("{error:#}").contains("am or pm"), "{error:#}");
        assert!(config.day_start.is_empty(), "the refusal changed nothing");
        assert!(config.set("night_start", "half past seven").is_err());
    }

    #[test]
    fn a_configured_hour_reads_back_as_an_hour_fraction() {
        let mut config = Config::default();
        config.set("day_start", "6:30am").expect("a time");
        config.set("night_start", "19:00").expect("a time");
        assert!((config.day_start_hour().expect("set") - 6.5).abs() < f64::EPSILON);
        assert!((config.night_start_hour().expect("set") - 19.0).abs() < f64::EPSILON);
        assert_eq!(Config::default().day_start_hour(), None);
    }

    #[test]
    fn the_rule_follows_the_mode_and_falls_back_to_discrete_midnight() {
        let mut config = Config::default();
        assert_eq!(config.day_rule(), DayRule::discrete_default());
        config.set("day_mode", "continuous").expect("a mode");
        assert_eq!(config.day_rule().mode, DayMode::Continuous);
        assert_eq!(
            config.day_rule().day_start_hour,
            None,
            "a rolling day has no hour to begin at"
        );
        config.set("day_mode", "discrete").expect("a mode");
        config.set("day_start", "6am").expect("a time");
        assert_eq!(config.day_rule().day_start_hour, Some(6.0));
    }

    #[test]
    fn an_unknown_key_lists_the_ones_that_exist() {
        let mut config = Config::default();
        let error = config.set("childs", "abc").expect_err("refused");
        assert!(format!("{error:#}").contains("timezone"), "{error:#}");
    }
}
