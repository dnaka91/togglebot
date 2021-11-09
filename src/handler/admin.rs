use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

use anyhow::{anyhow, bail, ensure, Result};
use chrono::{NaiveTime, Weekday};
use tracing::info;

use super::AsyncState;
use crate::{settings, AdminResponse, CustomCommandsResponse, Source};

pub fn help() -> AdminResponse {
    info!("admin: received `help` command");
    AdminResponse::Help
}

pub async fn schedule(
    state: AsyncState,
    field: &str,
    range_begin: &str,
    range_end: &str,
) -> AdminResponse {
    info!("admin: received `schedule` command");

    let res = || async {
        update_schedule(
            state,
            field.parse()?,
            (
                NaiveTime::parse_from_str(range_begin, "%I:%M%P")?,
                NaiveTime::parse_from_str(range_end, "%I:%M%P")?,
            ),
        )
        .await
    };

    AdminResponse::Schedule(res().await)
}

enum Field {
    Start,
    Finish,
}

impl FromStr for Field {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "start" | "begin" => Self::Start,
            "finish" | "end" => Self::Finish,
            s => bail!("unknown field `{}`", s),
        })
    }
}

async fn update_schedule(
    state: AsyncState,
    field: Field,
    range: (NaiveTime, NaiveTime),
) -> Result<()> {
    let mut state = state.write().await;
    match field {
        Field::Start => state.schedule.start = range,
        Field::Finish => state.schedule.finish = range,
    }

    settings::save_state(&*state).await?;

    Ok(())
}

pub async fn off_days(state: AsyncState, action: &str, weekday: &str) -> AdminResponse {
    info!("admin: received `off_days` command");

    let res = || async {
        update_off_days(
            state,
            action.parse()?,
            weekday
                .parse()
                .map_err(|_e| anyhow!("unknown weekday `{}`", weekday))?,
        )
        .await
    };

    AdminResponse::OffDays(res().await)
}

enum Action {
    Add,
    Remove,
}

impl FromStr for Action {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "add" => Self::Add,
            "remove" => Self::Remove,
            s => bail!("unknown action `{}`", s),
        })
    }
}

async fn update_off_days(state: AsyncState, action: Action, weekday: Weekday) -> Result<()> {
    let mut state = state.write().await;
    match action {
        Action::Add => {
            state.off_days.insert(weekday);
        }
        Action::Remove => {
            state.off_days.remove(&weekday);
        }
    }

    settings::save_state(&*state).await?;

    Ok(())
}

pub async fn custom_commands_list(state: AsyncState) -> AdminResponse {
    info!("admin: received `custom_commands list` command");
    AdminResponse::CustomCommands(CustomCommandsResponse::List(list_commands(state).await))
}

async fn list_commands(state: AsyncState) -> Result<BTreeMap<String, BTreeSet<Source>>> {
    Ok(state
        .read()
        .await
        .custom_commands
        .iter()
        .map(|(name, sources)| {
            (
                name.clone(),
                sources.iter().map(|(source, _)| *source).collect(),
            )
        })
        .collect())
}

pub async fn custom_commands(
    state: AsyncState,
    content: &str,
    action: &str,
    source: &str,
    name: &str,
) -> AdminResponse {
    info!("admin: received `custom_commands` command");

    let content = content
        .splitn(5, char::is_whitespace)
        .filter(|c| !c.is_empty())
        .nth(4);

    let res =
        || async { update_commands(state, action.parse()?, source.parse()?, name, content).await };

    AdminResponse::CustomCommands(CustomCommandsResponse::Edit(res().await))
}

enum CommandSource {
    Source(Source),
    All,
}

impl FromStr for CommandSource {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "discord" => Self::Source(Source::Discord),
            "twitch" => Self::Source(Source::Twitch),
            "all" => Self::All,
            _ => bail!("unkown source `{}`", s),
        })
    }
}

const RESERVED_COMMANDS: &[&str] = &["help", "bot", "commands", "links", "schedule", "ban"];

async fn update_commands(
    state: AsyncState,
    action: Action,
    source: CommandSource,
    name: &str,
    content: Option<&str>,
) -> Result<()> {
    ensure!(
        !name.starts_with('!'),
        "command names must not start with an `!`",
    );
    ensure!(
        name.starts_with(|c| ('a'..='z').contains(&c)),
        "command names must start with a lowercase letter",
    );
    ensure!(
        name.chars()
            .all(|c| c == '_' || ('a'..='z').contains(&c) || ('0'..='9').contains(&c)),
        "command names must constist of only letters, numbers and underscores",
    );
    ensure!(
        !RESERVED_COMMANDS.contains(&name),
        "the command name `{}` is reserved",
        name,
    );

    let mut state = state.write().await;
    match action {
        Action::Add => {
            if let Some(content) = content {
                match source {
                    CommandSource::Source(source) => {
                        state
                            .custom_commands
                            .entry(name.to_owned())
                            .or_default()
                            .insert(source, content.to_owned());
                    }
                    CommandSource::All => {
                        let entry = state.custom_commands.entry(name.to_owned()).or_default();

                        for source in &[Source::Discord, Source::Twitch] {
                            entry.insert(*source, content.to_owned());
                        }
                    }
                }
            } else {
                bail!("no content for the command provided");
            }
        }
        Action::Remove => match source {
            CommandSource::Source(source) => {
                if let Some(entry) = state.custom_commands.get_mut(name) {
                    entry.remove(&source);
                }
            }
            CommandSource::All => {
                state.custom_commands.remove(name);
            }
        },
    }

    settings::save_state(&state).await?;

    Ok(())
}
