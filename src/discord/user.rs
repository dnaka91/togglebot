use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use indoc::{formatdoc, indoc};
use poise::{serenity_prelude::CreateEmbed, CreateReply};
use time::{format_description::FormatItem, macros::format_description, UtcOffset};
use tracing::error;

use super::Context;
use crate::CrateSearch;

/// Gandalf's famous "You shall not pass!" scene.
const GANDALF_GIF: &str =
    "https://tenor.com/view/you-shall-not-pass-lotr-do-not-enter-not-allowed-scream-gif-16729885";

pub async fn help(ctx: Context<'_>) -> Result<()> {
    ctx.reply(indoc! {"
            Thanks for asking, I'm a bot to help answer some typical questions.
            Try out the `!commands` command to see what I can do.

            My source code is at <https://github.com/dnaka91/togglebot>
        "})
        .await?;

    Ok(())
}

pub async fn commands(ctx: Context<'_>, res: Result<Vec<String>>) -> Result<()> {
    let message = match res {
        Ok(names) => names.into_iter().enumerate().fold(
            formatdoc! {"
                    Available commands:
                    `!help` (or `!bot`) gives a short info about this bot.
                    `!ahelp` gives a list of admin commands (if you're an admin).
                    `!links` gives you a list of links to sites where **{0}** is present.
                    `!ban` refuse anything with the power of Gandalf.
                    `!crate(s)` get the link for any existing crate.
                    `!today` get details about the current day.
                    `!ftoc` convert Fahrenheit to Celsius.
                    `!ctof` convert Celsius to Fahrenheit.

                    Further custom commands:
                ",
                ctx.data().settings.streamer,
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

    ctx.reply(message).await?;

    Ok(())
}

pub async fn links(ctx: Context<'_>, links: Arc<HashMap<String, String>>) -> Result<()> {
    ctx.reply(
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
    )
    .await?;

    Ok(())
}

pub async fn ban(ctx: Context<'_>, target: String) -> Result<()> {
    ctx.reply(&format!(
        "{target}, **YOU SHALL NOT PASS!!**\n\n{GANDALF_GIF}",
    ))
    .await?;

    Ok(())
}

pub async fn crate_(ctx: Context<'_>, res: Result<CrateSearch>) -> Result<()> {
    const FORMAT: &[FormatItem<'static>] =
        format_description!("[year]-[month]-[day] [hour]:[minute] UTC");

    match res {
        Ok(search) => {
            let (content, embed) = match search {
                CrateSearch::Found(info) => (
                    String::new(),
                    CreateEmbed::new()
                        .title(format!("{} (v{})", info.name, info.newest_version))
                        .description(info.description)
                        .field(
                            "Last update",
                            info.updated_at.to_offset(UtcOffset::UTC).format(&FORMAT)?,
                            true,
                        )
                        .field(
                            "Downloads",
                            if info.downloads > 1_000_000 {
                                format!("{}+M", info.downloads / 1_000_000)
                            } else if info.downloads > 1_000 {
                                format!("{}+k", info.downloads / 1_000)
                            } else {
                                info.downloads.to_string()
                            },
                            true,
                        )
                        .field(
                            "Documentation",
                            info.documentation.unwrap_or(format!(
                                "https://docs.rs/{0}/{1}/{0}",
                                info.name, info.newest_version
                            )),
                            true,
                        )
                        .field("Repository", info.repository, true)
                        .field(
                            "More information",
                            format!("https://crates.io/crates/{0}", info.name),
                            true,
                        ),
                ),
                CrateSearch::NotFound(message) => (message, CreateEmbed::new()),
            };
            ctx.send(
                CreateReply::default()
                    .reply(true)
                    .content(content)
                    .embed(embed),
            )
            .await?;
        }
        Err(e) => {
            error!(error = ?e, "failed searching for crate");
            ctx.reply("Sorry, something went wrong looking up the crate")
                .await?;
        }
    }

    Ok(())
}

pub async fn string_reply(ctx: Context<'_>, content: String) -> Result<()> {
    ctx.reply(content).await?;
    Ok(())
}
