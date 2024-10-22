//! Twitch service connector that allows to receive commands from Twitch channels.

use std::{collections::HashMap, sync::Arc};

use anyhow::{Context, Result};
use futures_util::StreamExt;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tokio::{select, sync::oneshot};
use tokio_shutdown::Shutdown;
use tracing::{error, info, info_span, instrument, Instrument, Span};
use twitch_api::{
    eventsub::channel::ChannelChatMessageV1Payload,
    helix,
    twitch_oauth2::{
        client::Client as Oauth2Client, tokens::errors::ValidationError, RefreshToken, UserToken,
    },
    types::MsgId,
    HelixClient,
};

use self::eventsub::{EventSubClient, Replier};
use crate::{
    settings::{Commands as CommandSettings, Twitch as TwitchSettings},
    AuthorId, CrateSearch, Message, Queue, Response, Source, UserResponse,
};

mod eventsub;

#[expect(dead_code)]
#[derive(Debug)]
struct StreamInfo {
    id: String,
    started_at: OffsetDateTime,
    title: String,
    category: String,
}

impl TryFrom<helix::streams::Stream> for StreamInfo {
    type Error = anyhow::Error;

    fn try_from(value: helix::streams::Stream) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            id: value.id.take(),
            started_at: OffsetDateTime::parse(value.started_at.as_str(), &Rfc3339)
                .context("invalid stream start time")?,
            title: value.title,
            category: value.game_name,
        })
    }
}

/// Initialize and run the Twitch connection in a background task.
///
/// The given queue is used to transfer received messages for further processing, combined with a
/// oneshot channel to listen for any possible replies to a message. The shutdown handle is used
/// to gracefully disconnect from Twitch, before fully quitting the application.
#[allow(clippy::missing_panics_doc)]
pub async fn start(
    config: &TwitchSettings,
    settings: Arc<CommandSettings>,
    queue: Queue,
    shutdown: Shutdown,
) -> Result<()> {
    let client = HelixClient::with_client(reqwest::Client::new());
    let token = create_token(&client, config).await?;

    let streamer_id = client
        .get_channel_from_login(&settings.streamer, &token)
        .await?
        .context("streamer doesn't exist")?
        .broadcaster_id;

    let stream_info = client
        .get_streams_from_ids(&[&streamer_id][..].into(), &token)
        .next()
        .await
        .transpose()
        .context("failed getting stream info")?
        .map(StreamInfo::try_from)
        .transpose()
        .context("failed parsing stream info")?;

    info!(?stream_info);

    let mut sub = EventSubClient::new(client, token, streamer_id).await?;
    let replier = sub.create_replier();

    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let shutdown2 = shutdown.clone();

    tokio::spawn(async move {
        loop {
            select! {
                () = shutdown.handle() => break,
                res = sub.start(tx.clone()) => {
                    if let Err(e) = res {
                        error!(error = ?e, "failed running twitch client");
                    }
                }
            }
        }
    });

    tokio::spawn(async move {
        loop {
            select! {
                () = shutdown2.handle() => break,
                message = rx.recv() => {
                    if let Some(message) = message {
                        handle_message(queue.clone(), message, &replier).await.expect("success");
                    } else {
                        break;
                    }
                }
            }
        }
    });

    info!("twitch connection ready, listening for events");

    Ok(())
}

async fn create_token(client: &impl Oauth2Client, config: &TwitchSettings) -> Result<UserToken> {
    let result = UserToken::from_existing(
        client,
        config.access_token.clone().into(),
        Some(config.refresh_token.clone().into()),
        Some(config.client_secret.clone().into()),
    )
    .await;

    match result {
        Ok(token) => Ok(token),
        Err(ValidationError::NotAuthorized) => {
            // Token expired, use refresh token and try again
            let client_secret = config.client_secret.clone().into();
            let (access_token, _, refresh_token) = RefreshToken::from(config.refresh_token.clone())
                .refresh_token(client, &config.client_id.clone().into(), &client_secret)
                .await?;

            UserToken::from_existing(client, access_token, refresh_token, Some(client_secret))
                .await
                .map_err(Into::into)
        }
        Err(err) => Err(err.into()),
    }
}

#[instrument(skip_all, name = "twitch message", fields(source = %Source::Twitch))]
async fn handle_message(
    queue: Queue,
    msg: ChannelChatMessageV1Payload,
    client: &Replier,
) -> Result<()> {
    let response = async {
        let message = Message {
            span: Span::current(),
            source: Source::Twitch,
            content: msg.message.text.clone(),
            author: AuthorId::Twitch(msg.message_id.as_str().to_owned()),
            mention: None,
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
                Response::User(user_resp) => {
                    handle_user_message(user_resp, &msg.message_id, client).await
                }
                Response::Admin(_) | Response::Owner(_) => Ok(()),
            }
        }
        .instrument(info_span!("reply"))
        .await?;
    }

    Ok(())
}

async fn handle_user_message(resp: UserResponse, msg_id: &MsgId, client: &Replier) -> Result<()> {
    match resp {
        UserResponse::Help => handle_help(msg_id, client).await,
        UserResponse::Commands(res) => handle_commands(msg_id, client, res).await,
        UserResponse::Links(links) => handle_links(msg_id, client, links).await,
        UserResponse::Ban(target) => handle_ban(msg_id, client, target).await,
        UserResponse::Crate(res) => handle_crate(msg_id, client, res).await,
        UserResponse::Doc(res) => handle_doc(msg_id, client, res).await,
        UserResponse::Today(text)
        | UserResponse::FahrenheitToCelsius(text)
        | UserResponse::CelsiusToFahrenheit(text)
        | UserResponse::Custom(text) => handle_string_reply(msg_id, client, text).await,
        UserResponse::Unknown => Ok(()),
    }
}

async fn handle_help(msg_id: &MsgId, client: &Replier) -> Result<()> {
    client
        .send_chat_message(
            msg_id,
            "Thanks for asking, I'm a bot to help answer some typical questions. \
            Try out `!commands` command to see what I can do. \
            My source code is at https://github.com/dnaka91/togglebot"
                .to_owned(),
        )
        .await?;

    Ok(())
}

async fn handle_commands(msg_id: &MsgId, client: &Replier, res: Result<Vec<String>>) -> Result<()> {
    let message = match res {
        Ok(names) => names.into_iter().fold(
            String::from(
                "Available commands: !help (or !bot), !links, !ban, !crate(s), !doc(s), !today, \
                 !ftoc, !ctof",
            ),
            |mut list, name| {
                list.push_str(", !");
                list.push_str(&name);
                list
            },
        ),
        Err(e) => {
            error!(error = ?e, "failed listing commands");
            "Sorry, something went wrong fetching the list of commands".to_owned()
        }
    };

    client.send_chat_message(msg_id, message).await?;

    Ok(())
}

async fn handle_links(
    msg_id: &MsgId,
    client: &Replier,
    links: Arc<HashMap<String, String>>,
) -> Result<()> {
    client
        .send_chat_message(
            msg_id,
            links
                .iter()
                .enumerate()
                .fold(String::new(), |mut list, (i, (name, url))| {
                    if i > 0 {
                        list.push_str(" | ");
                    }

                    list.push_str(name);
                    list.push_str(": ");
                    list.push_str(url);
                    list
                }),
        )
        .await?;

    Ok(())
}

async fn handle_ban(msg_id: &MsgId, client: &Replier, target: String) -> Result<()> {
    client
        .send_chat_message(msg_id, format!("{target}, YOU SHALL NOT PASS!!"))
        .await?;

    Ok(())
}

async fn handle_crate(msg_id: &MsgId, client: &Replier, res: Result<CrateSearch>) -> Result<()> {
    let message = match res {
        Ok(search) => match search {
            CrateSearch::Found(info) => format!("https://crates.io/crates/{}", info.name),
            CrateSearch::NotFound(message) => message,
        },
        Err(e) => {
            error!(error = ?e, "failed searching for crate");
            "Sorry, something went wrong looking up the crate".to_owned()
        }
    };

    client.send_chat_message(msg_id, message).await?;

    Ok(())
}

async fn handle_doc(msg_id: &MsgId, client: &Replier, res: Result<String>) -> Result<()> {
    let message = match res {
        Ok(link) => link,
        Err(e) => {
            error!(error = ?e, "failed searching for docs");
            "Sorry, something went wrong looking up the documentation".to_owned()
        }
    };

    client.send_chat_message(msg_id, message).await?;

    Ok(())
}

async fn handle_string_reply(msg_id: &MsgId, client: &Replier, content: String) -> Result<()> {
    client.send_chat_message(msg_id, content).await?;

    Ok(())
}
