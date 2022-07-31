//! This is the `ToggleBot` bot used on [togglebit](https://github.com/togglebyte)'s
//! [Discord](https://discord.gg/qtyDMat) server and [Twitch](https://twitch.tv/togglebit) chat.

#![deny(missing_docs, rust_2018_idioms, clippy::all, clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    num::NonZeroU64,
    sync::Arc,
};

/// Result type used throughout the whole crate.
pub use anyhow::Result;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
pub use tokio::sync::{
    broadcast::Receiver as BroadcastReceiver, mpsc::Sender as MpscSender,
    oneshot::Sender as OneshotSender,
};

use crate::statistics::Statistics;

mod dirs;
pub mod discord;
pub mod emojis;
pub mod handler;
pub mod settings;
pub mod state;
pub mod statistics;
pub mod twitch;

/// A queue that service connecters can use to send received messages to the handler and get back a
/// reply to render to the user.
pub type Queue = MpscSender<(Message, OneshotSender<Response>)>;

/// A message that was received by a service connector. It contains all information needed by the
/// handler to parse and act upon the message.
#[derive(Debug)]
pub struct Message {
    /// Tells what service connector the message came from.
    pub source: Source,
    /// The whole message content.
    pub content: String,
    /// Whether this message is considered an admin command.
    pub author: AuthorId,
    /// ID of a mentioned user contained in the content. Currently specific to **Discord**.
    pub mention: Option<NonZeroU64>,
}

/// Possible sources that a message came from.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
pub enum Source {
    /// Discord source <https://discord.com>.
    Discord,
    /// Twitch source <https://twitch.tv>.
    Twitch,
}

/// Unique identifier of the message author, one variant for each service the message might come
/// from.
#[derive(Debug)]
pub enum AuthorId {
    /// Discord author ID.
    Discord(NonZeroU64),
    /// Twitch author ID.
    Twitch(String),
}

impl AsRef<str> for Source {
    fn as_ref(&self) -> &str {
        match self {
            Self::Discord => "Discord",
            Self::Twitch => "Twitch",
        }
    }
}

/// The response for a command sent by a user.
pub enum Response {
    /// Response for a normal user command.
    User(UserResponse),
    /// Response for an admin command.
    Admin(AdminResponse),
    /// Response for an owner command.
    Owner(OwnerResponse),
}

/// Response for a normal user command.
#[cfg_attr(test, derive(Debug))]
pub enum UserResponse {
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
    /// Get a direct docs link to any Rust crate or stdlib item.
    Doc(Result<String>),
    /// Get the current date, with unneeded level of detail (in UTC).
    Today(String),
    /// Encrypt a message with top-notch cryptography.
    Encipher(String),
    /// Decrypt a message previously encrypted with top-notch cryptography.
    Decipher(String),
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
pub enum AdminResponse {
    /// Command was not recognized and should be ignored.
    Unknown,
    /// Print a help message with all available admin control commands.
    Help,
    /// Configure custom user commands.
    CustomCommands(CustomCommandsResponse),
    /// Show statistics about user commands.
    Statistics(Result<(bool, Statistics)>),
}

/// Response for custom command administration related commands.
#[cfg_attr(test, derive(Debug))]
pub enum CustomCommandsResponse {
    /// List the available custom commands, split by service.
    List(Result<BTreeMap<String, BTreeSet<Source>>>),
    /// Add/change/delete custom commands.
    Edit(Result<()>),
}

/// Response for an owner command.
#[cfg_attr(test, derive(Debug))]
pub enum OwnerResponse {
    /// Unrecognized command.
    Unknown,
    /// Show the help message for owners.
    Help,
    /// Admin users related commands.
    Admins(AdminsResponse),
}

/// Response for admin user management commands.
#[cfg_attr(test, derive(Debug))]
pub enum AdminsResponse {
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
