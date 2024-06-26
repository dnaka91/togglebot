use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use indoc::{formatdoc, indoc};
use time::{format_description::FormatItem, macros::format_description, UtcOffset};
use tracing::error;
use twilight_http::Client;
use twilight_model::channel::Message as ChannelMessage;
use twilight_util::builder::embed::{EmbedBuilder, EmbedFieldBuilder};

use super::ExecModelExt;
use crate::{settings::Commands as CommandSettings, CrateSearch};

/// Gandalf's famous "You shall not pass!" scene.
const GANDALF_GIF: &str =
    "https://tenor.com/view/you-shall-not-pass-lotr-do-not-enter-not-allowed-scream-gif-16729885";

pub async fn help(msg: ChannelMessage, http: Arc<Client>) -> Result<()> {
    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(indoc! {"
            Thanks for asking, I'm a bot to help answer some typical questions.
            Try out the `!commands` command to see what I can do.

            My source code is at <https://github.com/dnaka91/togglebot>
        "})
        .send()
        .await?;

    Ok(())
}

pub async fn commands(
    settings: Arc<CommandSettings>,
    msg: ChannelMessage,
    http: Arc<Client>,
    res: Result<Vec<String>>,
) -> Result<()> {
    let message = match res {
        Ok(names) => names.into_iter().enumerate().fold(
            formatdoc! {"
                    Available commands:
                    `!help` (or `!bot`) gives a short info about this bot.
                    `!ahelp` gives a list of admin commands (if you're an admin).
                    `!links` gives you a list of links to sites where **{0}** is present.
                    `!ban` refuse anything with the power of Gandalf.
                    `!crate(s)` get the link for any existing crate.
                    `!doc(s)` get the link for any element of any crate (or stdlib).
                    `!today` get details about the current day.
                    `!ftoc` convert Fahrenheit to Celsius.
                    `!ctof` convert Celsius to Fahrenheit.

                    Further custom commands:
                ",
                settings.streamer,
            },
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
            error!(error = ?e, "failed listing commands");
            "Sorry, something went wrong fetching the list of commands".to_owned()
        }
    };

    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(&message)
        .send()
        .await?;

    Ok(())
}

pub async fn links(
    msg: ChannelMessage,
    http: Arc<Client>,
    links: Arc<HashMap<String, String>>,
) -> Result<()> {
    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(
            &links
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
        )
        .send()
        .await?;

    Ok(())
}

pub async fn ban(msg: ChannelMessage, http: Arc<Client>, target: String) -> Result<()> {
    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(&format!(
            "{target}, **YOU SHALL NOT PASS!!**\n\n{GANDALF_GIF}",
        ))
        .send()
        .await?;

    Ok(())
}

pub async fn crate_(
    msg: ChannelMessage,
    http: Arc<Client>,
    res: Result<CrateSearch>,
) -> Result<()> {
    const FORMAT: &[FormatItem<'static>] =
        format_description!("[year]-[month]-[day] [hour]:[minute] UTC");

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
                            info.updated_at.to_offset(UtcOffset::UTC).format(&FORMAT)?,
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
                            format!("https://crates.io/crates/{0}", info.name),
                        ))
                        .build(),
                ),
                CrateSearch::NotFound(message) => (message, EmbedBuilder::new().build()),
            };
            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content(&content)
                .embeds(&[embed])
                .send()
                .await?;
        }
        Err(e) => {
            error!(error = ?e, "failed searching for crate");
            http.create_message(msg.channel_id)
                .reply(msg.id)
                .content("Sorry, something went wrong looking up the crate")
                .send()
                .await?;
        }
    }

    Ok(())
}

pub async fn doc(msg: ChannelMessage, http: Arc<Client>, res: Result<String>) -> Result<()> {
    let message = match res {
        Ok(link) => link,
        Err(e) => {
            error!(error = ?e, "failed searching for docs");
            "Sorry, something went wrong looking up the documentation".to_owned()
        }
    };

    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(&message)
        .send()
        .await?;

    Ok(())
}

pub async fn string_reply(msg: ChannelMessage, http: Arc<Client>, content: String) -> Result<()> {
    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(&content)
        .send()
        .await?;

    Ok(())
}
