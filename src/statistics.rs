//! Statistics maangement for the bot.

use std::{collections::BTreeMap, io::ErrorKind};

use anyhow::{Context, Result};
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
}

impl Stats {
    /// Increment the usage counter for the given command by one.
    pub fn increment(&mut self, cmd: Command<'_>) {
        let month = OffsetDateTime::now_utc().month();
        if self.current.0 != month {
            self.current.0 = month;
            self.current.1 = Statistics::default();
        }

        for stats in [&mut self.current.1, &mut self.total] {
            *stats.command_usage.get_counter(cmd) += 1;
        }
    }

    /// Shorthand to increment the usage counter of a built-in command.
    pub fn increment_builtin(&mut self, cmd: BuiltinCommand) {
        self.increment(Command::Builtin(cmd));
    }

    /// Get the current or total statistics.
    #[must_use]
    pub const fn get(&self, total: bool) -> &Statistics {
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
            stats.command_usage.custom.remove(name);
        }
    }
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            current: (Month::January, Statistics::default()),
            total: Statistics::default(),
        }
    }
}

/// Statistics for various details about `togglebot` (well, currently only command usage counters).
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Statistics {
    /// Usage counters for commands.
    #[serde(default)]
    pub command_usage: CommandUsage,
}

/// Counters for all available **user** commands. These are split between builtin, custom and
/// unknown to allow better visualization and categorization.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct CommandUsage {
    /// Standard, built-in commands. Helps to find out which built in commands might be removed
    /// in the future due to low usage.
    pub builtin: BTreeMap<BuiltinCommand, u64>,
    /// Custom defined commands. Allows admins to see what commands might be retired.
    pub custom: BTreeMap<String, u64>,
    /// Unrecognized commands. Can give insight about common misspells or wished-for commands.
    pub unknown: BTreeMap<String, u64>,
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

/// One of the few pre-defined commands that are always available.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum BuiltinCommand {
    /// Info about the bots.
    Help,
    /// List of available commands.
    Commands,
    /// Several social media links.
    Links,
    /// Current stream schedule.
    Schedule,
    /// Rust crate info lookup.
    Crate,
    /// Rust crate docs lookup.
    Doc,
    /// Fake ban for fun.
    Ban,
    /// Any other command that may have existed in the past.
    ///
    /// This uses the `#[serde(other)]` configuration, so that commands can be deleted and then
    /// captured by this variant during deserialization. It allows to clean up the file system
    /// copy by doing a _deserialization -> serialization_ pass through [`serde`].
    #[serde(other)]
    Deprecated,
}

impl BuiltinCommand {
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Help => "help",
            Self::Commands => "commands",
            Self::Links => "links",
            Self::Schedule => "schedule",
            Self::Crate => "crate",
            Self::Doc => "doc",
            Self::Ban => "ban",
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
