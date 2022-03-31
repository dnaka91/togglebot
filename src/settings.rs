//! All configuration and state loading/saving logic.

use std::{collections::HashSet, num::NonZeroU64};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::dirs::DIRS;

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
    let buf = std::fs::read(DIRS.config_file()).context("failed reading config file")?;
    toml::from_slice(&buf).context("failed parsing settings")
}
