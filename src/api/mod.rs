use std::{
    fmt::{self, Display},
    num::NonZeroU64,
};

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use tracing::Span;

use self::{request::Request, response::Response};

pub mod request;
pub mod response;

/// A queue that service connecters can use to send received messages to the handler and get back a
/// reply to render to the user.
pub type Queue = mpsc::Sender<(Message, oneshot::Sender<Response>)>;

/// A message that was received by a service connector. It contains all information needed by the
/// handler to parse and act upon the message.
#[derive(Debug)]
pub struct Message {
    /// Tracing span to keep track of the origin of the message.
    pub span: Span,
    /// Tells what service connector the message came from.
    pub source: Source,
    /// The whole message content.
    pub content: Request,
    /// Whether this message is considered an admin command.
    pub author: AuthorId,
    /// ID of a mentioned user contained in the content. Currently specific to **Discord**.
    pub mention: Option<NonZeroU64>,
}

/// Possible sources that a message came from.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Discord source <https://discord.com>.
    Discord,
    /// Twitch source <https://twitch.tv>.
    Twitch,
}

impl Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Discord => "Discord",
            Self::Twitch => "Twitch",
        })
    }
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

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[cfg_attr(test, derive(PartialEq))]
#[serde(transparent)]
pub struct AdminId(NonZeroU64);

impl AdminId {
    pub fn new(value: u64) -> Option<Self> {
        NonZeroU64::new(value).map(Self)
    }

    #[must_use]
    pub fn get(&self) -> u64 {
        self.0.get()
    }

    #[must_use]
    pub fn from_author(id: &AuthorId) -> Option<Self> {
        match id {
            AuthorId::Discord(id) => Some(Self(*id)),
            AuthorId::Twitch(_) => None,
        }
    }
}

impl Display for AdminId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T> From<T> for AdminId
where
    T: Into<NonZeroU64>,
{
    fn from(value: T) -> Self {
        Self(value.into())
    }
}
