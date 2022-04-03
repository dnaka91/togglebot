//! All configuration loading/saving logic.

use std::{collections::HashSet, num::NonZeroU64};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::dirs::DIRS;

/// Main structure holding all the configuration values.
#[derive(Deserialize)]
pub struct Config {
    /// Discord related settings.
    pub discord: Discord,
    /// Twitch related settings.
    pub twitch: Twitch,
}

/// Information required to connect to Discord and additional data.
#[derive(Deserialize)]
pub struct Discord {
    /// Bot authentication token.
    pub token: String,
    /// List of owner IDs.
    pub owners: HashSet<NonZeroU64>,
}

/// Information required to connect to Twitch and additional data.
#[derive(Deserialize)]
pub struct Twitch {
    /// Username for login.
    pub login: String,
    /// Token for authentication.
    pub token: String,
}

/// Load the global bot configuration.
pub fn load() -> Result<Config> {
    let buf = std::fs::read(DIRS.config_file()).context("failed reading config file")?;
    toml::from_slice(&buf).context("failed parsing settings")
}
