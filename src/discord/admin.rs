use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};

use anyhow::Result;
use indoc::indoc;

use super::Context;
use crate::{api::Source, emojis, statistics::Statistics};

pub async fn help(ctx: Context<'_>) -> Result<()> {
    ctx.reply(indoc! {"
            Hey there, I support the following admin commands:

            ```
            !ohelp
            ```
            Show information about available owner commands. **Only available if \
            you're an owner yourself.**

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
        "})
        .await?;

    Ok(())
}

pub async fn custom_commands_list(
    ctx: Context<'_>,
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
        Err(e) => format!("{} some error happened: {e}", emojis::COLLISION),
    };

    ctx.reply(message).await?;

    Ok(())
}

pub async fn custom_commands_edit(ctx: Context<'_>, res: Result<()>) -> Result<()> {
    let message = match res {
        Ok(()) => format!("{} custom commands updated", emojis::OK_HAND),
        Err(e) => format!("{} some error happened: {e}", emojis::COLLISION),
    };

    ctx.reply(message).await?;

    Ok(())
}

pub async fn stats(ctx: Context<'_>, res: Result<(bool, Statistics)>) -> Result<()> {
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
                write!(&mut message, "\n`{}`: {count}", cmd.name()).ok();
            }

            message.push_str("\n\n**Custom**");
            for (cmd, count) in stats.command_usage.custom {
                write!(&mut message, "\n`{cmd}`: {count}").ok();
            }

            message.push_str("\n\n**Unknown**");
            for (cmd, count) in stats.command_usage.unknown {
                write!(&mut message, "\n`{cmd}`: {count}").ok();
            }

            message
        }
        Err(e) => {
            format!("Sorry, something went wrong fetching the statistics:\n{e}")
        }
    };

    ctx.reply(message).await?;

    Ok(())
}
