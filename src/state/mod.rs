//! State management and load/save logic for it.

#[cfg(test)]
use std::{collections::hash_map::DefaultHasher, hash::BuildHasherDefault};
use std::{io::ErrorKind, num::NonZeroU64};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::fs;

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
#[derive(Default, Serialize, Deserialize)]
pub struct State {
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

/// Synchronize the current in-memory state back to the file system.
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
            "custom_commands": {},
            "admins": []
        }};

        assert_eq!(expect, output);
    }

    #[test]
    fn ser_custom() {
        let output = serde_json::to_value(&State {
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
