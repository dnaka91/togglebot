//! Discord service connector that allows to receive commands from Discord servers.

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use futures_util::StreamExt;
use tokio::sync::oneshot;
use tracing::{error, info};
use twilight_gateway::{Event, EventTypeFlags, Intents, Shard};
use twilight_http::{request::channel::message::CreateMessage, Client};
use twilight_model::{channel::Message as ChannelMessage, id::UserId};

use crate::{
    settings::Discord, AdminResponse, CustomCommandsResponse, Message, Queue, Response, Shutdown,
    Source, UserResponse,
};

mod admin;
mod user;

pub async fn start(config: &Discord, queue: Queue, mut shutdown: Shutdown) -> Result<()> {
    let http = Arc::new(Client::new(config.token.clone()));

    let (shard, mut events) = Shard::builder(
        &config.token,
        Intents::GUILD_MESSAGES | Intents::DIRECT_MESSAGES,
    )
    .event_types(EventTypeFlags::READY | EventTypeFlags::MESSAGE_CREATE)
    .http_client(Arc::clone(&http))
    .build();
    let shard = Arc::new(shard);

    shard.start().await?;

    let shard_spawn = shard.clone();

    tokio::spawn(async move {
        shutdown.recv().await.ok();

        info!("discord connection shutting down");
        shard_spawn.shutdown();
    });

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

async fn handle_event(queue: Queue, event: Event, http: Arc<Client>) -> Result<()> {
    match event {
        Event::MessageCreate(msg) => handle_message(queue, msg.0, http).await?,
        Event::Ready(_) => info!("discord connection ready, listening for events"),
        _ => {}
    }

    Ok(())
}

/// List of admins that are allowed to customize the bot. Currently static and will be added to the
/// settings in the future.
#[allow(clippy::unreadable_literal)]
const ADMINS: &[UserId] = unsafe {
    &[
        UserId::new_unchecked(110883807707566080), // dnaka91
        UserId::new_unchecked(648566744797020190), // ToggleBit
        UserId::new_unchecked(327267106834087936), // _Bare
        UserId::new_unchecked(378644347354087434), // TrolledWoods
    ]
};

async fn handle_message(queue: Queue, msg: ChannelMessage, http: Arc<Client>) -> Result<()> {
    if msg.author.bot {
        // Ignore bots and our own messages.
        return Ok(());
    }

    let message = Message {
        source: Source::Discord,
        content: msg.content.clone(),
        admin: msg.guild_id.is_none() && ADMINS.contains(&msg.author.id),
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

async fn handle_user_message(
    resp: UserResponse,
    msg: ChannelMessage,
    http: Arc<Client>,
) -> Result<()> {
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
        UserResponse::Crate(res) => user::crate_(msg, http, res).await,
        UserResponse::Doc(res) => user::doc(msg, http, res).await,
        UserResponse::Custom(content) => user::custom(msg, http, content).await,
        UserResponse::Unknown => Ok(()),
    }
}

async fn handle_admin_message(
    resp: AdminResponse,
    msg: ChannelMessage,
    http: Arc<Client>,
) -> Result<()> {
    match resp {
        AdminResponse::Help => admin::help(msg, http).await,
        AdminResponse::Schedule(res) => admin::schedule(msg, http, res).await,
        AdminResponse::OffDays(res) => admin::off_days(msg, http, res).await,
        AdminResponse::CustomCommands(resp) => match resp {
            CustomCommandsResponse::List(res) => admin::custom_commands_list(msg, http, res).await,
            CustomCommandsResponse::Edit(res) => admin::custom_commands_edit(msg, http, res).await,
        },
        AdminResponse::Unknown => Ok(()),
    }
}

/// Simple trait that combines the new `value.exec().await?.model.await` chain into a simple
/// method call.
#[async_trait]
trait ExecModelExt {
    type Value;

    /// Send the command by calling `exec()` and `model()`.
    async fn send(self) -> Result<Self::Value>;
}

#[async_trait]
impl<'a> ExecModelExt for CreateMessage<'a> {
    type Value = ChannelMessage;

    async fn send(self) -> Result<Self::Value> {
        self.exec().await?.model().await.map_err(Into::into)
    }
}
