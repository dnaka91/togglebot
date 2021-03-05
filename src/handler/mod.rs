//! Main handling logic for all supported bot commands.

use std::sync::Arc;

use anyhow::{bail, Result};
use tokio::sync::RwLock;

use crate::{settings::State, AdminResponse, Message, UserResponse};

mod admin;
mod user;

/// Convenience type alias for a [`State`] wrapped in an [`Arc`] and a [`RwLock`].
pub type AsyncState = Arc<RwLock<State>>;

/// Handle any user facing message and prepare a response.
pub async fn user_message(state: AsyncState, message: Message) -> Result<UserResponse> {
    let mut parts = message.content.splitn(2, char::is_whitespace);
    let command = if let Some(cmd) = parts.next() {
        cmd
    } else {
        bail!("got message without content");
    };

    Ok(match (command.to_lowercase().as_ref(), parts.next()) {
        ("!help", None) | ("!bot", None) => user::help(),
        ("!commands", None) => user::commands(state, message.source).await,
        ("!links", None) => user::links(message.source),
        ("!schedule", None) => user::schedule(state).await,
        ("!ban", Some(target)) => user::ban(target),
        (name, None) => user::custom(state, message.source, name).await,
        _ => UserResponse::Unknown,
    })
}

/// Handle admin facing messages to control the bot and prepare a response.
pub async fn admin_message(state: AsyncState, content: String) -> Result<AdminResponse> {
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
            ("!help", None, None, None, None) => admin::help(),
            ("!schedule", Some("set"), Some(field), Some(range_begin), Some(range_end)) => {
                admin::schedule(state, field, range_begin, range_end).await
            }
            ("!off_days", Some(action), Some(weekday), None, None) => {
                admin::off_days(state, action, weekday).await
            }
            ("!custom_commands", Some("list"), None, None, None) => {
                admin::custom_commands_list(state).await
            }
            ("!custom_commands", Some(action), Some(source), Some(name), _) => {
                admin::custom_commands(state, &content, action, source, name).await
            }
            _ => AdminResponse::Unknown,
        },
    )
}
