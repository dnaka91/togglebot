//! All configuration loading/saving logic.

use std::{
    collections::{HashMap, HashSet},
    num::NonZeroU64,
    sync::Arc,
};

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::level_filters::LevelFilter;

use crate::dirs::DIRS;

/// Main structure holding all the configuration values.
#[derive(Deserialize)]
pub struct Config {
    /// Discord related settings.
    pub discord: Discord,
    /// Twitch related settings.
    pub twitch: Twitch,
    /// Settings for built-in commands.
    pub commands: Commands,
    /// Tracing related settings.
    #[serde(default)]
    pub tracing: Tracing,
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

/// Configuration for built-int commands.
#[cfg_attr(test, derive(Default))]
#[derive(Deserialize)]
pub struct Commands {
    /// Name of the streamer this bot runs for.
    pub streamer: String,
    /// List of social links for the `link` command.
    pub links: Arc<HashMap<String, String>>,
}

/// Configuration for tracing related features, like exporting trace spans to an external instance
/// for better visualization.
#[derive(Default, Deserialize)]
pub struct Tracing {
    /// Tracing level configuration.
    #[serde(default)]
    pub levels: Levels,
    /// Logging details for **stdout**.
    #[serde(default)]
    pub logging: Option<Logging>,
    /// Connection details for **Archer** collectors.
    #[serde(default)]
    pub archer: Option<Archer>,
}

/// Configuration for different logging levels of various targets.
#[derive(Deserialize)]
pub struct Levels {
    /// Default level applied to all targets.
    #[serde(
        default = "default_levels_default",
        deserialize_with = "de::level_filter"
    )]
    pub default: LevelFilter,
    /// This bot's level.
    #[serde(
        default = "default_levels_togglebot",
        deserialize_with = "de::level_filter"
    )]
    pub togglebot: LevelFilter,
    /// Additional pairs of arbitrary targets and levels.
    #[serde(
        default = "default_levels_targets",
        deserialize_with = "de::hashmap_level_filter",
        flatten
    )]
    pub targets: HashMap<String, LevelFilter>,
}

impl Default for Levels {
    fn default() -> Self {
        Self {
            default: default_levels_default(),
            togglebot: default_levels_togglebot(),
            targets: default_levels_targets(),
        }
    }
}

#[inline]
fn default_levels_default() -> LevelFilter {
    LevelFilter::WARN
}

#[inline]
fn default_levels_togglebot() -> LevelFilter {
    LevelFilter::TRACE
}

#[inline]
fn default_levels_targets() -> HashMap<String, LevelFilter> {
    [("docsearch".to_owned(), LevelFilter::TRACE)]
        .into_iter()
        .collect()
}

/// Details for logging to stdout.
#[derive(Deserialize)]
pub struct Logging {
    /// Whether to completely disable logging to the standard output.
    #[serde(default)]
    pub style: LogStyle,
}

/// Log style defines how logs are formatted.
#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogStyle {
    /// Normal logging style.
    #[default]
    Default,
    /// More compact variant.
    Compact,
    /// Verbose bug pretty variant.
    Pretty,
}

/// Details to connect and report tracing data to a **Archer** instance, using its custom protocol
/// for communication.
#[derive(Deserialize)]
pub struct Archer {
    /// Socket address of the server.
    pub address: String,
    /// Server certificate, to verify the connection.
    pub certificate: String,
}

/// Load the global bot configuration.
pub fn load() -> Result<Config> {
    let buf = std::fs::read_to_string(DIRS.config_file()).context("failed reading config file")?;
    toml::from_str(&buf).context("failed parsing settings")
}

mod de {
    use std::{borrow::Cow, collections::HashMap, fmt, hash::Hash, marker::PhantomData};

    use serde::de::{self, DeserializeOwned, Deserializer, Visitor};
    use tracing::level_filters::LevelFilter;

    pub fn level_filter<'de, D>(deserializer: D) -> Result<LevelFilter, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(LevelFilterVisitor)
    }

    struct LevelFilterVisitor;

    impl<'de> Visitor<'de> for LevelFilterVisitor {
        type Value = LevelFilter;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("tracing level filter")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            v.parse().map_err(E::custom)
        }
    }

    pub fn hashmap_level_filter<'de, D, K>(
        deserializer: D,
    ) -> Result<HashMap<K, LevelFilter>, D::Error>
    where
        D: Deserializer<'de>,
        K: DeserializeOwned + Eq + Hash,
    {
        deserializer.deserialize_map(HashMapLevelFilterVisitor { key: PhantomData })
    }

    struct HashMapLevelFilterVisitor<K> {
        key: PhantomData<K>,
    }

    impl<'de, K> Visitor<'de> for HashMapLevelFilterVisitor<K>
    where
        K: DeserializeOwned + Eq + Hash,
    {
        type Value = HashMap<K, LevelFilter>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("map from tracing targets to tracing level filters")
        }

        fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            let mut map = HashMap::with_capacity(access.size_hint().unwrap_or(0));

            while let Some((key, value)) = access.next_entry::<K, Cow<'_, str>>()? {
                let value = value.parse().map_err(de::Error::custom)?;
                map.insert(key, value);
            }

            Ok(map)
        }
    }
}
