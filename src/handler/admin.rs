use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, ensure, Result};
use tracing::{info, instrument};

use super::{AsyncState, AsyncStats};
use crate::{
    api::{request::StatisticsDate, response, Source},
    state,
};

#[instrument(skip_all)]
pub fn help() -> response::Admin {
    info!("received `help` command");
    response::Admin::Help
}

#[derive(Debug)]
pub(super) enum Action {
    Add,
    Remove,
}

#[instrument(skip_all)]
pub async fn custom_commands_list(state: AsyncState) -> response::Admin {
    info!("received `custom_commands list` command");
    response::Admin::CustomCommands(response::CustomCommands::List(list_commands(state).await))
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

#[instrument(skip_all)]
pub async fn custom_commands(
    state: AsyncState,
    statistics: AsyncStats,
    content: &str,
    action: Action,
    source: Option<Source>,
    name: &str,
) -> response::Admin {
    info!("received `custom_commands` command");

    let content = content
        .splitn(5, char::is_whitespace)
        .filter(|c| !c.is_empty())
        .nth(4);

    let res = || async { update_commands(state, statistics, action, source, name, content).await };

    response::Admin::CustomCommands(response::CustomCommands::Edit(res().await))
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
    "crate",
    "crates",
    "ban",
    "today",
    "ftoc",
    "ctof",
    // admin commands
    "admin_help",
    "admin-help",
    "adminhelp",
    "ahelp",
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

#[instrument(skip(state, statistics))]
async fn update_commands(
    state: AsyncState,
    statistics: AsyncStats,
    action: Action,
    source: Option<Source>,
    name: &str,
    content: Option<&str>,
) -> Result<()> {
    ensure!(
        !name.starts_with('!'),
        "command names must not start with an `!`",
    );
    ensure!(
        name.starts_with(|c: char| c.is_ascii_lowercase()),
        "command names must start with a lowercase letter",
    );
    ensure!(
        name.chars()
            .all(|c| c == '_' || c.is_ascii_lowercase() || c.is_ascii_digit()),
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
                if let Some(source) = source {
                    state
                        .custom_commands
                        .entry(name.to_owned())
                        .or_default()
                        .insert(source, content.to_owned());
                } else {
                    let entry = state.custom_commands.entry(name.to_owned()).or_default();

                    for source in &[Source::Discord, Source::Twitch] {
                        entry.insert(*source, content.to_owned());
                    }
                }
            } else {
                bail!("no content for the command provided");
            }
        }
        Action::Remove => {
            match source {
                Some(source) => {
                    if let Some(entry) = state.custom_commands.get_mut(name) {
                        entry.remove(&source);
                    }
                }
                None => {
                    state.custom_commands.remove(name);
                }
            }

            statistics.write().await.erase_custom(name);
        }
    }

    state::save(&state).await?;

    Ok(())
}

#[instrument(skip(stats))]
pub async fn stats(stats: AsyncStats, date: StatisticsDate) -> response::Admin {
    let res = || async {
        let total = match date {
            StatisticsDate::Total => true,
            StatisticsDate::Current => false,
        };

        Ok((total, stats.write().await.get(total).clone()))
    };

    response::Admin::Statistics(res().await)
}
