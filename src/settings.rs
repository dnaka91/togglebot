//! All configuration and state loading/saving logic.

#[cfg(test)]
use std::{collections::hash_map::DefaultHasher, hash::BuildHasherDefault};
use std::{io::ErrorKind, num::NonZeroU64};

use anyhow::{bail, Result};
use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::Source;

#[cfg(not(test))]
type HashSet<T> = std::collections::HashSet<T>;
#[cfg(test)]
type HashSet<T> = std::collections::HashSet<T, BuildHasherDefault<DefaultHasher>>;
#[cfg(not(test))]
type HashMap<K, V> = std::collections::HashMap<K, V>;
#[cfg(test)]
type HashMap<K, V> = std::collections::HashMap<K, V, BuildHasherDefault<DefaultHasher>>;

#[derive(Deserialize)]
pub struct Config {
    pub discord: Discord,
    pub twitch: Twitch,
}

#[derive(Deserialize)]
pub struct Discord {
    pub token: String,
    pub owners: HashSet<NonZeroU64>,
}

#[derive(Deserialize)]
pub struct Twitch {
    pub login: String,
    pub token: String,
}

pub fn load_config() -> Result<Config> {
    let locations = &[
        concat!("/etc/", env!("CARGO_PKG_NAME"), "/config.toml"),
        concat!("/app/", env!("CARGO_PKG_NAME"), ".toml"),
        concat!(env!("CARGO_PKG_NAME"), ".toml"),
    ];
    let buf = locations.iter().find_map(|loc| std::fs::read(loc).ok());

    match buf {
        Some(buf) => Ok(toml::from_slice(&buf)?),
        None => bail!("failed finding settings"),
    }
}

#[derive(Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub schedule: BaseSchedule,
    #[serde(default)]
    pub off_days: HashSet<Weekday>,
    #[serde(default)]
    pub custom_commands: HashMap<String, HashMap<Source, String>>,
    #[serde(default)]
    pub admins: HashSet<NonZeroU64>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            schedule: BaseSchedule::default(),
            off_days: [Weekday::Sat, Weekday::Sun].iter().copied().collect(),
            custom_commands: HashMap::default(),
            admins: HashSet::default(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct BaseSchedule {
    pub start: (NaiveTime, NaiveTime),
    pub finish: (NaiveTime, NaiveTime),
}

impl BaseSchedule {
    #[must_use]
    pub fn format_start(&self) -> String {
        Self::format_range(self.start)
    }

    #[must_use]
    pub fn format_finish(&self) -> String {
        Self::format_range(self.finish)
    }

    fn format_range(range: (NaiveTime, NaiveTime)) -> String {
        if range.0 == range.1 {
            range.0.format("%I:%M%P").to_string()
        } else {
            format!("{}~{}", range.0.format("%I:%M"), range.1.format("%I:%M%P"))
        }
    }
}

impl Default for BaseSchedule {
    fn default() -> Self {
        Self {
            start: (NaiveTime::from_hms(7, 0, 0), NaiveTime::from_hms(8, 0, 0)),
            finish: (NaiveTime::from_hms(16, 0, 0), NaiveTime::from_hms(16, 0, 0)),
        }
    }
}

const STATE_FILE: &str = concat!("/var/lib/", env!("CARGO_PKG_NAME"), "/state.json");

pub fn load_state() -> Result<State> {
    let state = match std::fs::read(STATE_FILE) {
        Ok(buf) => buf,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(State::default()),
        Err(e) => return Err(e.into()),
    };

    serde_json::from_slice(&state).map_err(Into::into)
}

pub async fn save_state(state: &State) -> Result<()> {
    const STATE_DIR: &str = concat!("/var/lib/", env!("CARGO_PKG_NAME"));
    const TEMP_FILE: &str = concat!("/var/lib/", env!("CARGO_PKG_NAME"), "/~temp-state.json");

    fs::create_dir_all(STATE_DIR).await?;

    let json = serde_json::to_vec_pretty(state)?;

    fs::write(TEMP_FILE, &json).await?;
    fs::rename(TEMP_FILE, STATE_FILE).await?;

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
                start: (
                    NaiveTime::from_hms(5, 30, 0),
                    NaiveTime::from_hms(7, 20, 11),
                ),
                finish: (
                    NaiveTime::from_hms(16, 0, 0),
                    NaiveTime::from_hms(17, 15, 20),
                ),
            },
            off_days: [Weekday::Mon].iter().copied().collect(),
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
}
