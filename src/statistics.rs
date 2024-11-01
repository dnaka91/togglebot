//! Statistics management for the bot.

use std::{hash::Hash, sync::Arc};

use anyhow::Result;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tracing::error;

pub use self::migrate::run as migrate;
use crate::db::{self, connection::Connection};

/// Main structure that hold the statistics for different time frames.
pub struct Stats(Arc<Connection>);

impl Stats {
    pub fn new(conn: Connection) -> Self {
        Self(conn.into())
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Self> {
        Connection::in_memory().map(Arc::new).map(Self)
    }

    /// Increment the usage counter for the given command by one.
    pub fn increment(&self, cmd: Command<'_>) -> Result<()> {
        // Don't track commands that are too long.
        if cmd.str_len() > 50 {
            return Ok(());
        }

        let now = OffsetDateTime::now_utc();

        let (kind, name) = match cmd {
            Command::Builtin(cmd) => (CommandKind::Builtin, cmd.name()),
            Command::Custom(cmd) => (CommandKind::Custom, cmd),
            Command::Unknown(cmd) => (CommandKind::Unknown, cmd),
        };

        db::exec(
            &self.0,
            include_str!("../queries/cmd_usage/increment.sql"),
            (now.year(), u8::from(now.month()), kind, name),
        )?;

        Ok(())
    }

    /// Shorthand to increment the usage count, but log an error instead of returning it.
    pub fn try_increment(&self, cmd: Command<'_>) {
        if let Err(e) = self.increment(cmd) {
            error!(error = ?e, ?cmd, "failed incrementing statistics");
        }
    }

    /// Get the current or total statistics.
    pub fn get(&self, total: bool) -> Result<Statistics> {
        let now = OffsetDateTime::now_utc();

        let stats = if total {
            db::query_vec::<_, Statistic>(
                &self.0,
                include_str!("../queries/cmd_usage/list_total.sql"),
                db::NO_PARAMS,
            )
        } else {
            db::query_vec::<_, Statistic>(
                &self.0,
                include_str!("../queries/cmd_usage/list_current.sql"),
                (now.year(), u8::from(now.month())),
            )
        }?;

        Ok(stats
            .into_iter()
            .fold(Statistics::default(), |mut acc, stat| {
                match stat.kind {
                    CommandKind::Builtin => {
                        if let Some(cmd) = BuiltinCommand::from_str(&stat.name) {
                            acc.command_usage.builtin.insert(cmd, stat.count);
                        }
                    }
                    CommandKind::Custom => {
                        acc.command_usage.custom.insert(stat.name, stat.count);
                    }
                    CommandKind::Unknown => {
                        acc.command_usage.unknown.insert(stat.name, stat.count);
                    }
                };
                acc
            }))
    }

    /// Erase the usage counter for a custom command. This is usually done when a custom command
    /// is deleted.
    pub fn erase_custom(&self, name: &str) -> Result<()> {
        db::exec(
            &self.0,
            include_str!("../queries/cmd_usage/delete.sql"),
            name,
        )?;

        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
struct Statistic {
    kind: CommandKind,
    name: String,
    count: u64,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum CommandKind {
    Builtin,
    Custom,
    Unknown,
}

/// Statistics for various details about `togglebot` (well, currently only command usage counters).
#[derive(Default)]
#[cfg_attr(test, derive(Debug))]
pub struct Statistics {
    /// Usage counters for commands.
    pub command_usage: CommandUsage,
}

/// Counters for all available **user** commands. These are split between builtin, custom and
/// unknown to allow better visualization and categorization.
#[derive(Default)]
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

impl From<BuiltinCommand> for Command<'_> {
    fn from(value: BuiltinCommand) -> Self {
        Self::Builtin(value)
    }
}

/// One of the few pre-defined commands that are always available.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub enum BuiltinCommand {
    /// Info about the bots.
    Help,
    /// List of available commands.
    Commands,
    /// Several social media links.
    Links,
    /// Rust crate info lookup.
    Crate,
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
            Self::Ban => "ban",
            Self::Today => "today",
            Self::FahrenheitToCelsius => "ftoc",
            Self::CelsiusToFahrenheit => "ctof",
            Self::Deprecated => "deprecated",
        }
    }

    #[must_use]
    fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "help" => Self::Help,
            "commands" => Self::Commands,
            "links" => Self::Links,
            "crate" => Self::Crate,
            "ban" => Self::Ban,
            "today" => Self::Today,
            "ftoc" => Self::FahrenheitToCelsius,
            "ctof" => Self::CelsiusToFahrenheit,
            "deprecated" => Self::Deprecated,
            _ => return None,
        })
    }
}

mod migrate {
    use std::{fs, io::ErrorKind};

    use anyhow::{Context, Result};
    use indexmap::IndexMap;
    use serde::Deserialize;
    use time::{Month, OffsetDateTime};

    use super::Connection;
    use crate::dirs::DIRS;

    #[derive(Deserialize)]
    struct Stats {
        current: (Month, Statistics),
        total: Statistics,
    }

    #[derive(Deserialize)]
    struct Statistics {
        #[serde(default)]
        command_usage: CommandUsage,
    }

    #[derive(Default, Deserialize)]
    struct CommandUsage {
        builtin: IndexMap<BuiltinCommand, u64>,
        custom: IndexMap<String, u64>,
        unknown: IndexMap<String, u64>,
    }

    #[derive(Eq, Hash, PartialEq, Deserialize)]
    pub enum BuiltinCommand {
        Help,
        Commands,
        Links,
        Crate,
        Ban,
        Today,
        FahrenheitToCelsius,
        CelsiusToFahrenheit,
        #[serde(other)]
        Deprecated,
    }

    impl AsRef<str> for BuiltinCommand {
        fn as_ref(&self) -> &str {
            match self {
                Self::Help => "help",
                Self::Commands => "commands",
                Self::Links => "links",
                Self::Crate => "crate",
                Self::Ban => "ban",
                Self::Today => "today",
                Self::FahrenheitToCelsius => "ftoc",
                Self::CelsiusToFahrenheit => "ctof",
                Self::Deprecated => "deprecated",
            }
        }
    }

    fn load() -> Result<Option<Stats>> {
        let state = match fs::read(DIRS.statistics_file()) {
            Ok(buf) => buf,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e).context("failed reading statistics file"),
        };

        serde_json::from_slice(&state)
            .context("failed parsing statistics data")
            .map(Some)
    }

    fn transform_map<'a, T: AsRef<str> + 'a>(
        kind: super::CommandKind,
        map: impl IntoIterator<Item = (&'a T, &'a u64)>,
    ) -> impl Iterator<Item = (super::CommandKind, &'a str, u64)> {
        map.into_iter().map(move |(k, &v)| (kind, k.as_ref(), v))
    }

    pub fn run(conn: &mut Connection) -> Result<()> {
        let Some(stats) = load()? else { return Ok(()) };

        let tx = conn.transaction()?;
        let mut stmt = tx.prepare(include_str!("../queries/cmd_usage/add.sql"))?;

        let stats = [
            (
                OffsetDateTime::now_utc().year(),
                stats.current.0,
                stats.current.1,
            ),
            (0, Month::January, stats.total),
        ];

        for (year, month, stats) in stats {
            let usages = transform_map(super::CommandKind::Builtin, &stats.command_usage.builtin)
                .chain(transform_map(
                    super::CommandKind::Custom,
                    &stats.command_usage.custom,
                ))
                .chain(transform_map(
                    super::CommandKind::Unknown,
                    &stats.command_usage.unknown,
                ));

            for (kind, name, count) in usages {
                stmt.execute(serde_rusqlite::to_params((
                    year,
                    u8::from(month),
                    kind,
                    name,
                    count,
                ))?)?;
            }
        }

        drop(stmt);
        tx.commit()?;

        fs::remove_file(DIRS.statistics_file())
            .context("failed deleting obsolete statistics file")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use similar_asserts::assert_eq;

    use super::*;

    #[test]
    fn increment() {
        let stats = Stats::in_memory().unwrap();
        for _ in 0..2 {
            stats.increment(BuiltinCommand::Help.into()).unwrap();
        }

        for _ in 0..3 {
            stats.increment(Command::Custom("me")).unwrap();
        }

        for _ in 0..4 {
            stats.increment(Command::Unknown("who")).unwrap();
        }

        let usage = &stats.get(false).unwrap().command_usage;
        assert_eq!(2, usage.builtin[&BuiltinCommand::Help]);
        assert_eq!(3, usage.custom["me"]);
        assert_eq!(4, usage.unknown["who"]);
    }

    #[test]
    fn erase_custom() {
        let stats = Stats::in_memory().unwrap();
        stats.increment(Command::Custom("me")).unwrap();
        stats.increment(Command::Custom("you")).unwrap();
        stats.erase_custom("you").unwrap();

        let usage = &stats.get(false).unwrap().command_usage;
        assert_eq!(1, usage.custom["me"]);
        assert!(usage.custom.get("you").is_none());
    }
}
