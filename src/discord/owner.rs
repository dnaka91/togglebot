use std::{num::NonZeroU64, sync::Arc};

use anyhow::Result;
use indoc::indoc;
use twilight_http::Client;
use twilight_model::channel::Message as ChannelMessage;

use super::ExecModelExt;
use crate::{emojis, AdminAction};

pub async fn help(msg: ChannelMessage, http: Arc<Client>) -> Result<()> {
    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(indoc! {"
            Hey there, I support the following owner commands:

            ```
            !admin(s) [add|remove] @name
            ```
            Add or remove a user to/from the admin list. An admin has access to most of \
            the bot-controlling commands.

            ```
            !admin(s) list
            ```
            List all currently configured admin users.
        "})?
        .send()
        .await?;

    Ok(())
}

pub async fn admins_list(
    msg: ChannelMessage,
    http: Arc<Client>,
    user_ids: Vec<NonZeroU64>,
) -> Result<()> {
    let message = user_ids
        .into_iter()
        .fold(String::from("current admins are:"), |mut buf, id| {
            buf.push_str(&format!("\n- <@{}>", id));
            buf
        });

    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(&message)?
        .send()
        .await?;

    Ok(())
}

pub async fn admins_edit(
    msg: ChannelMessage,
    http: Arc<Client>,
    res: Result<AdminAction>,
) -> Result<()> {
    let message = match res {
        Ok(action) => format!(
            "{} user {} admin list",
            emojis::OK_HAND,
            match action {
                AdminAction::Added => "added to",
                AdminAction::Removed => "removed from",
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
