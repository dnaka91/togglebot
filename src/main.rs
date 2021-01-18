#![deny(rust_2018_idioms, clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]

use anyhow::Result;
use futures_util::StreamExt;
use log::{debug, error, info};
use twilight_embed_builder::{EmbedBuilder, EmbedFieldBuilder};
use twilight_gateway::{Event, EventTypeFlags, Intents, Shard};
use twilight_http::Client;
use twilight_model::channel::Message;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    std::env::set_var("RUST_LOG", "warn,togglebot=trace");

    env_logger::init();

    let token = std::env::var("DISCORD_TOKEN")?;

    let http = Client::new(&token);

    let mut shard = Shard::builder(&token, Intents::GUILD_MESSAGES | Intents::DIRECT_MESSAGES)
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

    while let Some(event) = events.next().await {
        let http = http.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_event(event, http.clone()).await {
                error!("error during event handling: {}", e);
            }
        });
    }

    Ok(())
}

async fn handle_event(event: Event, http: Client) -> Result<()> {
    match event {
        Event::MessageCreate(msg) => handle_message(&*msg, http).await?,
        Event::Ready(_) => info!("bot started, listening for events"),
        _ => {}
    }

    Ok(())
}

async fn handle_message(msg: &Message, http: Client) -> Result<()> {
    if msg.author.bot {
        // Ignore bots and our own messages.
        return Ok(());
    }

    match msg.content.as_ref() {
        "!help" => {
            info!("received `help` command");

            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content(
                    "Thanks for asking, I'm a bot to help answer some typical questions.\n\
                    Currently I only know the `!schedule` command that tells you the \
                    Twitch streaming schedule of **togglebit**.",
                )?
                .await?;
        }
        "!schedule" => {
            info!("received `schedule` command");

            let embed = EmbedBuilder::new()
                .field(EmbedFieldBuilder::new(
                    "Days",
                    "**Monday** to **Friday**, weekend is off",
                )?)
                .field(EmbedFieldBuilder::new(
                    "Time",
                    "starting around **7~8am**, finishing around **4pm**",
                )?)
                .field(EmbedFieldBuilder::new("Timezone", "CEST")?)
                .build()?;
            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content(format!(
                    "Hey <@{}>, here is the stream schedule:",
                    msg.author.id
                ))?
                .embed(embed)?
                .await?;
        }
        "!schedule long" => {
            info!("receiver `schedule long` command");

            let embed = EmbedBuilder::new()
                .field(EmbedFieldBuilder::new(
                    "Monday",
                    "start around **7~8am**, finish around **4pm**",
                )?)
                .field(EmbedFieldBuilder::new(
                    "Tuesday",
                    "start around **7~8am**, finish around **4pm**",
                )?)
                .field(EmbedFieldBuilder::new(
                    "Wednesday",
                    "start around **7~8am**, finish around **4pm**",
                )?)
                .field(EmbedFieldBuilder::new(
                    "Thursday",
                    "start around **7~8am**, finish around **4pm**",
                )?)
                .field(EmbedFieldBuilder::new(
                    "Friday",
                    "start around **7~8am**, finish around **4pm**",
                )?)
                .field(EmbedFieldBuilder::new("Saturday", "off")?)
                .field(EmbedFieldBuilder::new("Sunday", "off")?)
                .build()?;
            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content(format!(
                    "Hey <@{}>, here is the detailed stream schedule in `CEST` time:",
                    msg.author.id
                ))?
                .embed(embed)?
                .await?;
        }
        _ => debug!("message: {}", msg.content),
    }

    Ok(())
}
