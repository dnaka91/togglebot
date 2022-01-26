//! This is the `ToggleBot` bot used on [togglebit](https://github.com/togglebyte)'s
//! [Discord](https://discord.gg/qtyDMat) server and [Twitch](https://twitch.tv/togglebit) chat.

#![deny(rust_2018_idioms, clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::missing_errors_doc)]

use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU64,
};

/// Result type used throughout the whole crate.
pub use anyhow::Result;
pub use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
pub use tokio::sync::{
    broadcast::Receiver as BroadcastReceiver, mpsc::Sender as MpscSender,
    oneshot::Sender as OneshotSender,
};

mod dirs;
pub mod discord;
pub mod emojis;
pub mod handler;
pub mod settings;
pub mod twitch;

/// A queue that service connecters can use to send received messages to the handler and get back a
/// reply to render to the user.
pub type Queue = MpscSender<(Message, OneshotSender<Response>)>;
/// Shutdown hook that service connecters use to be notified about a shutdown and shut down all
/// internal machinery.
pub type Shutdown = BroadcastReceiver<()>;

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

#[derive(Debug)]
pub enum AuthorId {
    Discord(NonZeroU64),
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
    Owner(OwnerResponse),
}

pub enum UserResponse {
    /// Command was not recognized and should be ignored.
    Unknown,
    /// Print a help message showing how to use the bot.
    Help,
    /// List all available commands to the user.
    Commands(Result<Vec<String>>),
    /// Show a list of links to various platforms where the streamer is present.
    Links(&'static [(&'static str, &'static str)]),
    Schedule {
        start: String,
        finish: String,
        off_days: Vec<String>,
    },
    Ban(String),
    Crate(Result<CrateSearch>),
    Doc(Result<String>),
    Custom(String),
}

pub enum CrateSearch {
    Found(CrateInfo),
    NotFound(String),
}

#[derive(Deserialize)]
pub struct CrateInfo {
    pub name: String,
    pub updated_at: DateTime<FixedOffset>,
    pub downloads: u64,
    pub newest_version: String,
    pub description: String,
    pub documentation: Option<String>,
    pub repository: String,
}

pub enum AdminResponse {
    /// Command was not recognized and should be ignored.
    Unknown,
    /// Print a help message with all available admin control commands.
    Help,
    Schedule(Result<()>),
    OffDays(Result<()>),
    CustomCommands(CustomCommandsResponse),
}

pub enum CustomCommandsResponse {
    List(Result<BTreeMap<String, BTreeSet<Source>>>),
    Edit(Result<()>),
}

pub enum OwnerResponse {
    Unknown,
    Help,
    Admins(AdminsResponse),
}

pub enum AdminsResponse {
    List(Vec<NonZeroU64>),
    Edit(Result<AdminAction>),
}

pub enum AdminAction {
    Added,
    Removed,
}
