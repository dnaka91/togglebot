#![deny(rust_2018_idioms, clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::map_err_ignore)]

use std::{str::FromStr, sync::Arc};

use anyhow::{anyhow, bail, Result};
use chrono::prelude::*;
use futures_util::StreamExt;
use log::{debug, error, info, warn};
use togglebot::{
    emojis,
    settings::{self, State},
};
use tokio::sync::RwLock;
use twilight_embed_builder::{EmbedBuilder, EmbedFieldBuilder};
use twilight_gateway::{Event, EventTypeFlags, Intents, Shard};
use twilight_http::Client;
use twilight_model::channel::Message;

type AsyncState = Arc<RwLock<State>>;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    std::env::set_var("RUST_LOG", "warn,togglebot=trace");
    env_logger::init();

    let config = settings::load_config().await?;
    let state = settings::load_state().await?;

    let http = Client::new(&config.discord.token);

    let mut shard = Shard::builder(
        &config.discord.token,
        Intents::GUILD_MESSAGES | Intents::DIRECT_MESSAGES,
    )
    .http_client(http.clone())
    .build();

    shard.start().await?;

    let shard_spawn = shard.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();

        info!("bot shutting down");
        shard_spawn.shutdown();
    });

    let mut events = shard.some_events(EventTypeFlags::READY | EventTypeFlags::MESSAGE_CREATE);
    let state = Arc::new(RwLock::new(state));

    while let Some(event) = events.next().await {
        let state = state.clone();
        let http = http.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_event(state, event, http.clone()).await {
                error!("error during event handling: {}", e);
            }
        });
    }

    Ok(())
}

async fn handle_event(state: AsyncState, event: Event, http: Client) -> Result<()> {
    match event {
        Event::MessageCreate(msg) => handle_message(state, &*msg, http).await?,
        Event::Ready(_) => info!("bot started, listening for events"),
        _ => {}
    }

    Ok(())
}

async fn handle_message(state: AsyncState, msg: &Message, http: Client) -> Result<()> {
    if msg.author.bot {
        // Ignore bots and our own messages.
        return Ok(());
    }

    if msg.guild_id.is_some() {
        handle_guild_message(state, msg, http).await
    } else {
        handle_direct_message(state, msg, http).await
    }
}

async fn handle_guild_message(state: AsyncState, msg: &Message, http: Client) -> Result<()> {
    match msg.content.as_ref() {
        "!help" => {
            info!("guild: received `help` command");

            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content(
                    "Thanks for asking, I'm a bot to help answer some typical questions.\n\
                    Currently I know the following commands:\n\
                    `!links` gives you a list of links to sites where **togglebit** is present.\n\
                    `!schedule` tells you the Twitch streaming schedule of **togglebit**.",
                )?
                .await?;
        }
        "!links" => {
            info!("guild: received `links` command");

            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content(
                    "Website: <https://togglebit.io>\n\
                    GitHub: <https://github.com/togglebyte>\n\
                    Twitch: <https://twitch.tv/togglebit>\n\
                    Discord: <https://discord.gg/qtyDMat>",
                )?
                .await?;
        }
        "!schedule" => {
            info!("guild: received `schedule` command");

            let (days, time) = async {
                let state = state.read().await;
                let last_off_day = state.off_days.len() - 1;

                (
                    format!(
                        "Every day, except {}",
                        state.off_days.iter().enumerate().fold(
                            String::new(),
                            |mut days, (i, day)| {
                                if i == last_off_day {
                                    days.push_str(" and ")
                                } else if i > 0 {
                                    days.push_str(", ")
                                }

                                days.push_str("**");
                                days.push_str(match day {
                                    Weekday::Mon => "Monday",
                                    Weekday::Tue => "Tuesday",
                                    Weekday::Wed => "Wednesday",
                                    Weekday::Thu => "Thursday",
                                    Weekday::Fri => "Friday",
                                    Weekday::Sat => "Saturday",
                                    Weekday::Sun => "Sunday",
                                });
                                days.push_str("**");
                                days
                            }
                        )
                    ),
                    state.schedule.format(),
                )
            }
            .await;

            let embed = EmbedBuilder::new()
                .field(EmbedFieldBuilder::new("Days", days)?)
                .field(EmbedFieldBuilder::new("Time", time)?)
                .field(EmbedFieldBuilder::new("Timezone", "CET")?)
                .build()?;
            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content("Here is togglebit's stream schedule:")?
                .embed(embed)?
                .await?;
        }
        _ => debug!("guild: message: {}", msg.content),
    }

    Ok(())
}

/// List of admins that are allowed to customize the bot. Currently static and will be added to the
/// settings in the future.
const ADMINS: &[(&str, &str)] = &[("dnaka91", "1754"), ("ToggleBit", "0090")];

async fn handle_direct_message(state: AsyncState, msg: &Message, http: Client) -> Result<()> {
    if !ADMINS.contains(&(&msg.author.name, &msg.author.discriminator)) {
        // Ignore commands from any unauthorized users.
        return Ok(());
    }

    let mut parts = msg.content.split_whitespace();
    let command = if let Some(cmd) = parts.next() {
        cmd
    } else {
        warn!("direct: got message without content");
        return Ok(());
    };

    match (
        command,
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
    ) {
        ("!help", None, None, None, None) => {
            info!("direct: received `help` command");

            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content(
                    "Hey there, I support the following admin commands:\n\
                    ```\n\
                    !schedule set [start|finish] <HH:MM[am|pm]> <HH:MM[am|pm]>\n\
                    ```\n\
                    Update the current schedule for either `start` or `finish` with the given \
                    range in 12-hour format like `07:00am 08:00am`.\n\
                    ```\n\
                    !off_days [add|remove] <weekday>\n\
                    ```\n\
                    Update the off days by `add`ing or `remove`ing a single weekday like \
                    `Mon` or `tuesday`.",
                )?
                .await?;
        }
        ("!schedule", Some("set"), Some(field), Some(range_begin), Some(range_end)) => {
            info!("direct: received `schedule` command");

            let res = || async {
                update_schedule(
                    state,
                    field.parse()?,
                    (
                        NaiveTime::parse_from_str(range_begin, "%I:%M%P")?,
                        NaiveTime::parse_from_str(range_end, "%I:%M%P")?,
                    ),
                )
                .await
            };

            let message = match res().await {
                Ok(()) => format!("{} schedule updated", emojis::OK_HAND),
                Err(e) => format!("{} some error happened: {}", emojis::COLLISION, e),
            };

            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content(message)?
                .await?;
        }
        ("!off_days", Some(action), Some(weekday), None, None) => {
            info!("direct: received `off_days` command");

            let res = || async {
                update_off_days(
                    state,
                    action.parse()?,
                    weekday
                        .parse()
                        .map_err(|_| anyhow!("unknown weekday `{}`", weekday))?,
                )
                .await
            };

            let message = match res().await {
                Ok(()) => format!("{} off days updated", emojis::OK_HAND),
                Err(e) => format!("{} some error happened: {}", emojis::COLLISION, e),
            };

            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content(message)?
                .await?;
        }
        _ => {}
    }

    Ok(())
}

enum Field {
    Start,
    Finish,
}

impl FromStr for Field {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "start" | "begin" => Self::Start,
            "finish" | "end" => Self::Finish,
            s => bail!("unknown field `{}`", s),
        })
    }
}

async fn update_schedule(
    state: AsyncState,
    field: Field,
    range: (NaiveTime, NaiveTime),
) -> Result<()> {
    let mut state = state.write().await;
    match field {
        Field::Start => state.schedule.start = range,
        Field::Finish => state.schedule.finish = range,
    }

    settings::save_state(&*state).await?;

    Ok(())
}

enum Action {
    Add,
    Remove,
}

impl FromStr for Action {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "add" => Self::Add,
            "remove" => Self::Remove,
            s => bail!("unknown action `{}`", s),
        })
    }
}

async fn update_off_days(state: AsyncState, action: Action, weekday: Weekday) -> Result<()> {
    let mut state = state.write().await;
    match action {
        Action::Add => {
            state.off_days.insert(weekday);
        }
        Action::Remove => {
            state.off_days.remove(&weekday);
        }
    }

    settings::save_state(&*state).await?;

    Ok(())
}
