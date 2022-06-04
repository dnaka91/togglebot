//! Twitch service connector that allows to receive commands from Twitch channels.

use anyhow::Result;
use tokio::{select, sync::oneshot};
use tokio_shutdown::Shutdown;
use tracing::{error, info};
use twitch_irc::{
    login::StaticLoginCredentials,
    message::{PrivmsgMessage, ServerMessage},
    ClientConfig, SecureTCPTransport, TwitchIRCClient,
};

use crate::{
    settings::Twitch, AuthorId, CrateSearch, Message, Queue, Response, ScheduleResponse, Source,
    UserResponse,
};

type Client = TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>;

const CHANNEL: &str = "togglebit";

/// Initialize and run the Twitch connection in a background task.
///
/// The given queue is used to transfer received messages for further processing, combined with a
/// oneshot channel to listen for any possible replies to a message. The shutdown handle is used
/// to gracefully disconnect from Twitch, before fully quitting the application.
#[allow(clippy::missing_panics_doc)]
pub async fn start(config: &Twitch, queue: Queue, shutdown: Shutdown) -> Result<()> {
    let config = ClientConfig::new_simple(StaticLoginCredentials::new(
        config.login.clone(),
        Some(config.token.clone()),
    ));
    let (mut messages, client) = Client::new(config);

    client.join(CHANNEL.to_owned())?;

    tokio::spawn(async move {
        loop {
            select! {
                _ = shutdown.handle() => break,
                message = messages.recv() => {
                    if let Some(message) = message {
                        let client = client.clone();
                        let queue = queue.clone();

                        tokio::spawn(async move {
                            if let Err(e) = handle_server_message(queue, message, client).await {
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
        author: AuthorId::Twitch(msg.sender.id),
        mention: None,
    };
    let (tx, rx) = oneshot::channel();

    if queue.send((message, tx)).await.is_ok() {
        if let Ok(resp) = rx.await {
            match resp {
                Response::User(user_resp) => {
                    handle_user_message(user_resp, msg.message_id, client).await?;
                }
                Response::Admin(_) | Response::Owner(_) => {}
            }
        }
    }

    Ok(())
}

#[allow(clippy::match_same_arms)]
async fn handle_user_message(resp: UserResponse, msg_id: String, client: Client) -> Result<()> {
    match resp {
        UserResponse::Help => handle_help(msg_id, client).await,
        UserResponse::Commands(res) => handle_commands(msg_id, client, res).await,
        UserResponse::Links(links) => handle_links(msg_id, client, links).await,
        UserResponse::Schedule(res) => handle_schedule(msg_id, client, res).await,
        UserResponse::Ban(target) => handle_ban(msg_id, client, target).await,
        UserResponse::Crate(res) => handle_crate(msg_id, client, res).await,
        UserResponse::Doc(res) => handle_doc(msg_id, client, res).await,
        UserResponse::Today(date) => handle_today(msg_id, client, date).await,
        UserResponse::Custom(content) => handle_custom(msg_id, client, content).await,
        UserResponse::Unknown => Ok(()),
    }
}

async fn handle_help(msg_id: String, client: Client) -> Result<()> {
    client
        .say_in_response(
            CHANNEL.to_owned(),
            "Thanks for asking, I'm a bot to help answer some typical questions. \
            Try out `!commands` command to see what I can do. \
            My source code is at https://github.com/dnaka91/togglebot"
                .to_owned(),
            Some(msg_id),
        )
        .await?;

    Ok(())
}

async fn handle_commands(msg_id: String, client: Client, res: Result<Vec<String>>) -> Result<()> {
    let message = match res {
        Ok(names) => names.into_iter().fold(
            String::from(
                "Available commands: \
                !help (or !bot), \
                !links, !schedule, \
                !crate(s), \
                !doc(s), \
                !ban",
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
        .say_in_response(CHANNEL.to_owned(), message, Some(msg_id))
        .await?;

    Ok(())
}

async fn handle_links(msg_id: String, client: Client, links: &[(&str, &str)]) -> Result<()> {
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
            Some(msg_id),
        )
        .await?;

    Ok(())
}

async fn handle_schedule(
    msg_id: String,
    client: Client,
    res: Result<ScheduleResponse>,
) -> Result<()> {
    let message = match res {
        Ok(ScheduleResponse {
            start,
            finish,
            off_days,
        }) => {
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

            format!("{} | {} | Timezone CET", days, time)
        }
        Err(e) => {
            error!(error = ?e, "failed creating schedule response");
            "Sorry, something went wrong while getting the schedule".to_owned()
        }
    };

    client
        .say_in_response(CHANNEL.to_owned(), message, Some(msg_id))
        .await?;

    Ok(())
}

async fn handle_ban(msg_id: String, client: Client, target: String) -> Result<()> {
    client
        .say_in_response(
            CHANNEL.to_owned(),
            format!("{}, YOU SHALL NOT PASS!!", target),
            Some(msg_id),
        )
        .await?;

    Ok(())
}

async fn handle_crate(msg_id: String, client: Client, res: Result<CrateSearch>) -> Result<()> {
    let message = match res {
        Ok(search) => match search {
            CrateSearch::Found(info) => format!("https://lib.rs/crates/{}", info.name),
            CrateSearch::NotFound(message) => message,
        },
        Err(e) => {
            error!(error = ?e, "failed searching for crate");
            "Sorry, something went wrong looking up the crate".to_owned()
        }
    };

    client
        .say_in_response(CHANNEL.to_owned(), message, Some(msg_id))
        .await?;

    Ok(())
}

async fn handle_doc(msg_id: String, client: Client, res: Result<String>) -> Result<()> {
    let message = match res {
        Ok(link) => link,
        Err(e) => {
            error!(error = ?e, "failed searching for docs");
            "Sorry, something went wrong looking up the documentation".to_owned()
        }
    };

    client
        .say_in_response(CHANNEL.to_owned(), message, Some(msg_id))
        .await?;

    Ok(())
}

async fn handle_today(msg_id: String, client: Client, date: String) -> Result<()> {
    client
        .say_in_response(CHANNEL.to_owned(), date, Some(msg_id))
        .await?;

    Ok(())
}

async fn handle_custom(msg_id: String, client: Client, content: String) -> Result<()> {
    client
        .say_in_response(CHANNEL.to_owned(), content, Some(msg_id))
        .await?;

    Ok(())
}
