#![deny(rust_2018_idioms, clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::missing_errors_doc)]

pub use anyhow::Result;
pub use tokio::sync::{
    broadcast::Receiver as BroadcastReceiver, mpsc::Sender as MpscSender,
    oneshot::Sender as OneshotSender,
};

pub mod discord;
pub mod emojis;
pub mod settings;

type Queue = MpscSender<(Message, OneshotSender<Response>)>;
type Shutdown = BroadcastReceiver<()>;

pub struct Message {
    pub source: Source,
    pub content: String,
    pub admin: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Discord,
    Twitch,
}

pub enum Response {
    User(UserResponse),
    Admin(AdminResponse),
}

pub enum UserResponse {
    Unknown,
    Help,
    Links(&'static [(&'static str, &'static str)]),
    Schedule {
        start: String,
        finish: String,
        off_days: Vec<String>,
    },
}

pub enum AdminResponse {
    Unknown,
    Help,
    Schedule(Result<()>),
    OffDays(Result<()>),
}
