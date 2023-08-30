//! Twitch service connector that allows to receive commands from Twitch channels.

use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use tokio::{select, sync::oneshot};
use tokio_shutdown::Shutdown;
use tracing::{error, info, info_span, instrument, Instrument, Span};
use twitch_irc::{
    login::StaticLoginCredentials,
    message::{PrivmsgMessage, ServerMessage},
    ClientConfig, SecureTCPTransport, TwitchIRCClient,
};

use crate::{
    settings::{Commands as CommandSettings, Twitch as TwitchSettings},
    AuthorId, CrateSearch, Message, Queue, Response, Source, UserResponse,
};

type Client = TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>;

/// Initialize and run the Twitch connection in a background task.
///
/// The given queue is used to transfer received messages for further processing, combined with a
/// oneshot channel to listen for any possible replies to a message. The shutdown handle is used
/// to gracefully disconnect from Twitch, before fully quitting the application.
#[allow(clippy::missing_panics_doc)]
pub fn start(
    config: &TwitchSettings,
    settings: Arc<CommandSettings>,
    queue: Queue,
    shutdown: Shutdown,
) -> Result<()> {
    let config = ClientConfig::new_simple(StaticLoginCredentials::new(
        config.login.clone(),
        Some(config.token.clone()),
    ));
    let (mut messages, client) = Client::new(config);

    client.join(settings.streamer.clone())?;

    tokio::spawn(async move {
        loop {
            select! {
                _ = shutdown.handle() => break,
                message = messages.recv() => {
                    if let Some(message) = message {
                        let settings = Arc::clone(&settings);
                        let client = client.clone();
                        let queue = queue.clone();

                        tokio::spawn(async move {
                            if let Err(e) = handle_server_message(settings, queue, message, client).await {
                                error!(error = ?e, "error during event handling");
                            }
                        });
                    } else {
                        break;
                    }
                }
            }
        }

        info!("twitch connection shutting down");
    });

    Ok(())
}

async fn handle_server_message(
    settings: Arc<CommandSettings>,
    queue: Queue,
    message: ServerMessage,
    client: Client,
) -> Result<()> {
    match message {
        ServerMessage::Privmsg(msg) => handle_message(settings, queue, msg, client).await?,
        ServerMessage::Join(_) => info!("twitch connection ready, listening for events"),
        _ => {}
    }

    Ok(())
}

#[instrument(skip_all, name = "twitch message", fields(source = %Source::Twitch))]
async fn handle_message(
    settings: Arc<CommandSettings>,
    queue: Queue,
    msg: PrivmsgMessage,
    client: Client,
) -> Result<()> {
    let response = async {
        let message = Message {
            span: Span::current(),
            source: Source::Twitch,
            content: msg.message_text.clone(),
            author: AuthorId::Twitch(msg.sender.id),
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
                    handle_user_message(settings, user_resp, msg.message_id, client).await
                }
                Response::Admin(_) | Response::Owner(_) => Ok(()),
            }
        }
        .instrument(info_span!("reply"))
        .await?;
    }

    Ok(())
}

#[allow(clippy::match_same_arms)]
async fn handle_user_message(
    settings: Arc<CommandSettings>,
    resp: UserResponse,
    msg_id: String,
    client: Client,
) -> Result<()> {
    match resp {
        UserResponse::Help => handle_help(settings, msg_id, client).await,
        UserResponse::Commands(res) => handle_commands(settings, msg_id, client, res).await,
        UserResponse::Links(links) => handle_links(settings, msg_id, client, links).await,
        UserResponse::Ban(target) => handle_ban(settings, msg_id, client, target).await,
        UserResponse::Crate(res) => handle_crate(settings, msg_id, client, res).await,
        UserResponse::Doc(res) => handle_doc(settings, msg_id, client, res).await,
        UserResponse::Today(text)
        | UserResponse::FahrenheitToCelsius(text)
        | UserResponse::CelsiusToFahrenheit(text)
        | UserResponse::Custom(text) => handle_string_reply(settings, msg_id, client, text).await,
        UserResponse::Unknown => Ok(()),
    }
}

async fn handle_help(settings: Arc<CommandSettings>, msg_id: String, client: Client) -> Result<()> {
    client
        .say_in_reply_to(
            &(&settings.streamer, &msg_id),
            "Thanks for asking, I'm a bot to help answer some typical questions. \
            Try out `!commands` command to see what I can do. \
            My source code is at https://github.com/dnaka91/togglebot"
                .to_owned(),
        )
        .await?;

    Ok(())
}

async fn handle_commands(
    settings: Arc<CommandSettings>,
    msg_id: String,
    client: Client,
    res: Result<Vec<String>>,
) -> Result<()> {
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

    client
        .say_in_reply_to(&(&settings.streamer, &msg_id), message)
        .await?;

    Ok(())
}

async fn handle_links(
    settings: Arc<CommandSettings>,
    msg_id: String,
    client: Client,
    links: Arc<HashMap<String, String>>,
) -> Result<()> {
    client
        .say_in_reply_to(
            &(&settings.streamer, &msg_id),
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

async fn handle_ban(
    settings: Arc<CommandSettings>,
    msg_id: String,
    client: Client,
    target: String,
) -> Result<()> {
    client
        .say_in_reply_to(
            &(&settings.streamer, &msg_id),
            format!("{target}, YOU SHALL NOT PASS!!"),
        )
        .await?;

    Ok(())
}

async fn handle_crate(
    settings: Arc<CommandSettings>,
    msg_id: String,
    client: Client,
    res: Result<CrateSearch>,
) -> Result<()> {
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

    client
        .say_in_reply_to(&(&settings.streamer, &msg_id), message)
        .await?;

    Ok(())
}

async fn handle_doc(
    settings: Arc<CommandSettings>,
    msg_id: String,
    client: Client,
    res: Result<String>,
) -> Result<()> {
    let message = match res {
        Ok(link) => link,
        Err(e) => {
            error!(error = ?e, "failed searching for docs");
            "Sorry, something went wrong looking up the documentation".to_owned()
        }
    };

    client
        .say_in_reply_to(&(&settings.streamer, &msg_id), message)
        .await?;

    Ok(())
}

async fn handle_string_reply(
    settings: Arc<CommandSettings>,
    msg_id: String,
    client: Client,
    content: String,
) -> Result<()> {
    client
        .say_in_reply_to(&(&settings.streamer, &msg_id), content)
        .await?;

    Ok(())
}
