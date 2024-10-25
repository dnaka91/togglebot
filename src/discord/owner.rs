use std::{fmt::Write, num::NonZeroU64};

use anyhow::Result;
use indoc::indoc;
use poise::{serenity_prelude::CreateAllowedMentions, CreateReply};

use super::Context;
use crate::{api::response::AdminAction, emojis};

pub async fn help(ctx: Context<'_>) -> Result<()> {
    ctx.reply(indoc! {"
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
        "})
        .await?;
    Ok(())
}

pub async fn admins_list(ctx: Context<'_>, user_ids: Vec<NonZeroU64>) -> Result<()> {
    let message = user_ids
        .into_iter()
        .fold(String::from("current admins are:"), |mut buf, id| {
            write!(buf, "\n- <@{id}>").unwrap();
            buf
        });

    ctx.send(
        CreateReply::default()
            .reply(true)
            .content(message)
            .allowed_mentions(CreateAllowedMentions::new()),
    )
    .await?;

    Ok(())
}

pub async fn admins_edit(ctx: Context<'_>, res: Result<AdminAction>) -> Result<()> {
    let message = match res {
        Ok(action) => format!(
            "{} user {} admin list",
            emojis::OK_HAND,
            match action {
                AdminAction::Added => "added to",
                AdminAction::Removed => "removed from",
            },
        ),
        Err(e) => format!("{} some error happened: {e}", emojis::COLLISION),
    };

    ctx.reply(message).await?;

    Ok(())
}
