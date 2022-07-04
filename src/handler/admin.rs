use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

use anyhow::{anyhow, bail, ensure, Result};
use time::{Time, Weekday};
use tracing::info;

use super::{AsyncState, AsyncStats};
use crate::{
    state::{self, SCHEDULE_TIME_FORMAT},
    AdminResponse, CustomCommandsResponse, Source,
};

pub fn help() -> AdminResponse {
    info!("received `help` command");
    AdminResponse::Help
}

pub async fn schedule(
    state: AsyncState,
    field: &str,
    range_begin: &str,
    range_end: &str,
) -> AdminResponse {
    info!("received `schedule` command");

    let res = || async {
        update_schedule(
            state,
            field.parse()?,
            (
                Time::parse(range_begin, &SCHEDULE_TIME_FORMAT)?,
                Time::parse(range_end, &SCHEDULE_TIME_FORMAT)?,
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
            s => bail!("unknown field `{s}`"),
        })
    }
}

async fn update_schedule(state: AsyncState, field: Field, range: (Time, Time)) -> Result<()> {
    let mut state = state.write().await;
    match field {
        Field::Start => state.schedule.start = range,
        Field::Finish => state.schedule.finish = range,
    }

    state::save(&*state).await?;

    Ok(())
}

pub async fn off_days(state: AsyncState, action: &str, weekday: &str) -> AdminResponse {
    info!("received `off_days` command");

    let res = || async {
        update_off_days(
            state,
            action.parse()?,
            weekday
                .parse()
                .map_err(|_e| anyhow!("unknown weekday `{weekday}`"))?,
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
            s => bail!("unknown action `{s}`"),
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

    state::save(&*state).await?;

    Ok(())
}

pub async fn custom_commands_list(state: AsyncState) -> AdminResponse {
    info!("received `custom_commands list` command");
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
    statistics: AsyncStats,
    content: &str,
    action: &str,
    source: &str,
    name: &str,
) -> AdminResponse {
    info!("received `custom_commands` command");

    let content = content
        .splitn(5, char::is_whitespace)
        .filter(|c| !c.is_empty())
        .nth(4);

    let res = || async {
        update_commands(
            state,
            statistics,
            action.parse()?,
            source.parse()?,
            name,
            content,
        )
        .await
    };

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
            _ => bail!("unkown source `{s}`"),
        })
    }
}

/// List of all pre-defined commands that can not be defined as name for custom commands.
///
/// As custom commands are checked last, there is no chance of accidentally hiding the other
/// commands, but refusing these names helps to avoid confusion about commands not being triggered.
const RESERVED_COMMANDS: &[&str] = &[
    // user commands
    "help",
    "bot",
    "commands",
    "links",
    "schedule",
    "crate",
    "crates",
    "doc",
    "docs",
    "ban",
    "today",
    // admin commands
    "admin_help",
    "admin-help",
    "adminhelp",
    "ahelp",
    "edit_schedule",
    "off_days",
    "custom_commands",
    "custom_command",
    "stats",
    // owner commands
    "owner_help",
    "owner-help",
    "ownerhelp",
    "ohelp",
    "admins",
    "admin",
];

async fn update_commands(
    state: AsyncState,
    statistics: AsyncStats,
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
        "command names must consist of only letters, numbers and underscores",
    );
    ensure!(
        !RESERVED_COMMANDS.contains(&name),
        "the command name `{name}` is reserved",
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
        Action::Remove => {
            match source {
                CommandSource::Source(source) => {
                    if let Some(entry) = state.custom_commands.get_mut(name) {
                        entry.remove(&source);
                    }
                }
                CommandSource::All => {
                    state.custom_commands.remove(name);
                }
            }

            statistics.write().await.erase_custom(name);
        }
    }

    state::save(&state).await?;

    Ok(())
}

pub async fn stats(stats: AsyncStats, date: Option<&str>) -> AdminResponse {
    let res = || async {
        let total = date
            .map(|r| {
                Ok(match r {
                    "total" => true,
                    "current" => false,
                    _ => bail!("invalid range `{r}`, possible values are `total` or `current`"),
                })
            })
            .transpose()?
            .unwrap_or_default();

        Ok((total, stats.write().await.get(total).clone()))
    };

    AdminResponse::Statistics(res().await)
}
