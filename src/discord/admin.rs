use anyhow::Result;
use twilight_http::Client;
use twilight_model::channel::Message as ChannelMessage;

use crate::{emojis, Source};

pub async fn help(msg: ChannelMessage, http: Client) -> Result<()> {
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
            letters, numbers and underscores and must not start with the `!`.\n\
            \n\
            ```\n\
            !custom_commands list\n\
            ```\n\
            List all currently available custom commands.",
        )?
        .await?;

    Ok(())
}

pub async fn schedule(msg: ChannelMessage, http: Client, res: Result<()>) -> Result<()> {
    let message = match res {
        Ok(()) => format!("{} schedule updated", emojis::OK_HAND),
        Err(e) => format!("{} some error happened: {}", emojis::COLLISION, e),
    };

    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(message)?
        .await?;

    Ok(())
}

pub async fn off_days(msg: ChannelMessage, http: Client, res: Result<()>) -> Result<()> {
    let message = match res {
        Ok(()) => format!("{} off days updated", emojis::OK_HAND),
        Err(e) => format!("{} some error happened: {}", emojis::COLLISION, e),
    };

    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(message)?
        .await?;

    Ok(())
}

pub async fn custom_commands(
    msg: ChannelMessage,
    http: Client,
    res: Result<Option<Vec<(String, Source, String)>>>,
) -> Result<()> {
    let message = match res {
        Ok(Some(list)) => list.into_iter().fold(
            String::from("available custom commands:"),
            |mut list, (name, source, content)| {
                list.push_str("\n\n`!");
                list.push_str(&name);
                list.push_str("` (");
                list.push_str(source.as_ref());
                list.push_str("):\n> ");
                list.push_str(&content);
                list
            },
        ),
        Ok(None) => format!("{} custom commands updated", emojis::OK_HAND),
        Err(e) => format!("{} some error happened: {}", emojis::COLLISION, e),
    };

    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(message)?
        .await?;

    Ok(())
}
