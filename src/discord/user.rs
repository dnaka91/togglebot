use anyhow::Result;
use indoc::indoc;
use log::error;
use twilight_embed_builder::{EmbedBuilder, EmbedFieldBuilder};
use twilight_http::Client;
use twilight_model::channel::Message as ChannelMessage;

use crate::CrateSearch;

/// Gandalf's famous "You shall not pass!" scene.
const GANDALF_GIF: &str =
    "https://tenor.com/view/you-shall-not-pass-lotr-do-not-enter-not-allowed-scream-gif-16729885";

pub async fn help(msg: ChannelMessage, http: Client) -> Result<()> {
    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(indoc! {"
            Thanks for asking, I'm a bot to help answer some typical questions.
            Try out the `!commands` command to see what I can do.

            My source code is at <https://github.com/dnaka91/togglebot>
        "})?
        .await?;

    Ok(())
}

pub async fn commands(msg: ChannelMessage, http: Client, res: Result<Vec<String>>) -> Result<()> {
    let message = match res {
        Ok(names) => names.into_iter().enumerate().fold(
            String::from(indoc! {"
                    Available commands:
                    `!help` (or `!bot`) gives a short info about this bot.
                    `!lark` tells **togglebit** that he's a lark.
                    `!links` gives you a list of links to sites where **togglebit** is present.
                    `!schedule` tells you the Twitch streaming schedule of **togglebit**.
                    `!crate` get the link for any existing crate.
                    `!ban` refuse anything with the power of Gandalf.

                    Further custom commands:
                "}),
            |mut list, (i, name)| {
                if i > 0 {
                    list.push_str(", ");
                }
                list.push_str("`!");
                list.push_str(&name);
                list.push('`');
                list
            },
        ),
        Err(e) => {
            error!("failed listing commands: {}", e);
            "Sorry, something went wrong fetching the list of commands".to_owned()
        }
    };

    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(message)?
        .await?;

    Ok(())
}

pub async fn links(msg: ChannelMessage, http: Client, links: &[(&str, &str)]) -> Result<()> {
    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(
            links
                .iter()
                .enumerate()
                .fold(String::new(), |mut list, (i, (name, url))| {
                    if i > 0 {
                        list.push('\n');
                    }

                    list.push_str(name);
                    list.push_str(": <");
                    list.push_str(url);
                    list.push('>');
                    list
                }),
        )?
        .await?;

    Ok(())
}

pub async fn schedule(
    msg: ChannelMessage,
    http: Client,
    start: String,
    finish: String,
    off_days: Vec<String>,
) -> Result<()> {
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
        .embeds([EmbedBuilder::new()
            .field(EmbedFieldBuilder::new("Days", days))
            .field(EmbedFieldBuilder::new("Time", time))
            .field(EmbedFieldBuilder::new("Timezone", "CET"))
            .build()?])?
        .await?;

    Ok(())
}
pub async fn ban(msg: ChannelMessage, http: Client, target: String) -> Result<()> {
    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(format!(
            "{}, **YOU SHALL NOT PASS!!**\n\n{}",
            target, GANDALF_GIF
        ))?
        .await?;

    Ok(())
}

pub async fn crate_(msg: ChannelMessage, http: Client, res: Result<CrateSearch>) -> Result<()> {
    match res {
        Ok(search) => {
            let (content, embed) = match search {
                CrateSearch::Found(info) => (
                    String::new(),
                    EmbedBuilder::new()
                        .title(format!("{} (v{})", info.name, info.newest_version))
                        .description(info.description)
                        .field(EmbedFieldBuilder::new(
                            "Last update",
                            info.updated_at
                                .naive_utc()
                                .format("%Y-%m-%d %H:%M UTC")
                                .to_string(),
                        ))
                        .field(EmbedFieldBuilder::new(
                            "Downloads",
                            if info.downloads > 1_000_000 {
                                format!("{}+M", info.downloads / 1_000_000)
                            } else if info.downloads > 1_000 {
                                format!("{}+k", info.downloads / 1_000)
                            } else {
                                info.downloads.to_string()
                            },
                        ))
                        .field(EmbedFieldBuilder::new(
                            "Documentation",
                            info.documentation.unwrap_or(format!(
                                "https://docs.rs/{0}/{1}/{0}",
                                info.name, info.newest_version
                            )),
                        ))
                        .field(EmbedFieldBuilder::new("Repository", info.repository))
                        .field(EmbedFieldBuilder::new(
                            "More information",
                            format!(
                                "https://lib.rs/crates/{0} or\nhttps://crates.io/crates/{0}",
                                info.name
                            ),
                        ))
                        .build()?,
                ),
                CrateSearch::NotFound(message) => (message, EmbedBuilder::new().build()?),
            };
            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content(content)?
                .embeds([embed])?
                .await?;
        }
        Err(e) => {
            error!("failed searching for crate: {}", e);
            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content("Sorry, something went wrong looking up the crate")?
                .await?;
        }
    }

    Ok(())
}

pub async fn custom(msg: ChannelMessage, http: Client, content: String) -> Result<()> {
    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(content)?
        .await?;

    Ok(())
}
