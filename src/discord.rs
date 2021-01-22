use anyhow::Result;
use futures_util::StreamExt;
use log::{error, info};
use tokio::sync::oneshot;
use twilight_embed_builder::{EmbedBuilder, EmbedFieldBuilder};
use twilight_gateway::{Event, EventTypeFlags, Intents, Shard};
use twilight_http::Client;
use twilight_model::channel::Message as ChannelMessage;

use crate::{
    emojis, settings::Discord, AdminResponse, Message, Queue, Response, Shutdown, Source,
    UserResponse,
};

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
const ADMINS: &[(&str, &str)] = &[("dnaka91", "1754"), ("ToggleBit", "0090")];

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
        UserResponse::Help => {
            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content(
                    "Thanks for asking, I'm a bot to help answer some typical questions.\n\
                    Try out the `!commands` command to see what I can do.\n\n\
                    My source code is at <https://github.com/dnaka91/togglebot>",
                )?
                .await?;
        }
        UserResponse::Commands => {
            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content(
                    "Available commands:\n\
                    `!help` gives a short info about this bot.\n\
                    `!lark` tells **togglebit** that he's a lark.\n\
                    `!links` gives you a list of links to sites where **togglebit** is present.\n\
                    `!schedule` tells you the Twitch streaming schedule of **togglebit**.",
                )?
                .await?;
        }
        UserResponse::Lark => {
            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content("Oh ToggleBit, you lark!")?
                .await?;
        }
        UserResponse::Links(links) => {
            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content(links.iter().enumerate().fold(
                    String::new(),
                    |mut list, (i, (name, url))| {
                        if i > 0 {
                            list.push('\n');
                        }

                        list.push_str(name);
                        list.push_str(": <");
                        list.push_str(url);
                        list.push('>');
                        list
                    },
                ))?
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

                        days.push_str("**");
                        days.push_str(&day);
                        days.push_str("**");
                        days
                    })
            );
            let time = format!(
                "starting around **{}**, finishing around **{}**",
                start, finish
            );

            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content("Here is togglebit's stream schedule:")?
                .embed(
                    EmbedBuilder::new()
                        .field(EmbedFieldBuilder::new("Days", days)?)
                        .field(EmbedFieldBuilder::new("Time", time)?)
                        .field(EmbedFieldBuilder::new("Timezone", "CET")?)
                        .build()?,
                )?
                .await?;
        }
        UserResponse::Custom(content) => {
            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content(content)?
                .await?;
        }
        UserResponse::Unknown => {}
    }

    Ok(())
}

async fn handle_admin_message(
    resp: AdminResponse,
    msg: ChannelMessage,
    http: Client,
) -> Result<()> {
    match resp {
        AdminResponse::Help => {
            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content(
                    "Hey there, I support the following admin commands:\n\
                    \n\
                    ```\n\
                    !schedule set [start|finish] <HH:MM[am|pm]> <HH:MM[am|pm]>\n\
                    ```\n\
                    Update the current schedule for either `start` or `finish` with the given \
                    range in 12-hour format like `07:00am 08:00am`.\n\
                    \n\
                    ```\n\
                    !off_days [add|remove] <weekday>\n\
                    ```\n\
                    Update the off days by `add`ing or `remove`ing a single weekday like \
                    `Mon` or `tuesday`.
                    \n\
                    ```\n\
                    !custom_commands [add|remove] [all|discord|twitch] <name> <content>\n\
                    ```\n\
                    Add or remove a custom command that has fixed content and can be anything. \
                    The command can be modified for all sources or individually. \
                    Command names must start with a lowercase letter, only consist of lowercase \
                    letters, numbers and underscores and must not start with the `!`.",
                )?
                .await?;
        }
        AdminResponse::Schedule(res) => {
            let message = match res {
                Ok(()) => format!("{} schedule updated", emojis::OK_HAND),
                Err(e) => format!("{} some error happened: {}", emojis::COLLISION, e),
            };

            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content(message)?
                .await?;
        }
        AdminResponse::OffDays(res) => {
            let message = match res {
                Ok(()) => format!("{} off days updated", emojis::OK_HAND),
                Err(e) => format!("{} some error happened: {}", emojis::COLLISION, e),
            };

            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content(message)?
                .await?;
        }
        AdminResponse::CustomCommands(res) => {
            let message = match res {
                Ok(()) => format!("{} custom commands updated", emojis::OK_HAND),
                Err(e) => format!("{} some error happened: {}", emojis::COLLISION, e),
            };

            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content(message)?
                .await?;
        }
        AdminResponse::Unknown => {}
    }

    Ok(())
}
