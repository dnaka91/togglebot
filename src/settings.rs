//! All configuration and state loading/saving logic.

use std::{collections::HashSet, io::ErrorKind};

use anyhow::Result;
use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Deserialize)]
pub struct Config {
    pub discord: Discord,
}

#[derive(Deserialize)]
pub struct Discord {
    pub token: String,
}

pub async fn load_config() -> Result<Config> {
    let config = fs::read("/app/config.toml").await;
    let config = match config {
        Ok(c) => c,
        Err(_) => fs::read("config.toml").await?,
    };

    toml::from_slice(&config).map_err(Into::into)
}

#[derive(Serialize, Deserialize)]
pub struct State {
    pub schedule: BaseSchedule,
    pub off_days: HashSet<Weekday>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            schedule: BaseSchedule::default(),
            off_days: [Weekday::Sat, Weekday::Sun].iter().copied().collect(),
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
    pub fn format(&self) -> String {
        format!(
            "starting around **{}**, finishing around **{}**",
            Self::format_range(self.start),
            Self::format_range(self.finish)
        )
    }

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

pub async fn load_state() -> Result<State> {
    let state = match fs::read("state.json").await {
        Ok(buf) => buf,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(State::default()),
        Err(e) => return Err(e.into()),
    };

    serde_json::from_slice(&state).map_err(Into::into)
}

pub async fn save_state(state: &State) -> Result<()> {
    let json = serde_json::to_vec_pretty(state)?;

    fs::write("~temp-state.json", &json).await?;
    fs::rename("~temp-state.json", "state.json").await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use maplit::hashset;
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
            "off_days": ["Sat", "Sun"]
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
            off_days: hashset![Weekday::Mon],
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
            "off_days": ["Mon"]
        }};

        assert_eq!(expect, output);
    }

    #[test]
    fn format() {
        let schedule = BaseSchedule {
            start: (NaiveTime::from_hms(8, 0, 0), NaiveTime::from_hms(9, 0, 0)),
            finish: (NaiveTime::from_hms(16, 0, 0), NaiveTime::from_hms(16, 0, 0)),
        };

        assert_eq!(
            "starting around **08:00~09:00am**, finishing around **04:00pm**",
            schedule.format()
        );
    }
}
