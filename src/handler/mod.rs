//! Main handling logic for all supported bot commands.

use std::{num::NonZeroU64, sync::Arc};

use anyhow::{bail, Result};
use tokio::sync::RwLock;
use tracing::Span;

use crate::{
    settings::{Commands as CommandSettings, Discord as DiscordSettings},
    state::State,
    statistics::{BuiltinCommand, Command, Stats},
    AdminResponse, AuthorId, OwnerResponse, Source, UserResponse,
};

mod admin;
mod owner;
mod user;

/// Convenience type alias for a [`State`] wrapped in an [`Arc`] and a [`RwLock`].
pub type AsyncState = Arc<RwLock<State>>;
/// Convenience type alias for [`Stats`] wrapped in an [`Arc`] and a [`RwLock`].
pub type AsyncStats = Arc<RwLock<Stats>>;
/// Convenience type alias for [`settings::Commands`] wrapped in an [`Arc`].
pub type AsyncCommandSettings = Arc<CommandSettings>;

/// Possible access levels for users, controlling access over accessible bot commands.
#[derive(Clone, Copy)]
pub enum Access {
    /// Default user level, only granting access to the user commands.
    Standard,
    /// Admin user level, allowing access to admin and user commands.
    ///
    /// The admin commands include management of settings for all builtin commands and custom
    /// commands.
    Admin,
    /// Owner user level, allowwing access to all commands (owner, admin and user).
    ///
    /// The owner commands give control over the admin user list.
    Owner,
}

/// Determine the access level for the author of a chat message.
///
/// - In **Discord** all possible access levels exist, owners defined in a pre-defined static list
///   and admins defined in a dynamic list controlled by owners at runtime.
/// - In **Twitch** only standard users exist, regardless of any settings.
pub async fn access(settings: &DiscordSettings, state: AsyncState, author: &AuthorId) -> Access {
    match author {
        AuthorId::Discord(id) => {
            if settings.owners.contains(id) {
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
#[tracing::instrument(parent = span, skip_all, name = "user")]
pub async fn user_message(
    span: Span,
    settings: AsyncCommandSettings,
    state: AsyncState,
    statistics: AsyncStats,
    content: &str,
    source: Source,
) -> Result<UserResponse> {
    let mut parts = content.splitn(2, char::is_whitespace);
    let command = if let Some(cmd) = parts.next() {
        match cmd.strip_prefix('!') {
            Some(cmd) => cmd,
            None => return Ok(UserResponse::Unknown),
        }
    } else {
        bail!("got message without content");
    };

    Ok(match (command.to_lowercase().as_ref(), parts.next()) {
        ("help" | "bot", None) => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::Help);
            user::help()
        }
        ("commands", None) => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::Commands);
            user::commands(state, source).await
        }
        ("links", None) => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::Links);
            user::links(&settings)
        }
        ("crate" | "crates", Some(name)) => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::Crate);
            user::crate_(name).await
        }
        ("doc" | "docs", Some(path)) => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::Doc);
            user::doc(path).await
        }
        ("ban", Some(target)) => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::Ban);
            user::ban(target)
        }
        ("today", None) => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::Today);
            user::today()
        }
        (name, None) => {
            let response = user::custom(state, source, name).await;

            let name = if matches!(response, UserResponse::Unknown) {
                Command::Unknown(name)
            } else {
                Command::Custom(name)
            };
            statistics.write().await.increment(name);

            response
        }
        _ => UserResponse::Unknown,
    })
}

/// Handle admin facing messages to control the bot and prepare a response.
#[tracing::instrument(parent = span, skip_all, name = "admin")]
pub async fn admin_message(
    span: Span,
    state: AsyncState,
    statistics: AsyncStats,
    content: &str,
) -> Result<AdminResponse> {
    let mut parts = content.split_whitespace();
    let command = if let Some(cmd) = parts.next() {
        match cmd.strip_prefix('!') {
            Some(cmd) => cmd,
            None => return Ok(AdminResponse::Unknown),
        }
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
            ("admin_help" | "admin-help" | "adminhelp" | "ahelp", None, None, None, None) => {
                admin::help()
            }
            ("custom_commands" | "custom_command", Some("list"), None, None, None) => {
                admin::custom_commands_list(state).await
            }
            ("custom_commands" | "custom_command", Some(action), Some(source), Some(name), _) => {
                admin::custom_commands(state, statistics, content, action, source, name).await
            }
            ("stats", date, None, None, None) => admin::stats(statistics, date).await,
            _ => AdminResponse::Unknown,
        },
    )
}

/// Handle messages only accessible to owners defined in the settings and prepare a response.
#[tracing::instrument(parent = span, skip_all, name = "owner")]
pub async fn owner_message(
    span: Span,
    state: AsyncState,
    content: &str,
    mention: Option<NonZeroU64>,
) -> Result<OwnerResponse> {
    let mut parts = content.splitn(3, char::is_whitespace);
    let command = if let Some(cmd) = parts.next() {
        match cmd.strip_prefix('!') {
            Some(cmd) => cmd,
            None => return Ok(OwnerResponse::Unknown),
        }
    } else {
        bail!("got message without content");
    };

    Ok(
        match (command.to_lowercase().as_ref(), parts.next(), parts.next()) {
            ("owner_help" | "owner-help" | "ownerhelp" | "ohelp", None, None) => owner::help(),
            ("admins" | "admin", Some("list"), None) => owner::admins_list(state).await,
            ("admins" | "admin", Some(action), Some(_)) => {
                owner::admins_edit(state, action, mention).await
            }
            _ => OwnerResponse::Unknown,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdminAction, AdminsResponse, CustomCommandsResponse};

    fn defaults() -> (AsyncCommandSettings, AsyncState, AsyncStats, Source) {
        (
            Arc::new(CommandSettings::default()),
            Arc::new(RwLock::new(State::default())),
            Arc::new(RwLock::new(Stats::default())),
            Source::Discord,
        )
    }

    async fn run_user_message(content: &str) -> Result<UserResponse> {
        tracing_subscriber::fmt::try_init().ok();
        let (settings, state, statistics, source) = defaults();
        user_message(
            Span::current(),
            settings,
            state,
            statistics,
            content,
            source,
        )
        .await
    }

    async fn run_admin_message(content: &str) -> Result<AdminResponse> {
        tracing_subscriber::fmt::try_init().ok();
        let (_, state, statistics, _) = defaults();
        admin_message(Span::current(), state, statistics, content).await
    }

    async fn run_owner_message(
        content: &str,
        mention: Option<NonZeroU64>,
    ) -> Result<OwnerResponse> {
        tracing_subscriber::fmt::try_init().ok();
        let (_, state, _, _) = defaults();
        owner_message(Span::current(), state, content, mention).await
    }

    #[tokio::test]
    async fn user_cmd_unknown() {
        assert!(matches!(
            run_user_message("!kaboom").await,
            Ok(UserResponse::Unknown)
        ));
    }

    #[tokio::test]
    async fn user_cmd_help() {
        assert!(matches!(
            run_user_message("!help").await,
            Ok(UserResponse::Help)
        ));
    }

    #[tokio::test]
    async fn user_cmd_commands() {
        match run_user_message("!commands").await.unwrap() {
            UserResponse::Commands(Ok(cmds)) => assert!(cmds.is_empty()),
            UserResponse::Commands(Err(e)) => panic!("{e:?}"),
            res => panic!("unexpected response: {res:?}"),
        }
    }

    #[tokio::test]
    async fn user_cmd_links() {
        assert!(matches!(
            run_user_message("!links").await,
            Ok(UserResponse::Links(_))
        ));
    }

    #[tokio::test]
    async fn user_cmd_ban() {
        match run_user_message("!ban me").await.unwrap() {
            UserResponse::Ban(target) => assert_eq!("me", target),
            res => panic!("unexpected response: {res:?}"),
        }
    }

    #[tokio::test]
    async fn user_cmd_crate() {
        match run_user_message("!crate anyhow").await.unwrap() {
            UserResponse::Crate(Ok(_)) => {}
            UserResponse::Crate(Err(e)) => panic!("{e:?}"),
            res => panic!("unexpected response: {res:?}"),
        }
    }

    #[tokio::test]
    async fn user_cmd_doc() {
        match run_user_message("!doc anyhow").await.unwrap() {
            UserResponse::Doc(Ok(_)) => {}
            UserResponse::Doc(Err(e)) => panic!("{e:?}"),
            res => panic!("unexpected response: {res:?}"),
        }
    }

    #[tokio::test]
    async fn user_cmd_custom() {
        tracing_subscriber::fmt::try_init().ok();

        let (settings, state, statistics, source) = defaults();
        state.write().await.custom_commands.insert(
            "hi".to_owned(),
            [(Source::Discord, "hello".to_owned())]
                .into_iter()
                .collect(),
        );

        match user_message(Span::current(), settings, state, statistics, "!hi", source)
            .await
            .unwrap()
        {
            UserResponse::Custom(message) => assert_eq!("hello", message),
            res => panic!("unexpected response: {res:?}"),
        }
    }

    #[tokio::test]
    async fn admin_cmd_unknown() {
        assert!(matches!(
            run_admin_message("!kaboom").await,
            Ok(AdminResponse::Unknown)
        ));
    }

    #[tokio::test]
    async fn admin_cmd_ahelp() {
        assert!(matches!(
            run_admin_message("!ahelp").await,
            Ok(AdminResponse::Help)
        ));
    }

    #[tokio::test]
    async fn admin_cmd_custom_commands() {
        match run_admin_message("!custom_commands list").await.unwrap() {
            AdminResponse::CustomCommands(CustomCommandsResponse::List(Ok(list))) => {
                assert!(list.is_empty());
            }
            AdminResponse::CustomCommands(CustomCommandsResponse::List(Err(e))) => panic!("{e:?}"),
            res => panic!("unexpected response: {res:?}"),
        }
    }

    #[tokio::test]
    async fn admin_cmd_statistics() {
        assert!(matches!(
            run_admin_message("!stats").await,
            Ok(AdminResponse::Statistics(Ok((false, _))))
        ));
    }

    #[tokio::test]
    async fn owner_cmd_ohelp() {
        assert!(matches!(
            run_owner_message("!ohelp", None).await,
            Ok(OwnerResponse::Help)
        ));
    }

    #[tokio::test]
    async fn owner_cmd_admins_list() {
        match run_owner_message("!admins list", None).await.unwrap() {
            OwnerResponse::Admins(AdminsResponse::List(list)) => assert!(list.is_empty()),
            res => panic!("unexpected response: {res:?}"),
        }
    }

    #[tokio::test]
    async fn owner_cmd_admins_add() {
        match run_owner_message("!admins add @test", Some(NonZeroU64::new(1).unwrap()))
            .await
            .unwrap()
        {
            OwnerResponse::Admins(AdminsResponse::Edit(Ok(AdminAction::Added))) => {}
            OwnerResponse::Admins(AdminsResponse::Edit(Err(e))) => panic!("{e:?}"),
            res => panic!("unexpected response: {res:?}"),
        }
    }
}
