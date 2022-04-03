//! Discord service connector that allows to receive commands from Discord servers.

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use futures_util::StreamExt;
use tokio::sync::oneshot;
use tokio_shutdown::Shutdown;
use tracing::{error, info};
use twilight_gateway::{Event, EventTypeFlags, Intents, Shard};
use twilight_http::{request::channel::message::CreateMessage, Client};
use twilight_model::channel::Message as ChannelMessage;

use crate::{
    settings::Discord, AdminResponse, AdminsResponse, AuthorId, CustomCommandsResponse, Message,
    OwnerResponse, Queue, Response, Source, UserResponse,
};

mod admin;
mod owner;
mod user;

/// Initiate and run the Discord bot connection in a background task.
///
/// It pushes messages into the given queue for processing, each message accompanied by a oneshot
/// channel, that allows to listen for the generated reply (if any). The shutdown handler is used
/// to gracefully shut down the connection before fully quitting the application.
pub async fn start(config: &Discord, queue: Queue, shutdown: Shutdown) -> Result<()> {
    let http = Arc::new(Client::new(config.token.clone()));

    let (shard, mut events) = Shard::builder(
        config.token.clone(),
        Intents::GUILD_MESSAGES | Intents::DIRECT_MESSAGES | Intents::MESSAGE_CONTENT,
    )
    .event_types(EventTypeFlags::READY | EventTypeFlags::MESSAGE_CREATE)
    .http_client(Arc::clone(&http))
    .build();
    let shard = Arc::new(shard);

    shard.start().await?;

    let shard_spawn = shard.clone();

    tokio::spawn(async move {
        shutdown.handle().await;

        info!("discord connection shutting down");
        shard_spawn.shutdown();
    });

    tokio::spawn(async move {
        while let Some(event) = events.next().await {
            let http = http.clone();
            let queue = queue.clone();

            tokio::spawn(async move {
                if let Err(e) = handle_event(queue, event, http).await {
                    error!(error = ?e, "error during event handling");
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

async fn handle_message(queue: Queue, msg: ChannelMessage, http: Arc<Client>) -> Result<()> {
    if msg.author.bot {
        // Ignore bots and our own messages.
        return Ok(());
    }

    let message = Message {
        source: Source::Discord,
        content: msg.content.clone(),
        author: AuthorId::Discord(msg.author.id.into()),
        mention: msg
            .mentions
            .first()
            .filter(|mention| !mention.bot)
            .map(|mention| mention.id.into()),
    };
    let (tx, rx) = oneshot::channel();

    if queue.send((message, tx)).await.is_ok() {
        if let Ok(resp) = rx.await {
            match resp {
                Response::User(user_resp) => handle_user_message(user_resp, msg, http).await?,
                Response::Admin(admin_resp) => handle_admin_message(admin_resp, msg, http).await?,
                Response::Owner(owner_resp) => handle_owner_message(owner_resp, msg, http).await?,
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
        UserResponse::Schedule(res) => user::schedule(msg, http, res).await,
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
        AdminResponse::Statistics(res) => admin::stats(msg, http, res).await,
        AdminResponse::Unknown => Ok(()),
    }
}

async fn handle_owner_message(
    resp: OwnerResponse,
    msg: ChannelMessage,
    http: Arc<Client>,
) -> Result<()> {
    match resp {
        OwnerResponse::Help => owner::help(msg, http).await,
        OwnerResponse::Admins(resp) => match resp {
            AdminsResponse::List(res) => owner::admins_list(msg, http, res).await,
            AdminsResponse::Edit(res) => owner::admins_edit(msg, http, res).await,
        },
        OwnerResponse::Unknown => Ok(()),
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
