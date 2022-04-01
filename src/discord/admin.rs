use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
    sync::Arc,
};

use anyhow::Result;
use indoc::indoc;
use twilight_http::Client;
use twilight_model::channel::Message as ChannelMessage;

use super::ExecModelExt;
use crate::{emojis, statistics::Statistics, Source};

pub async fn help(msg: ChannelMessage, http: Arc<Client>) -> Result<()> {
    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(indoc! {"
            Hey there, I support the following admin commands:

            ```
            !ohelp
            ```
            Show information about available owner commands. **Only available if \
            you're an owner yourself.**

            ```
            !edit_schedule set [start|finish] <HH:MM[am|pm]> <HH:MM[am|pm]>
            ```
            Update the current schedule for either `start` or `finish` with the given \
            range in 12-hour format like `07:00am 08:00am`.

            ```
            !off_days [add|remove] <weekday>
            ```
            Update the off days by `add`ing or `remove`ing a single weekday like \
            `Mon` or `tuesday`.

            ```
            !custom_command(s) [add|remove] [all|discord|twitch] <name> <content>
            ```
            Add or remove a custom command that has fixed content and can be anything. \
            The command can be modified for all sources or individually. \
            Command names must start with a lowercase letter, only consist of lowercase \
            letters, numbers and underscores and must not start with the `!`.

            ```
            !custom_commands list
            ```
            List all currently available custom commands.

            ```
            !stats [current|total]
            ```
            Get statistics about command usage, either for the **current month** or the \
            overall counters for **all time**.
        "})?
        .send()
        .await?;

    Ok(())
}

pub async fn schedule(msg: ChannelMessage, http: Arc<Client>, res: Result<()>) -> Result<()> {
    let message = match res {
        Ok(()) => format!("{} schedule updated", emojis::OK_HAND),
        Err(e) => format!("{} some error happened: {}", emojis::COLLISION, e),
    };

    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(&message)?
        .send()
        .await?;

    Ok(())
}

pub async fn off_days(msg: ChannelMessage, http: Arc<Client>, res: Result<()>) -> Result<()> {
    let message = match res {
        Ok(()) => format!("{} off days updated", emojis::OK_HAND),
        Err(e) => format!("{} some error happened: {}", emojis::COLLISION, e),
    };

    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(&message)?
        .send()
        .await?;

    Ok(())
}

pub async fn custom_commands_list(
    msg: ChannelMessage,
    http: Arc<Client>,
    res: Result<BTreeMap<String, BTreeSet<Source>>>,
) -> Result<()> {
    let message = match res {
        Ok(list) => list.into_iter().fold(
            String::from("available custom commands:"),
            |mut list, (name, sources)| {
                list.push_str("\n`!");
                list.push_str(&name);
                list.push_str("` (");

                for (i, source) in sources.into_iter().enumerate() {
                    if i > 0 {
                        list.push_str(", ");
                    }
                    list.push_str(source.as_ref());
                }

                list.push(')');
                list
            },
        ),
        Err(e) => format!("{} some error happened: {}", emojis::COLLISION, e),
    };

    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(&message)?
        .send()
        .await?;

    Ok(())
}

pub async fn custom_commands_edit(
    msg: ChannelMessage,
    http: Arc<Client>,
    res: Result<()>,
) -> Result<()> {
    let message = match res {
        Ok(()) => format!("{} custom commands updated", emojis::OK_HAND),
        Err(e) => format!("{} some error happened: {}", emojis::COLLISION, e),
    };

    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(&message)?
        .send()
        .await?;

    Ok(())
}

pub async fn stats(
    msg: ChannelMessage,
    http: Arc<Client>,
    res: Result<(bool, Statistics)>,
) -> Result<()> {
    let message = match res {
        Ok((total, stats)) => {
            let mut message = format!(
                "Here are the statistics of {}",
                if total {
                    "all time"
                } else {
                    "the current month"
                }
            );

            message.push_str("\n\n**Built-in**");
            for (cmd, count) in stats.command_usage.builtin {
                let _ = write!(&mut message, "\n`{}`: {count}", cmd.name());
            }

            message.push_str("\n\n**Custom**");
            for (cmd, count) in stats.command_usage.custom {
                let _ = write!(&mut message, "\n`{cmd}`: {count}");
            }

            message.push_str("\n\n**Unknown**");
            for (cmd, count) in stats.command_usage.unknown {
                let _ = write!(&mut message, "\n`{cmd}`: {count}");
            }

            message
        }
        Err(e) => {
            format!("Sorry, something went wrong fetching the statistics:\n{e}")
        }
    };

    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(&message)?
        .send()
        .await?;

    Ok(())
}
