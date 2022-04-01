//! Main handling logic for all supported bot commands.

use std::{num::NonZeroU64, sync::Arc};

use anyhow::{bail, Result};
use tokio::sync::RwLock;

use crate::{
    settings::Config,
    state::State,
    statistics::{BuiltinCommand, Command, Stats},
    AdminResponse, AuthorId, OwnerResponse, Source, UserResponse,
};

mod admin;
mod owner;
mod user;

/// Convenience type alias for a [`State`] wrapped in an [`Arc`] and a [`RwLock`].
pub type AsyncState = Arc<RwLock<State>>;

pub type AsyncStats = Arc<RwLock<Stats>>;

#[derive(Clone, Copy)]
pub enum Access {
    Standard,
    Admin,
    Owner,
}

pub async fn access(config: &Config, state: AsyncState, author: &AuthorId) -> Access {
    match author {
        AuthorId::Discord(id) => {
            if config.discord.owners.contains(id) {
                Access::Owner
            } else if state.read().await.admins.contains(id) {
                Access::Admin
            } else {
                Access::Standard
            }
        }
        AuthorId::Twitch(_) => Access::Standard,
    }
}

/// Handle any user facing message and prepare a response.
#[tracing::instrument(skip_all, name = "user")]
pub async fn user_message(
    state: AsyncState,
    statistics: AsyncStats,
    content: &str,
    source: Source,
) -> Result<UserResponse> {
    let mut parts = content.splitn(2, char::is_whitespace);
    let command = if let Some(cmd) = parts.next() {
        cmd
    } else {
        bail!("got message without content");
    };

    Ok(match (command.to_lowercase().as_ref(), parts.next()) {
        ("!help" | "!bot", None) => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::Help);
            user::help()
        }
        ("!commands", None) => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::Commands);
            user::commands(state, source).await
        }
        ("!links", None) => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::Links);
            user::links(source)
        }
        ("!schedule", None) => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::Schedule);
            user::schedule(state).await
        }
        ("!crate" | "!crates", Some(name)) => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::Crate);
            user::crate_(name).await
        }
        ("!doc" | "!docs", Some(path)) => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::Doc);
            user::doc(path).await
        }
        ("!ban", Some(target)) => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::Ban);
            user::ban(target)
        }
        (name, None) => {
            let response = user::custom(state, source, name).await;

            if name.starts_with('!') {
                let cmd = if matches!(response, UserResponse::Unknown) {
                    Command::Unknown(name)
                } else {
                    Command::Custom(name)
                };
                statistics.write().await.increment(cmd);
            }

            response
        }
        _ => UserResponse::Unknown,
    })
}

/// Handle admin facing messages to control the bot and prepare a response.
#[tracing::instrument(skip_all, name = "admin")]
pub async fn admin_message(
    state: AsyncState,
    statistics: AsyncStats,
    content: &str,
) -> Result<AdminResponse> {
    let mut parts = content.split_whitespace();
    let command = if let Some(cmd) = parts.next() {
        cmd
    } else {
        bail!("got message without content");
    };

    Ok(
        match (
            command.to_lowercase().as_ref(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) {
            ("!admin_help" | "!admin-help" | "!adminhelp" | "!ahelp", None, None, None, None) => {
                admin::help()
            }
            ("!edit_schedule", Some("set"), Some(field), Some(range_begin), Some(range_end)) => {
                admin::schedule(state, field, range_begin, range_end).await
            }
            ("!off_days", Some(action), Some(weekday), None, None) => {
                admin::off_days(state, action, weekday).await
            }
            ("!custom_commands" | "!custom_command", Some("list"), None, None, None) => {
                admin::custom_commands_list(state).await
            }
            ("!custom_commands" | "!custom_command", Some(action), Some(source), Some(name), _) => {
                admin::custom_commands(state, statistics, content, action, source, name).await
            }
            ("!stats", date, None, None, None) => admin::stats(statistics, date).await,
            _ => AdminResponse::Unknown,
        },
    )
}

/// Handle messages only accessible to owners defined in the settings and prepare a response.
#[tracing::instrument(skip_all, name = "owner")]
pub async fn owner_message(
    state: AsyncState,
    content: &str,
    mention: Option<NonZeroU64>,
) -> Result<OwnerResponse> {
    let mut parts = content.splitn(3, char::is_whitespace);
    let command = if let Some(cmd) = parts.next() {
        cmd
    } else {
        bail!("got message without content");
    };

    Ok(
        match (command.to_lowercase().as_ref(), parts.next(), parts.next()) {
            ("!owner_help" | "!owner-help" | "!ownerhelp" | "!ohelp", None, None) => owner::help(),
            ("!admins" | "!admin", Some("list"), None) => owner::admins_list(state).await,
            ("!admins" | "!admin", Some(action), Some(_)) => {
                if let Some(mention) = mention {
                    owner::admins_edit(state, action, mention).await
                } else {
                    OwnerResponse::Unknown
                }
            }
            _ => OwnerResponse::Unknown,
        },
    )
}
