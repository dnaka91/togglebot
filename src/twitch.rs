use anyhow::Result;
use log::{error, info};
use tokio::{select, sync::oneshot};
use twitch_irc::{
    login::StaticLoginCredentials,
    message::{PrivmsgMessage, ServerMessage},
    ClientConfig, TCPTransport, TwitchIRCClient,
};

use crate::{settings::Twitch, Message, Queue, Response, Shutdown, Source, UserResponse};

type Client = TwitchIRCClient<TCPTransport, StaticLoginCredentials>;

const CHANNEL: &str = "togglebit";

pub async fn start(config: &Twitch, queue: Queue, mut shutdown: Shutdown) -> Result<()> {
    let config = ClientConfig::new_simple(StaticLoginCredentials::new(
        config.login.clone(),
        Some(config.token.clone()),
    ));
    let (mut messages, client) = Client::new(config);

    client.join(CHANNEL.to_owned());

    tokio::spawn(async move {
        loop {
            select! {
                _ = shutdown.recv() => break,
                message = messages.recv() => {
                    if let Some(message) = message {
                        let client = client.clone();
                        let queue = queue.clone();

                        tokio::spawn(async move {
                            if let Err(e) = handle_server_message(queue, message, client).await {
                                error!("error during event handling: {}", e);
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

async fn handle_server_message(queue: Queue, message: ServerMessage, client: Client) -> Result<()> {
    match message {
        ServerMessage::Privmsg(msg) => handle_message(queue, msg, client).await?,
        ServerMessage::Join(_) => info!("twitch connection ready, listening for events"),
        _ => {}
    }

    Ok(())
}

async fn handle_message(queue: Queue, msg: PrivmsgMessage, client: Client) -> Result<()> {
    let message = Message {
        source: Source::Twitch,
        content: msg.message_text.clone(),
        admin: false,
    };
    let (tx, rx) = oneshot::channel();

    if queue.send((message, tx)).await.is_ok() {
        if let Ok(resp) = rx.await {
            match resp {
                Response::User(user_resp) => handle_user_message(user_resp, msg, client).await?,
                Response::Admin(_) => {}
            }
        }
    }

    Ok(())
}

async fn handle_user_message(
    resp: UserResponse,
    msg: PrivmsgMessage,
    client: Client,
) -> Result<()> {
    match resp {
        UserResponse::Help => {
            client
                .say_in_response(
                    CHANNEL.to_owned(),
                    format!(
                        "Thanks for asking @{}, I'm a bot to help answer some typical questions. \
                        Currently I know the following commands: !links, !schedule",
                        msg.sender.login
                    ),
                    Some(msg.message_id),
                )
                .await?;
        }
        UserResponse::Links(links) => {
            client
                .say_in_response(
                    CHANNEL.to_owned(),
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
                    Some(msg.message_id),
                )
                .await?;
        }
        UserResponse::Schedule {
            start,
            finish,
            off_days,
        } => {
            let last_off_day = off_days.len() - 1;
            let days = format!(
                "Every day, except {}",
                off_days
                    .into_iter()
                    .enumerate()
                    .fold(String::new(), |mut days, (i, day)| {
                        if i == last_off_day {
                            days.push_str(" and ");
                        } else if i > 0 {
                            days.push_str(", ");
                        }

                        days.push_str(&day);
                        days
                    })
            );
            let time = format!("Starting around {}, finishing around {}", start, finish);

            client
                .say_in_response(
                    CHANNEL.to_owned(),
                    format!("{} | {} | Timezone CET", days, time),
                    Some(msg.message_id),
                )
                .await?;
        }
        UserResponse::Unknown => {}
    }
    Ok(())
}
