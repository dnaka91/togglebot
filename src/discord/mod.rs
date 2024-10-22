//! Discord service connector that allows to receive commands from Discord servers.

use std::{
    fmt::{self, Display},
    sync::Arc,
};

use anyhow::Result;
use poise::serenity_prelude::{self as serenity, UserId};
use tokio::sync::oneshot;
use tokio_shutdown::Shutdown;
use tracing::{error, info, info_span, instrument, Instrument, Span};

use crate::{
    settings::{Commands as CommandSettings, Discord as DiscordSettings},
    AdminResponse, AdminsResponse, AuthorId, CustomCommandsResponse, Message, OwnerResponse, Queue,
    Response, Source, UserResponse,
};

mod admin;
mod owner;
mod user;

type Context<'a> = poise::ApplicationContext<'a, State, anyhow::Error>;

// --------------------------------------------
// OWNERS
// --------------------------------------------

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
            content: format!("!admins add @{user}"),
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
            content: format!("!admins remove @{user}"),
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
            content: "!admins list".to_owned(),
            author: ctx.author().id,
            mention: None,
        },
    )
    .await
}

// --------------------------------------------
// ADMINS
// --------------------------------------------

/// Show information about available owner commands. **Only available if you're an owner yourself.**
#[poise::command(slash_command, category = "Admin")]
async fn ohelp(ctx: Context<'_>) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: "!help".to_owned(),
            author: ctx.author().id,
            mention: None,
        },
    )
    .await
}

#[allow(clippy::unused_async)]
#[poise::command(
    slash_command,
    category = "Owner",
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
#[poise::command(slash_command, category = "Owner", rename = "add")]
async fn custom_commands_add(
    ctx: Context<'_>,
    target: Target,
    name: String,
    content: String,
) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: format!("!custom_commands add {target} {name} {content}"),
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
#[poise::command(slash_command, category = "Owner", rename = "remove")]
async fn custom_commands_remove(
    ctx: Context<'_>,
    target: Target,
    name: String,
    content: String,
) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: format!("!custom_commands remove {target} {name} {content}"),
            author: ctx.author().id,
            mention: None,
        },
    )
    .await
}

/// List all currently available custom commands.
#[poise::command(slash_command, category = "Owner", rename = "list")]
async fn custom_commands_list(ctx: Context<'_>) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: "!custom_commands list".to_owned(),
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
#[poise::command(slash_command, category = "Owner")]
async fn stats(ctx: Context<'_>, time: Time) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: format!("!stats {time}"),
            author: ctx.author().id,
            mention: None,
        },
    )
    .await
}

// --------------------------------------------
// USERS
// --------------------------------------------

/// Gives a list of admin commands (if you're an admin).
#[poise::command(slash_command, category = "User")]
async fn ahelp(ctx: Context<'_>) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: "!ahelp".to_owned(),
            author: ctx.author().id,
            mention: None,
        },
    )
    .await
}

/// Gives a short info about this bot.
#[poise::command(slash_command, aliases("bot"), category = "User")]
async fn help(ctx: Context<'_>) -> Result<()> {
    handle_message(
        ctx,
        SerenityMessage {
            content: "!help".to_owned(),
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
            content: "!links".to_owned(),
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
            content: format!("!ban {target}"),
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
            content: format!("!crates {name}"),
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
            content: "!today".to_owned(),
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
            content: format!("!ftoc {fahrenheit}"),
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
            content: format!("!ctof {celsius}"),
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
                admins(),
                // admins
                ohelp(),
                custom_commands(),
                stats(),
                // users
                ahelp(),
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

#[derive(Clone)]
struct State {
    settings: Arc<CommandSettings>,
    queue: Queue,
}

struct SerenityMessage {
    content: String,
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
            content: msg.content.clone(),
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

async fn handle_user_message(resp: UserResponse, ctx: Context<'_>) -> Result<()> {
    match resp {
        UserResponse::Help => user::help(ctx).await,
        UserResponse::Commands(res) => user::commands(ctx, res).await,
        UserResponse::Links(links) => user::links(ctx, links).await,
        UserResponse::Ban(target) => user::ban(ctx, target).await,
        UserResponse::Crate(res) => user::crate_(ctx, res).await,
        UserResponse::Today(content)
        | UserResponse::FahrenheitToCelsius(content)
        | UserResponse::CelsiusToFahrenheit(content)
        | UserResponse::Custom(content) => user::string_reply(ctx, content).await,
        UserResponse::Unknown => Ok(()),
    }
}

async fn handle_admin_message(resp: AdminResponse, ctx: Context<'_>) -> Result<()> {
    match resp {
        AdminResponse::Help => admin::help(ctx).await,
        AdminResponse::CustomCommands(resp) => match resp {
            CustomCommandsResponse::List(res) => admin::custom_commands_list(ctx, res).await,
            CustomCommandsResponse::Edit(res) => admin::custom_commands_edit(ctx, res).await,
        },
        AdminResponse::Statistics(res) => admin::stats(ctx, res).await,
        AdminResponse::Unknown => Ok(()),
    }
}

async fn handle_owner_message(resp: OwnerResponse, ctx: Context<'_>) -> Result<()> {
    match resp {
        OwnerResponse::Help => owner::help(ctx).await,
        OwnerResponse::Admins(resp) => match resp {
            AdminsResponse::List(res) => owner::admins_list(ctx, res).await,
            AdminsResponse::Edit(res) => owner::admins_edit(ctx, res).await,
        },
        OwnerResponse::Unknown => Ok(()),
    }
}
