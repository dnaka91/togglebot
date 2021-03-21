//! Discord service connector that allows to receive commands from Discord servers.

use anyhow::Result;
use futures_util::StreamExt;
use log::{error, info};
use tokio::sync::oneshot;
use twilight_gateway::{Event, EventTypeFlags, Intents, Shard};
use twilight_http::Client;
use twilight_model::channel::Message as ChannelMessage;

use crate::{
    settings::Discord, AdminResponse, Message, Queue, Response, Shutdown, Source, UserResponse,
};

mod admin;
mod user;

pub async fn start(config: &Discord, queue: Queue, mut shutdown: Shutdown) -> Result<()> {
    let http = Client::new(&config.token);

    let mut shard = Shard::builder(
        &config.token,
        Intents::GUILD_MESSAGES | Intents::DIRECT_MESSAGES,
    )
    .http_client(http.clone())
    .build();

    shard.start().await?;

    let shard_spawn = shard.clone();

    tokio::spawn(async move {
        shutdown.recv().await.ok();

        info!("discord connection shutting down");
        shard_spawn.shutdown();
    });

    let mut events = shard.some_events(EventTypeFlags::READY | EventTypeFlags::MESSAGE_CREATE);

    tokio::spawn(async move {
        while let Some(event) = events.next().await {
            let http = http.clone();
            let queue = queue.clone();

            tokio::spawn(async move {
                if let Err(e) = handle_event(queue, event, http).await {
                    error!("error during event handling: {}", e);
                }
            });
        }
    });

    Ok(())
}

async fn handle_event(queue: Queue, event: Event, http: Client) -> Result<()> {
    match event {
        Event::MessageCreate(msg) => handle_message(queue, msg.0, http).await?,
        Event::Ready(_) => info!("discord connection ready, listening for events"),
        _ => {}
    }

    Ok(())
}

/// List of admins that are allowed to customize the bot. Currently static and will be added to the
/// settings in the future.
const ADMINS: &[(&str, &str)] = &[
    ("dnaka91", "1754"),
    ("ToggleBit", "0090"),
    ("_Bare", "6674"),
    ("TrolledWoods", "2954"),
];

async fn handle_message(queue: Queue, msg: ChannelMessage, http: Client) -> Result<()> {
    if msg.author.bot {
        // Ignore bots and our own messages.
        return Ok(());
    }

    let message = Message {
        source: Source::Discord,
        content: msg.content.clone(),
        admin: msg.guild_id.is_none()
            && ADMINS.contains(&(&msg.author.name, &msg.author.discriminator)),
    };
    let (tx, rx) = oneshot::channel();

    if queue.send((message, tx)).await.is_ok() {
        if let Ok(resp) = rx.await {
            match resp {
                Response::User(user_resp) => handle_user_message(user_resp, msg, http).await?,
                Response::Admin(admin_resp) => handle_admin_message(admin_resp, msg, http).await?,
            }
        }
    }

    Ok(())
}

async fn handle_user_message(resp: UserResponse, msg: ChannelMessage, http: Client) -> Result<()> {
    match resp {
        UserResponse::Help => user::help(msg, http).await,
        UserResponse::Commands(res) => user::commands(msg, http, res).await,
        UserResponse::Links(links) => user::links(msg, http, links).await,
        UserResponse::Schedule {
            start,
            finish,
            off_days,
        } => user::schedule(msg, http, start, finish, off_days).await,
        UserResponse::Ban(target) => user::ban(msg, http, target).await,
        UserResponse::Custom(content) => user::custom(msg, http, content).await,
        UserResponse::Unknown => Ok(()),
    }
}

async fn handle_admin_message(
    resp: AdminResponse,
    msg: ChannelMessage,
    http: Client,
) -> Result<()> {
    match resp {
        AdminResponse::Help => admin::help(msg, http).await,
        AdminResponse::Schedule(res) => admin::schedule(msg, http, res).await,
        AdminResponse::OffDays(res) => admin::off_days(msg, http, res).await,
        AdminResponse::CustomCommands(res) => admin::custom_commands(msg, http, res).await,
        AdminResponse::Unknown => Ok(()),
    }
}
