use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    num::NonZeroU64,
    sync::Arc,
};

use anyhow::Result;
use serde::Deserialize;
use time::OffsetDateTime;

use super::Source;
use crate::statistics::Statistics;

/// The response for a command sent by a user.
pub enum Response {
    /// Response for a normal user command.
    User(User),
    /// Response for an admin command.
    Admin(Admin),
    /// Response for an owner command.
    Owner(Owner),
}

/// Response for a normal user command.
#[cfg_attr(test, derive(Debug))]
pub enum User {
    /// Command was not recognized and should be ignored.
    Unknown,
    /// Print a help message showing how to use the bot.
    Help,
    /// List all available commands to the user.
    Commands(Result<Vec<String>>),
    /// Show a list of links to various platforms where the streamer is present.
    Links(Arc<HashMap<String, String>>),
    /// Fake ban anybody or anything.
    Ban(String),
    /// Lookup details about a single Rust crate.
    Crate(Result<CrateSearch>),
    /// Get the current date, with unneeded level of detail (in UTC).
    Today(String),
    /// Convert Fahrenheit degrees to Celsius degrees.
    FahrenheitToCelsius(String),
    /// Convert Celsius degrees to Fahrenheit degrees.
    CelsiusToFahrenheit(String),
    /// Execute a custom command.
    Custom(String),
}

/// Result of a crate search, either it was found, providing the details, or it wasn't giving some
/// generic reply message (possibly with reason why).
#[cfg_attr(test, derive(Debug))]
pub enum CrateSearch {
    /// Found request crate.
    Found(CrateInfo),
    /// Request crate couldn't be found.
    NotFound(String),
}

/// Information about a single Rust crate.
#[derive(Deserialize)]
#[cfg_attr(test, derive(Debug))]
pub struct CrateInfo {
    /// Name of the crate.
    pub name: String,
    /// Last time a new version was released.
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    /// Total amount of downloads.
    pub downloads: u64,
    /// Version string of the latest version.
    pub newest_version: String,
    /// Crate description.
    pub description: String,
    /// Optional documentation link.
    pub documentation: Option<String>,
    /// Link the the source code repository.
    pub repository: String,
}

/// Response for an admin command.
#[cfg_attr(test, derive(Debug))]
pub enum Admin {
    /// Print a help message with all available admin control commands.
    Help,
    /// Configure custom user commands.
    CustomCommands(CustomCommands),
    /// Show statistics about user commands.
    Statistics(Result<(bool, Statistics)>),
}

/// Response for custom command administration related commands.
#[cfg_attr(test, derive(Debug))]
pub enum CustomCommands {
    /// List the available custom commands, split by service.
    List(Result<BTreeMap<String, BTreeSet<Source>>>),
    /// Add/change/delete custom commands.
    Edit(Result<()>),
}

/// Response for an owner command.
#[cfg_attr(test, derive(Debug))]
pub enum Owner {
    /// Show the help message for owners.
    Help,
    /// Admin users related commands.
    Admins(Admins),
}

/// Response for admin user management commands.
#[cfg_attr(test, derive(Debug))]
pub enum Admins {
    /// List the current admins.
    List(Vec<NonZeroU64>),
    /// Edit the current admin list.
    Edit(Result<AdminAction>),
}

/// Possible actions for admin list edits.
#[cfg_attr(test, derive(Debug))]
pub enum AdminAction {
    /// Account was added to the admin list.
    Added,
    /// Account was removed from the admin list.
    Removed,
}
