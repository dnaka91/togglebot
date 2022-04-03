//! State management and load/save logic for it.

mod serde;

#[cfg(test)]
use std::{collections::hash_map::DefaultHasher, hash::BuildHasherDefault};
use std::{io::ErrorKind, num::NonZeroU64};

use anyhow::{Context, Result};
use time::{
    format_description::FormatItem,
    macros::{format_description, time},
    Time, Weekday,
};
use tokio::fs;

use self::serde::{Deserialize, Serialize};
use crate::{dirs::DIRS, Source};

#[cfg(not(test))]
type HashSet<T> = std::collections::HashSet<T>;
#[cfg(test)]
type HashSet<T> = std::collections::HashSet<T, BuildHasherDefault<DefaultHasher>>;
#[cfg(not(test))]
type HashMap<K, V> = std::collections::HashMap<K, V>;
#[cfg(test)]
type HashMap<K, V> = std::collections::HashMap<K, V, BuildHasherDefault<DefaultHasher>>;

/// Main state structure holding all dynamic (runtime changeable) settings.
#[derive(Serialize, Deserialize)]
pub struct State {
    /// Information for the regular streaming schedule (for the `schedule` command).
    #[serde(default)]
    pub schedule: BaseSchedule,
    /// List of weekdays that are considered off days (for the `schedule` command).
    #[serde(default, with = "self::serde::weekdays")]
    pub off_days: HashSet<Weekday>,
    /// Collection of all the custom commands this bot knows.
    ///
    /// Each command can be defined multiple times, one for each data source. That allows to have
    /// different formatting for different services (like plain text for Twitter and Markdown for
    /// Discord).
    #[serde(default)]
    pub custom_commands: HashMap<String, HashMap<Source, String>>,
    /// List of user accounts that are considered admins.
    ///
    /// These users get access to the admin commands of the bot, mostly allowing to edit the custom
    /// commands and adjust settings for other builtin commands.
    #[serde(default)]
    pub admins: HashSet<NonZeroU64>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            schedule: BaseSchedule::default(),
            off_days: [Weekday::Saturday, Weekday::Sunday]
                .iter()
                .copied()
                .collect(),
            custom_commands: HashMap::default(),
            admins: HashSet::default(),
        }
    }
}

/// Format definition for schedule times.
pub const SCHEDULE_TIME_FORMAT: &[FormatItem<'static>] =
    format_description!("[hour repr:12]:[minute][period case:lower]");

/// Start and finish time ranges for the streaming schedule.
///
/// The time ranges are defined as two time settings. If they're the same, it is considered a
/// specific time and displayed as a fixed time accordingly. If they differ instead, they're
/// considered an approximate time range.
#[derive(Serialize, Deserialize)]
pub struct BaseSchedule {
    /// Start time range.
    #[serde(with = "self::serde::pair_time_hms")]
    pub start: (Time, Time),
    /// Finish time range.
    #[serde(with = "self::serde::pair_time_hms")]
    pub finish: (Time, Time),
}

impl BaseSchedule {
    /// Format the start time range as string.
    pub fn format_start(&self) -> Result<String> {
        Self::format_range(self.start)
    }

    /// Format the finish time range as string.
    pub fn format_finish(&self) -> Result<String> {
        Self::format_range(self.finish)
    }

    fn format_range(range: (Time, Time)) -> Result<String> {
        Ok(if range.0 == range.1 {
            range.0.format(&SCHEDULE_TIME_FORMAT)?
        } else {
            format!(
                "{}~{}",
                range.0.format(&SCHEDULE_TIME_FORMAT)?,
                range.1.format(&SCHEDULE_TIME_FORMAT)?
            )
        })
    }
}

impl Default for BaseSchedule {
    fn default() -> Self {
        Self {
            start: (time!(07:00:00), time!(08:00:00)),
            finish: (time!(16:00:00), time!(16:00:00)),
        }
    }
}

/// Load the global state (the dynamic runtime settings) of this bot and sanitize the data during
/// the process, if needed.
pub fn load() -> Result<State> {
    let state = match std::fs::read(DIRS.state_file()) {
        Ok(buf) => buf,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(State::default()),
        Err(e) => return Err(e.into()),
    };

    serde_json::from_slice(&state).context("failed parsing state data")
}

/// Syncronize the current in-memory state back to the file system.
pub async fn save(state: &State) -> Result<()> {
    if cfg!(test) {
        return Ok(());
    }

    fs::create_dir_all(DIRS.data_dir()).await?;

    let json = serde_json::to_vec_pretty(state)?;

    fs::write(DIRS.state_temp_file(), &json).await?;
    fs::rename(DIRS.state_temp_file(), DIRS.state_file()).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    #[test]
    fn ser_default() {
        let output = serde_json::to_value(&State::default()).unwrap();
        let expect = json! {{
            "schedule": {
                "start": [
                    "07:00:00",
                    "08:00:00"
                ],
                "finish": [
                    "16:00:00",
                    "16:00:00"
                ]
            },
            "off_days": ["Sat", "Sun"],
            "custom_commands": {},
            "admins": []
        }};

        assert_eq!(expect, output);
    }

    #[test]
    fn ser_custom() {
        let output = serde_json::to_value(&State {
            schedule: BaseSchedule {
                start: (time!(05:30:00), time!(07:20:11)),
                finish: (time!(16:00:00), time!(17:15:20)),
            },
            off_days: [Weekday::Monday].iter().copied().collect(),
            custom_commands: vec![(
                "hello".to_owned(),
                vec![(Source::Discord, "Hello World!".to_owned())]
                    .into_iter()
                    .collect(),
            )]
            .into_iter()
            .collect(),
            admins: [NonZeroU64::new(1).unwrap()].into_iter().collect(),
        })
        .unwrap();
        let expect = json! {{
            "schedule": {
                "start": [
                    "05:30:00",
                    "07:20:11"
                ],
                "finish": [
                    "16:00:00",
                    "17:15:20"
                ]
            },
            "off_days": ["Mon"],
            "custom_commands": {
                "hello": {
                    "Discord": "Hello World!"
                }
            },
            "admins": [1]
        }};

        assert_eq!(expect, output);
    }

    #[test]
    fn parse_schedule_time() {
        let res = Time::parse("08:00am", &SCHEDULE_TIME_FORMAT);
        assert_eq!(Ok(time!(08:00:00)), res);
    }
}
