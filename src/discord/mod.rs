//! Discord service connector that allows to receive commands from Discord servers.

use std::{
    fmt::{self, Display},
    sync::Arc,
};

use anyhow::Result;
use poise::serenity_prelude::{self as serenity, UserId};
use tokio::sync::oneshot;
use tokio_shutdown::Shutdown;
use tracing::{Instrument, Span, error, info, info_span, instrument};

use crate::{
    api::{
        AuthorId, Message, Queue, Source,
        request::{self, Request, StatisticsDate},
        response::{self, Response},
    },
    settings::{Commands as CommandSettings, Discord as DiscordSettings},
};

mod admin;
mod owner;
mod user;

type Context<'a> = poise::ApplicationContext<'a, State, anyhow::Error>;

// --------------------------------------------
// OWNERS
// --------------------------------------------

/// Show information about available owner commands. **Only available if you're an owner yourself.**
#[poise::command(slash_command, category = "Owner")]
async fn ohelp(ctx: Context<'_>) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: Request::Owner(request::Owner::Help),
            author: ctx.author().id,
            mention: None,
        },
    )
    .await
}

#[allow(clippy::unused_async)]
#[poise::command(
    slash_command,
    owners_only,
    category = "Owner",
    subcommands("admins_add", "admins_remove", "admins_list")
)]
async fn admins(_: Context<'_>) -> Result<()> {
    Ok(())
}

/// Add a user to/from the admin list.
///
/// An admin has access to most of the bot-controlling commands.
#[poise::command(slash_command, owners_only, category = "Owner", rename = "add")]
async fn admins_add(ctx: Context<'_>, user: UserId) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: Request::Owner(request::Owner::Admins(request::Admins::Add(user.into()))),
            author: ctx.author().id,
            mention: Some(user),
        },
    )
    .await
}

/// Remove a user to/from the admin list.
///
/// An admin has access to most of the bot-controlling commands.
#[poise::command(slash_command, owners_only, category = "Owner", rename = "remove")]
async fn admins_remove(ctx: Context<'_>, user: UserId) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: Request::Owner(request::Owner::Admins(request::Admins::Remove(user.into()))),
            author: ctx.author().id,
            mention: Some(user),
        },
    )
    .await
}

/// List all currently configured admin users.
#[poise::command(slash_command, owners_only, category = "Owner", rename = "list")]
async fn admins_list(ctx: Context<'_>) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: Request::Owner(request::Owner::Admins(request::Admins::List)),
            author: ctx.author().id,
            mention: None,
        },
    )
    .await
}

// --------------------------------------------
// ADMINS
// --------------------------------------------

/// Gives a list of admin commands (if you're an admin).
#[poise::command(slash_command, category = "Admin")]
async fn ahelp(ctx: Context<'_>) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: Request::Admin(request::Admin::Help),
            author: ctx.author().id,
            mention: None,
        },
    )
    .await
}

#[allow(clippy::unused_async)]
#[poise::command(
    slash_command,
    category = "Admin",
    subcommands(
        "custom_commands_add",
        "custom_commands_remove",
        "custom_commands_list"
    )
)]
async fn custom_commands(_: Context<'_>) -> Result<()> {
    Ok(())
}

#[derive(poise::ChoiceParameter)]
enum Target {
    /// Everywhere (Discord and Twitch).
    All,
    /// Only Discord.
    Discord,
    /// Only Twitch.
    Twitch,
}

impl Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::All => "all",
            Self::Discord => "discord",
            Self::Twitch => "twitch",
        })
    }
}

/// Add a custom command that has fixed content and can be anything.
///
/// The command can be modified for all sources or individually. Command names must start with a
/// lowercase letter, only consist of lowercase letters, numbers and underscores and must not start
/// with the `!`.
#[poise::command(slash_command, category = "Admin", rename = "add")]
async fn custom_commands_add(
    ctx: Context<'_>,
    target: Target,
    name: String,
    content: String,
) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: Request::Admin(request::Admin::CustomCommands(
                request::CustomCommands::Add {
                    source: match target {
                        Target::All => None,
                        Target::Discord => Some(Source::Discord),
                        Target::Twitch => Some(Source::Twitch),
                    },
                    name,
                    content,
                },
            )),
            author: ctx.author().id,
            mention: None,
        },
    )
    .await
}

/// Remove a custom command that has fixed content and can be anything.
///
/// The command can be modified for all sources or individually. Command names must start with a
/// lowercase letter, only consist of lowercase letters, numbers and underscores and must not start
/// with the `!`.
#[poise::command(slash_command, category = "Admin", rename = "remove")]
async fn custom_commands_remove(ctx: Context<'_>, target: Target, name: String) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: Request::Admin(request::Admin::CustomCommands(
                request::CustomCommands::Remove {
                    source: match target {
                        Target::All => None,
                        Target::Discord => Some(Source::Discord),
                        Target::Twitch => Some(Source::Twitch),
                    },
                    name,
                },
            )),
            author: ctx.author().id,
            mention: None,
        },
    )
    .await
}

/// List all currently available custom commands.
#[poise::command(slash_command, category = "Admin", rename = "list")]
async fn custom_commands_list(ctx: Context<'_>) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: Request::Admin(request::Admin::CustomCommands(
                request::CustomCommands::List,
            )),
            author: ctx.author().id,
            mention: None,
        },
    )
    .await
}

#[derive(poise::ChoiceParameter)]
enum Time {
    Current,
    Total,
}

impl Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Current => "current",
            Self::Total => "total",
        })
    }
}

/// Get statistics about command usage.
///
/// Either for the **current month** or the overall counters for **all time**.
#[poise::command(slash_command, category = "Admin")]
async fn stats(ctx: Context<'_>, time: Time) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: Request::Admin(request::Admin::Statistics(match time {
                Time::Current => StatisticsDate::Current,
                Time::Total => StatisticsDate::Total,
            })),
            author: ctx.author().id,
            mention: None,
        },
    )
    .await
}

// --------------------------------------------
// USERS
// --------------------------------------------

/// Gives a short info about this bot.
#[poise::command(slash_command, aliases("bot"), category = "User")]
async fn help(ctx: Context<'_>) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: Request::User(request::User::Help),
            author: ctx.author().id,
            mention: None,
        },
    )
    .await
}

/// List all available commands of the bot.
#[poise::command(slash_command, category = "User")]
async fn commands(ctx: Context<'_>, command: Option<String>) -> Result<()> {
    poise::builtins::help(
        ctx.into(),
        command.as_deref(),
        poise::builtins::HelpConfiguration::default(),
    )
    .await
    .map_err(Into::into)
}

/// Gives you a list of links to sites where the streamer is present.
#[poise::command(slash_command, category = "User")]
async fn links(ctx: Context<'_>) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: Request::User(request::User::Links),
            author: ctx.author().id,
            mention: None,
        },
    )
    .await
}

/// Refuse anything with the power of Gandalf.
#[poise::command(slash_command, category = "User")]
async fn ban(ctx: Context<'_>, target: String) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: Request::User(request::User::Ban(target)),
            author: ctx.author().id,
            mention: None,
        },
    )
    .await
}

/// Get the link for any existing crate.
#[poise::command(slash_command, category = "User")]
async fn crates(ctx: Context<'_>, name: String) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: Request::User(request::User::Ban(name)),
            author: ctx.author().id,
            mention: None,
        },
    )
    .await
}

/// Get details about the current day.
#[poise::command(slash_command, category = "User")]
async fn today(ctx: Context<'_>) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: Request::User(request::User::Today),
            author: ctx.author().id,
            mention: None,
        },
    )
    .await
}

/// Convert Fahrenheit to Celsius.
#[poise::command(slash_command, category = "User")]
async fn ftoc(ctx: Context<'_>, fahrenheit: f64) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: Request::User(request::User::Ftoc(fahrenheit)),
            author: ctx.author().id,
            mention: None,
        },
    )
    .await
}

/// Convert Celsius to Fahrenheit.
#[poise::command(slash_command, category = "User")]
async fn ctof(ctx: Context<'_>, celsius: f64) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: Request::User(request::User::Ftoc(celsius)),
            author: ctx.author().id,
            mention: None,
        },
    )
    .await
}

/// Initiate and run the Discord bot connection in a background task.
///
/// It pushes messages into the given queue for processing, each message accompanied by a oneshot
/// channel, that allows to listen for the generated reply (if any). The shutdown handler is used
/// to gracefully shut down the connection before fully quitting the application.
pub async fn start(
    config: &DiscordSettings,
    settings: Arc<CommandSettings>,
    queue: Queue,
    shutdown: Shutdown,
) -> Result<()> {
    let token = config.token.clone();
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                // owners
                ohelp(),
                admins(),
                // admins
                ahelp(),
                custom_commands(),
                stats(),
                // users
                help(),
                commands(),
                links(),
                ban(),
                crates(),
                today(),
                ftoc(),
                ctof(),
            ],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                anyhow::Ok(State { settings, queue })
            })
        })
        .build();

    let mut client =
        match serenity::ClientBuilder::new(token, serenity::GatewayIntents::non_privileged())
            .framework(framework)
            .await
        {
            Ok(client) => client,
            Err(e) => {
                error!(?e, "failed creating discord client");
                return Err(e.into());
            }
        };

    info!("discord connection ready, listening for events");

    tokio::spawn(async move {
        tokio::select! {
            () = shutdown.handle() => {}
            res = client.start() => {
                if let Err(e) = res {
                    error!(error = ?e, "failed running discord client");
                }
            }
        }

        client.shard_manager.shutdown_all().await;
        info!("discord connection shutting down");
    });

    Ok(())
}

struct State {
    settings: Arc<CommandSettings>,
    queue: Queue,
}

struct SerenityMessage {
    content: Request,
    author: UserId,
    mention: Option<UserId>,
}

#[instrument(skip_all, name = "discord message", fields(source = %Source::Discord))]
async fn handle_message(ctx: Context<'_>, msg: SerenityMessage) -> Result<()> {
    if ctx.author().bot {
        // Ignore bots and our own messages.
        return Ok(());
    }

    let queue = ctx.data().queue.clone();

    let response = async {
        let message = Message {
            span: Span::current(),
            source: Source::Discord,
            content: msg.content,
            author: AuthorId::Discord(msg.author.into()),
            mention: msg.mention.map(Into::into),
        };

        let (tx, rx) = oneshot::channel();

        if queue.send((message, tx)).await.is_ok() {
            Some(rx.await)
        } else {
            None
        }
    }
    .instrument(info_span!("handle"))
    .await;

    if let Some(Ok(resp)) = response {
        async {
            match resp {
                Response::User(user_resp) => handle_user_message(user_resp, ctx).await,
                Response::Admin(admin_resp) => handle_admin_message(admin_resp, ctx).await,
                Response::Owner(owner_resp) => handle_owner_message(owner_resp, ctx).await,
            }
        }
        .instrument(info_span!("reply"))
        .await?;
    }

    Ok(())
}

async fn handle_user_message(resp: response::User, ctx: Context<'_>) -> Result<()> {
    match resp {
        response::User::Help => user::help(ctx).await,
        response::User::Commands(res) => user::commands(ctx, res).await,
        response::User::Links(links) => user::links(ctx, links).await,
        response::User::Ban(target) => user::ban(ctx, target).await,
        response::User::Crate(res) => user::crate_(ctx, res).await,
        response::User::Today(content)
        | response::User::FahrenheitToCelsius(content)
        | response::User::CelsiusToFahrenheit(content) => user::string_reply(ctx, content).await,
        response::User::Custom(content) => user::custom_reply(ctx, content).await,
        response::User::Unknown => Ok(()),
    }
}

async fn handle_admin_message(resp: response::Admin, ctx: Context<'_>) -> Result<()> {
    match resp {
        response::Admin::Help => admin::help(ctx).await,
        response::Admin::CustomCommands(resp) => match resp {
            response::CustomCommands::List(res) => admin::custom_commands_list(ctx, res).await,
            response::CustomCommands::Edit(res) => admin::custom_commands_edit(ctx, res).await,
        },
        response::Admin::Statistics(res) => admin::stats(ctx, res).await,
    }
}

async fn handle_owner_message(resp: response::Owner, ctx: Context<'_>) -> Result<()> {
    match resp {
        response::Owner::Help => owner::help(ctx).await,
        response::Owner::Admins(resp) => match resp {
            response::Admins::List(res) => owner::admins_list(ctx, res).await,
            response::Admins::Edit(res) => owner::admins_edit(ctx, res).await,
        },
    }
}
