//! Statistics management for the bot.

use std::{hash::Hash, io::ErrorKind};

use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use time::{Month, OffsetDateTime};
use tokio::fs;

use crate::dirs::DIRS;

/// Main structure that hold the statistics for different time frames.
#[derive(Serialize, Deserialize)]
pub struct Stats {
    /// Statistics only for the current month. Will be reset on each month.
    current: (Month, Statistics),
    /// Overall statistics through the whole lifetime of `togglebot`.
    total: Statistics,
    /// Marker for sorting state of command usage statistics inside of [`Statistics`].
    #[serde(skip)]
    sorted: bool,
}

impl Stats {
    /// Increment the usage counter for the given command by one.
    pub fn increment(&mut self, cmd: Command<'_>) {
        // Don't track commands that are too long.
        if cmd.str_len() > 50 {
            return;
        }

        let month = OffsetDateTime::now_utc().month();
        if self.current.0 != month {
            self.current.0 = month;
            self.current.1 = Statistics::default();
        }

        for stats in [&mut self.current.1, &mut self.total] {
            *stats.command_usage.get_counter(cmd) += 1;

            // Clean up the command maps, keeping only the 50 most used.
            limit_size(&mut stats.command_usage.custom, 50);
            limit_size(&mut stats.command_usage.unknown, 50);
        }

        // Mark as unsorted, as we modified counts and have to sort again.
        self.sorted = false;
    }

    /// Shorthand to increment the usage counter of a built-in command.
    pub fn increment_builtin(&mut self, cmd: BuiltinCommand) {
        self.increment(Command::Builtin(cmd));
    }

    /// Get the current or total statistics.
    #[must_use]
    pub fn get(&mut self, total: bool) -> &Statistics {
        if !self.sorted {
            self.total.command_usage.sort();
            self.current.1.command_usage.sort();
            self.sorted = true;
        }

        if total {
            &self.total
        } else {
            &self.current.1
        }
    }

    /// Erase the usage counter for a custom command. This is usually done when a custom command
    /// is deleted.
    pub fn erase_custom(&mut self, name: &str) {
        for stats in [&mut self.current.1, &mut self.total] {
            stats.command_usage.custom.shift_remove(name);
        }
    }
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            current: (Month::January, Statistics::default()),
            total: Statistics::default(),
            sorted: false,
        }
    }
}

/// Statistics for various details about `togglebot` (well, currently only command usage counters).
#[derive(Clone, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(Debug))]
pub struct Statistics {
    /// Usage counters for commands.
    #[serde(default)]
    pub command_usage: CommandUsage,
}

/// Counters for all available **user** commands. These are split between builtin, custom and
/// unknown to allow better visualization and categorization.
#[derive(Clone, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(Debug))]
pub struct CommandUsage {
    /// Standard, built-in commands. Helps to find out which built in commands might be removed
    /// in the future due to low usage.
    pub builtin: IndexMap<BuiltinCommand, u64>,
    /// Custom defined commands. Allows admins to see what commands might be retired.
    pub custom: IndexMap<String, u64>,
    /// Unrecognized commands. Can give insight about common misspells or wished-for commands.
    pub unknown: IndexMap<String, u64>,
}

impl CommandUsage {
    /// Get a mutable reference to the counter for given command. This automatically creates a new
    /// entry for the command if it doesn't exist yet.
    fn get_counter(&mut self, cmd: Command<'_>) -> &mut u64 {
        match cmd {
            Command::Builtin(cmd) => self.builtin.entry(cmd).or_default(),
            Command::Custom(cmd) => {
                let cmd = cmd.strip_prefix('!').unwrap_or(cmd).to_owned();
                self.custom.entry(cmd).or_default()
            }
            Command::Unknown(cmd) => {
                let cmd = cmd.strip_prefix('!').unwrap_or(cmd).to_owned();
                self.unknown.entry(cmd).or_default()
            }
        }
    }

    /// Sort the statistics of all commands.
    fn sort(&mut self) {
        sort(&mut self.builtin);
        sort(&mut self.custom);
        sort(&mut self.unknown);
    }
}

/// A command that belongs in one of the defined categories.
#[derive(Clone, Copy, Debug)]
pub enum Command<'a> {
    /// Pre-defined command.
    Builtin(BuiltinCommand),
    /// Custom command, created by admins or owners.
    Custom(&'a str),
    /// Unrecognized command.
    Unknown(&'a str),
}

impl<'a> Command<'a> {
    /// Get the string length of the command.
    fn str_len(&self) -> usize {
        match self {
            Self::Builtin(_) => 0,
            Self::Custom(v) | Self::Unknown(v) => v.len(),
        }
    }
}

/// One of the few pre-defined commands that are always available.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum BuiltinCommand {
    /// Info about the bots.
    Help,
    /// List of available commands.
    Commands,
    /// Several social media links.
    Links,
    /// Rust crate info lookup.
    Crate,
    /// Rust crate docs lookup.
    Doc,
    /// Fake ban for fun.
    Ban,
    /// Get the current date (in UTC).
    Today,
    /// Convert Fahrenheit degrees to Celsius degrees.
    FahrenheitToCelsius,
    /// Convert Celsius degrees to Fahrenheit degrees.
    CelsiusToFahrenheit,
    /// Any other command that may have existed in the past.
    ///
    /// This uses the `#[serde(other)]` configuration, so that commands can be deleted and then
    /// captured by this variant during deserialization. It allows to clean up the file system
    /// copy by doing a _deserialization -> serialization_ pass through [`serde`].
    #[serde(other)]
    Deprecated,
}

impl BuiltinCommand {
    /// Get the display name for this command. It does **not** include the command prefix.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Help => "help",
            Self::Commands => "commands",
            Self::Links => "links",
            Self::Crate => "crate",
            Self::Doc => "doc",
            Self::Ban => "ban",
            Self::Today => "today",
            Self::FahrenheitToCelsius => "ftoc",
            Self::CelsiusToFahrenheit => "ctof",
            Self::Deprecated => "deprecated",
        }
    }
}

/// Read and parse the file system copy of the current statistics.
pub fn load() -> Result<Stats> {
    let state = match std::fs::read(DIRS.statistics_file()) {
        Ok(buf) => buf,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Stats::default()),
        Err(e) => return Err(e.into()),
    };

    serde_json::from_slice(&state).context("failed parsing state data")
}

/// Save back current in-memory statistics to the file system.
pub async fn save(state: &Stats) -> Result<()> {
    fs::create_dir_all(DIRS.data_dir()).await?;

    let json = serde_json::to_vec_pretty(state)?;

    fs::write(DIRS.statistics_temp_file(), &json).await?;
    fs::rename(DIRS.statistics_temp_file(), DIRS.statistics_file()).await?;

    Ok(())
}

/// Limit the size of the given map to a certain amount by removing entries with the least count
/// first.
///
/// The size is **NOT** reduce unless the map is at the double of the set limit. That is, to reduce
/// the overhead, as the whole map needs to be cloned to determine the smallest counters.
fn limit_size(map: &mut IndexMap<String, u64>, limit: usize) {
    if map.len() < limit * 2 {
        return;
    }

    let inverse = map
        .iter()
        .map(|(k, v)| (*v, k.clone()))
        .collect::<IndexMap<_, _>>();

    for cmd in inverse.into_values().take(map.len() - limit) {
        map.swap_remove(&cmd);
    }

    sort(map);
}

/// Sort any index map, first descending by the value (the usage counter), then ascending by the
/// key (which usually represents the used command).
fn sort<K>(map: &mut IndexMap<K, u64>)
where
    K: Eq + Hash + Ord,
{
    map.sort_by(|key_a, value_a, key_b, value_b| {
        value_a.cmp(value_b).reverse().then(key_a.cmp(key_b))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn increment() {
        let mut stats = Stats::default();
        for _ in 0..2 {
            stats.increment_builtin(BuiltinCommand::Help);
        }

        for _ in 0..3 {
            stats.increment(Command::Custom("me"));
        }

        for _ in 0..4 {
            stats.increment(Command::Unknown("who"));
        }

        let usage = &stats.get(false).command_usage;
        assert_eq!(2, usage.builtin[&BuiltinCommand::Help]);
        assert_eq!(3, usage.custom["me"]);
        assert_eq!(4, usage.unknown["who"]);
    }

    #[test]
    fn erase_custom() {
        let mut stats = Stats::default();
        stats.increment(Command::Custom("me"));
        stats.increment(Command::Custom("you"));
        stats.erase_custom("you");

        let usage = &stats.get(false).command_usage;
        assert_eq!(1, usage.custom["me"]);
        assert!(usage.custom.get("you").is_none());
    }
}
