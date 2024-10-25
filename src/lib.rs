//! This is the `ToggleBot` bot used on [togglebit](https://github.com/togglebyte)'s
//! [Discord](https://discord.gg/qtyDMat) server and [Twitch](https://twitch.tv/togglebit) chat.

#![deny(missing_docs, rust_2018_idioms, clippy::all, clippy::pedantic)]
#![allow(clippy::missing_errors_doc, missing_docs)]

pub mod api;
mod dirs;
pub mod discord;
pub mod emojis;
pub mod handler;
pub mod settings;
pub mod state;
pub mod statistics;
mod textparse;
pub mod twitch;
