//! Main handling logic for all supported bot commands.

use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;
use tracing::Span;

use crate::{
    api::{request, response, AuthorId, Source},
    settings::{Commands as CommandSettings, Discord as DiscordSettings},
    state::State,
    statistics::{BuiltinCommand, Command, Stats},
};

mod admin;
mod owner;
mod user;

/// Convenience type alias for a [`State`] wrapped in an [`Arc`] and a [`RwLock`].
pub type AsyncState = Arc<RwLock<State>>;
/// Convenience type alias for [`Stats`] wrapped in an [`Arc`] and a [`RwLock`].
pub type AsyncStats = Arc<RwLock<Stats>>;
/// Convenience type alias for [`CommandSettings`] wrapped in an [`Arc`].
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
    content: request::User,
    source: Source,
) -> Result<response::User> {
    Ok(match content {
        request::User::Help => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::Help);
            user::help()
        }
        request::User::Commands(source) => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::Commands);
            user::commands(state, source).await
        }
        request::User::Links => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::Links);
            user::links(&settings)
        }
        request::User::Crate(name) => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::Crate);
            user::crate_(&name).await
        }
        request::User::Ban(target) => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::Ban);
            user::ban(&target)
        }
        request::User::Today => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::Today);
            user::today()
        }
        request::User::Ftoc(fahrenheit) => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::FahrenheitToCelsius);
            user::ftoc(fahrenheit)
        }
        request::User::Ctof(celsius) => {
            statistics
                .write()
                .await
                .increment_builtin(BuiltinCommand::CelsiusToFahrenheit);
            user::ctof(celsius)
        }
        request::User::Custom(name) => {
            let response = user::custom(state, source, &name).await;

            let name = match response {
                Some(_) => Command::Custom(&name),
                None => Command::Unknown(&name),
            };
            statistics.write().await.increment(name);

            response.unwrap_or(response::User::Unknown)
        }
    })
}

/// Handle admin facing messages to control the bot and prepare a response.
#[tracing::instrument(parent = span, skip_all, name = "admin")]
pub async fn admin_message(
    span: Span,
    state: AsyncState,
    statistics: AsyncStats,
    content: request::Admin,
) -> Result<response::Admin> {
    Ok(match content {
        request::Admin::Help => admin::help(),
        request::Admin::CustomCommands(request::CustomCommands::List) => {
            admin::custom_commands_list(state).await
        }
        request::Admin::CustomCommands(request::CustomCommands::Add {
            source,
            name,
            content,
        }) => {
            admin::custom_commands(
                state,
                statistics,
                &content,
                admin::Action::Add,
                source,
                &name,
            )
            .await
        }
        request::Admin::CustomCommands(request::CustomCommands::Remove { source, name }) => {
            admin::custom_commands(state, statistics, "", admin::Action::Remove, source, &name)
                .await
        }
        request::Admin::Statistics(date) => admin::stats(statistics, date).await,
    })
}

/// Handle messages only accessible to owners defined in the settings and prepare a response.
#[tracing::instrument(parent = span, skip_all, name = "owner")]
pub async fn owner_message(
    span: Span,
    state: AsyncState,
    content: request::Owner,
) -> Result<response::Owner> {
    Ok(match content {
        request::Owner::Help => owner::help(),
        request::Owner::Admins(request::Admins::List) => owner::admins_list(state).await,
        request::Owner::Admins(request::Admins::Add(id)) => {
            owner::admins_edit(state, owner::Action::Add, id).await
        }
        request::Owner::Admins(request::Admins::Remove(id)) => {
            owner::admins_edit(state, owner::Action::Remove, id).await
        }
    })
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use similar_asserts::assert_eq;

    use self::response::AdminAction;
    use super::*;
    use crate::api::request::StatisticsDate;

    fn defaults() -> (AsyncCommandSettings, AsyncState, AsyncStats, Source) {
        (
            Arc::new(CommandSettings::default()),
            Arc::new(RwLock::new(State::default())),
            Arc::new(RwLock::new(Stats::default())),
            Source::Discord,
        )
    }

    async fn run_user_message(content: request::User) -> Result<response::User> {
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

    async fn run_admin_message(content: request::Admin) -> Result<response::Admin> {
        tracing_subscriber::fmt::try_init().ok();
        let (_, state, statistics, _) = defaults();
        admin_message(Span::current(), state, statistics, content).await
    }

    async fn run_owner_message(content: request::Owner) -> Result<response::Owner> {
        tracing_subscriber::fmt::try_init().ok();
        let (_, state, _, _) = defaults();
        owner_message(Span::current(), state, content).await
    }

    // #[tokio::test]
    // async fn user_cmd_unknown() {
    //     assert!(matches!(
    //         run_user_message("!kaboom").await,
    //         Ok(response::User::Unknown)
    //     ));
    // }

    #[tokio::test]
    async fn user_cmd_help() {
        assert!(matches!(
            run_user_message(request::User::Help).await,
            Ok(response::User::Help)
        ));
    }

    #[tokio::test]
    async fn user_cmd_commands() {
        match run_user_message(request::User::Commands(Source::Twitch))
            .await
            .unwrap()
        {
            response::User::Commands(Ok(cmds)) => assert!(cmds.is_empty()),
            response::User::Commands(Err(e)) => panic!("{e:?}"),
            res => panic!("unexpected response: {res:?}"),
        }
    }

    #[tokio::test]
    async fn user_cmd_links() {
        assert!(matches!(
            run_user_message(request::User::Links).await,
            Ok(response::User::Links(_))
        ));
    }

    #[tokio::test]
    async fn user_cmd_ban() {
        match run_user_message(request::User::Ban("me".to_owned()))
            .await
            .unwrap()
        {
            response::User::Ban(target) => assert_eq!("me", target),
            res => panic!("unexpected response: {res:?}"),
        }
    }

    #[tokio::test]
    async fn user_cmd_crate() {
        match run_user_message(request::User::Crate("anyhow".to_owned()))
            .await
            .unwrap()
        {
            response::User::Crate(Ok(_)) => {}
            response::User::Crate(Err(e)) => panic!("{e:?}"),
            res => panic!("unexpected response: {res:?}"),
        }
    }

    #[tokio::test]
    async fn user_cmd_ftoc() {
        match run_user_message(request::User::Ftoc(350.0)).await.unwrap() {
            response::User::FahrenheitToCelsius(msg) => assert_eq!("350.0°F => 176.7°C", msg),
            res => panic!("unexpected response: {res:?}"),
        }
    }

    // #[tokio::test]
    // async fn user_cmd_ftoc_invalid() {
    //     match run_user_message("!ftoc test").await.unwrap() {
    //         response::User::FahrenheitToCelsius(msg) => {
    //             assert_eq!("that doesn't appear to be a number?!", msg);
    //         }
    //         res => panic!("unexpected response: {res:?}"),
    //     }
    // }

    #[tokio::test]
    async fn user_cmd_ctof() {
        match run_user_message(request::User::Ctof(176.67)).await.unwrap() {
            response::User::CelsiusToFahrenheit(msg) => assert_eq!("176.7°C => 350.0°F", msg),
            res => panic!("unexpected response: {res:?}"),
        }
    }

    // #[tokio::test]
    // async fn user_cmd_ctof_invalid() {
    //     match run_user_message("!ctof test").await.unwrap() {
    //         response::User::CelsiusToFahrenheit(msg) => {
    //             assert_eq!("that doesn't appear to be a number?!", msg);
    //         }
    //         res => panic!("unexpected response: {res:?}"),
    //     }
    // }

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

        match user_message(
            Span::current(),
            settings,
            state,
            statistics,
            request::User::Custom("hi".to_owned()),
            source,
        )
        .await
        .unwrap()
        {
            response::User::Custom(message) => assert_eq!("hello", message),
            res => panic!("unexpected response: {res:?}"),
        }
    }

    // #[tokio::test]
    // async fn admin_cmd_unknown() {
    //     assert!(matches!(
    //         run_admin_message("!kaboom").await,
    //         Ok(response::Admin::Unknown)
    //     ));
    // }

    #[tokio::test]
    async fn admin_cmd_ahelp() {
        assert!(matches!(
            run_admin_message(request::Admin::Help).await,
            Ok(response::Admin::Help)
        ));
    }

    #[tokio::test]
    async fn admin_cmd_custom_commands() {
        match run_admin_message(request::Admin::CustomCommands(
            request::CustomCommands::List,
        ))
        .await
        .unwrap()
        {
            response::Admin::CustomCommands(response::CustomCommands::List(Ok(list))) => {
                assert!(list.is_empty());
            }
            response::Admin::CustomCommands(response::CustomCommands::List(Err(e))) => {
                panic!("{e:?}")
            }
            res => panic!("unexpected response: {res:?}"),
        }
    }

    #[tokio::test]
    async fn admin_cmd_statistics() {
        assert!(matches!(
            run_admin_message(request::Admin::Statistics(StatisticsDate::Current)).await,
            Ok(response::Admin::Statistics(Ok((false, _))))
        ));
    }

    #[tokio::test]
    async fn owner_cmd_ohelp() {
        assert!(matches!(
            run_owner_message(request::Owner::Help).await,
            Ok(response::Owner::Help)
        ));
    }

    #[tokio::test]
    async fn owner_cmd_admins_list() {
        match run_owner_message(request::Owner::Admins(request::Admins::List))
            .await
            .unwrap()
        {
            response::Owner::Admins(response::Admins::List(list)) => assert!(list.is_empty()),
            res => panic!("unexpected response: {res:?}"),
        }
    }

    #[tokio::test]
    async fn owner_cmd_admins_add() {
        match run_owner_message(request::Owner::Admins(request::Admins::Add(
            NonZeroU64::new(1).unwrap(),
        )))
        .await
        .unwrap()
        {
            response::Owner::Admins(response::Admins::Edit(Ok(AdminAction::Added))) => {}
            response::Owner::Admins(response::Admins::Edit(Err(e))) => panic!("{e:?}"),
            res => panic!("unexpected response: {res:?}"),
        }
    }
}
