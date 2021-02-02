use anyhow::Result;
use log::error;
use twilight_embed_builder::{EmbedBuilder, EmbedFieldBuilder};
use twilight_http::Client;
use twilight_model::channel::Message as ChannelMessage;

pub async fn help(msg: ChannelMessage, http: Client) -> Result<()> {
    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(
            "Thanks for asking, I'm a bot to help answer some typical questions.\n\
        Try out the `!commands` command to see what I can do.\n\n\
        My source code is at <https://github.com/dnaka91/togglebot>",
        )?
        .await?;

    Ok(())
}

pub async fn commands(msg: ChannelMessage, http: Client, res: Result<Vec<String>>) -> Result<()> {
    let message = match res {
        Ok(names) => names.into_iter().enumerate().fold(
            String::from(
                "Available commands:\n\
                `!help` (or `!bot`) gives a short info about this bot.\n\
                `!lark` tells **togglebit** that he's a lark.\n\
                `!links` gives you a list of links to sites where **togglebit** is present.\n\
                `!schedule` tells you the Twitch streaming schedule of **togglebit**.\n\
                \n\
                Further custom commands:\n",
            ),
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
        .embed(
            EmbedBuilder::new()
                .field(EmbedFieldBuilder::new("Days", days)?)
                .field(EmbedFieldBuilder::new("Time", time)?)
                .field(EmbedFieldBuilder::new("Timezone", "CET")?)
                .build()?,
        )?
        .await?;

    Ok(())
}

pub async fn custom(msg: ChannelMessage, http: Client, content: String) -> Result<()> {
    http.create_message(msg.channel_id)
        .reply(msg.id)
        .content(content)?
        .await?;

    Ok(())
}
