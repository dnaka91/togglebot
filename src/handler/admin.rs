use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};
use tracing::{info, instrument};

use crate::{
    api::{Source, request::StatisticsDate, response},
    state::State,
    statistics::Stats,
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
pub async fn custom_commands_list(state: &State) -> response::Admin {
    info!("received `custom_commands list` command");

    response::Admin::CustomCommands(response::CustomCommands::List(list_commands(state).await))
}

async fn list_commands(state: &State) -> Result<BTreeMap<String, BTreeSet<Source>>> {
    Ok(state.list_custom_commands().await?.into_iter().fold(
        BTreeMap::new(),
        |mut acc, (name, source)| {
            acc.entry(name).or_default().insert(source);
            acc
        },
    ))
}

#[instrument(skip_all)]
pub async fn custom_commands(
    state: &State,
    statistics: &Stats,
    content: &str,
    action: Action,
    source: Option<Source>,
    name: &str,
) -> response::Admin {
    info!("received `custom_commands` command");

    response::Admin::CustomCommands(response::CustomCommands::Edit(
        update_commands(state, statistics, action, source, name, content).await,
    ))
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
    state: &State,
    statistics: &Stats,
    action: Action,
    source: Option<Source>,
    name: &str,
    content: &str,
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

    match action {
        Action::Add => {
            ensure!(!content.is_empty(), "no content for the command provided");

            if let Some(source) = source {
                state.add_custom_command(source, name, content).await?;
            } else {
                for source in [Source::Discord, Source::Twitch] {
                    state.add_custom_command(source, name, content).await?;
                }
            }
        }
        Action::Remove => {
            match source {
                Some(source) => {
                    state.remove_custom_command(source, name).await?;
                }
                None => {
                    state.remove_custom_command_by_name(name).await?;
                }
            }

            statistics.erase_custom(name).await?;
        }
    }

    Ok(())
}

#[instrument(skip(stats))]
pub async fn stats(stats: &Stats, date: StatisticsDate) -> response::Admin {
    let res = async || {
        let total = match date {
            StatisticsDate::Total => true,
            StatisticsDate::Current => false,
        };

        Ok((total, stats.get(total).await?))
    };

    response::Admin::Statistics(res().await)
}
