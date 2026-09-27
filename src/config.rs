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

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

/// The file name inside this tool's own configuration directory.
const FILE_NAME: &str = "config.toml";

/// Everything this tool remembers between runs.
///
/// `deny_unknown_fields` turns a typo in a hand-edited file into an error that
/// names the settings that do exist, rather than a line that silently does
/// nothing. `default` fills in whatever the file leaves out, so a partial file
/// is valid and an empty one is the defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// The word `greet` opens with.
    pub greeting: String,
    /// Who `greet` greets when `--name` is left out. Empty means "ask me".
    pub name: String,
    /// Print detailed diagnostics without passing `--verbose` every time.
    pub verbose: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            greeting: "Hello".to_owned(),
            name: String::new(),
            verbose: false,
        }
    }
}

impl Config {
    /// Every setting name, in the order `config show` prints them.
    pub const KEYS: [&'static str; 3] = ["greeting", "name", "verbose"];

    /// One line saying what a setting is for, or `None` for an unknown key.
    #[must_use]
    pub fn describe(key: &str) -> Option<&'static str> {
        match key {
            "greeting" => Some("the word `greet` opens with"),
            "name" => Some("who `greet` greets when --name is left out"),
            "verbose" => Some("print detailed diagnostics without --verbose"),
            _ => None,
        }
    }

    /// The effective settings as `key`/`value` pairs, in [`Self::KEYS`] order.
    #[must_use]
    pub fn entries(&self) -> Vec<(&'static str, String)> {
        vec![
            ("greeting", self.greeting.clone()),
            ("name", self.name.clone()),
            ("verbose", self.verbose.to_string()),
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
            "greeting" => {
                if value.is_empty() {
                    bail!("greeting cannot be empty");
                }
                value.clone_into(&mut self.greeting);
            }
            "name" => value.clone_into(&mut self.name),
            "verbose" => self.verbose = parse_bool(value)?,
            _ => bail!(unknown_key_message(key)),
        }
        Ok(())
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
mod tests {
    use super::*;

    #[test]
    fn an_empty_file_is_the_defaults() {
        assert_eq!(parse("").expect("an empty file parses"), Config::default());
    }

    #[test]
    fn a_partial_file_keeps_the_other_defaults() {
        let config = parse("name = \"Ada\"\n").expect("a partial file parses");
        assert_eq!(config.name, "Ada");
        assert_eq!(config.greeting, Config::default().greeting);
    }

    #[test]
    fn an_unknown_setting_in_the_file_is_reported() {
        let error = parse("greetings = \"Yo\"\n").expect_err("a typo is caught");
        assert!(format!("{error:#}").contains("greetings"), "{error:#}");
    }

    #[test]
    fn what_is_written_is_what_is_read_back() {
        let mut config = Config::default();
        config.set("greeting", "Howdy").expect("a greeting is text");
        config.set("verbose", "true").expect("a bool is a bool");
        let text = serialize(&config).expect("serializing");
        assert_eq!(parse(&text).expect("parsing"), config);
    }

    #[test]
    fn every_key_is_shown_and_readable() {
        let config = Config::default();
        let shown: Vec<&str> = config.entries().into_iter().map(|(key, _)| key).collect();
        assert_eq!(shown, Config::KEYS);
        for key in Config::KEYS {
            assert!(config.get(key).is_some(), "{key} is not readable");
            assert!(Config::describe(key).is_some(), "{key} is not described");
        }
    }

    #[test]
    fn every_key_can_be_set() {
        let mut config = Config::default();
        config.set("greeting", "Howdy").expect("greeting");
        config.set("name", "Ada").expect("name");
        config.set("verbose", "true").expect("verbose");
        assert_eq!(config.greeting, "Howdy");
        assert_eq!(config.name, "Ada");
        assert!(config.verbose);
    }

    #[test]
    fn a_value_is_trimmed_before_it_is_stored() {
        let mut config = Config::default();
        config.set("name", "  Ada \n").expect("a padded name");
        assert_eq!(config.name, "Ada");
    }

    #[test]
    fn an_empty_greeting_is_refused() {
        let mut config = Config::default();
        assert!(config.set("greeting", "   ").is_err());
    }

    #[test]
    fn a_bool_that_is_not_one_names_both_spellings() {
        let mut config = Config::default();
        let error = config.set("verbose", "yes").expect_err("yes is not a bool");
        assert!(format!("{error:#}").contains("true or false"), "{error:#}");
    }

    #[test]
    fn an_unknown_key_lists_the_known_ones() {
        let message = unknown_key_message("verbos");
        for key in Config::KEYS {
            assert!(message.contains(key), "{message} omits {key}");
        }
    }

    #[test]
    fn the_file_sits_under_the_tools_own_directory() {
        let path = path_in(Path::new("/home/ada/.config"));
        assert!(path.ends_with(Path::new(env!("CARGO_PKG_NAME")).join(FILE_NAME)));
    }

    #[test]
    fn xdg_config_home_wins_over_home() {
        let chosen = config_home(
            Some(PathBuf::from("/xdg")),
            Some(PathBuf::from("/home/ada")),
        );
        assert_eq!(chosen, Some(PathBuf::from("/xdg")));
    }

    #[test]
    fn a_relative_xdg_config_home_is_ignored() {
        let chosen = config_home(Some(PathBuf::from("xdg")), Some(PathBuf::from("/home/ada")));
        assert_eq!(chosen, Some(PathBuf::from("/home/ada/.config")));
    }

    #[test]
    fn without_either_variable_there_is_no_directory() {
        assert_eq!(config_home(None, None), None);
    }

    #[test]
    fn a_missing_file_loads_as_the_defaults() {
        let missing = Path::new("/nonexistent/no-such-directory/config.toml");
        assert_eq!(
            load(missing).expect("a missing file is fine"),
            Config::default()
        );
    }
}
