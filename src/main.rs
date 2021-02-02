#![deny(rust_2018_idioms, clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::map_err_ignore)]

use std::{str::FromStr, sync::Arc};

use anyhow::{anyhow, bail, ensure, Result};
use chrono::prelude::*;
use log::{error, info, warn};
use togglebot::{
    discord,
    settings::{self, State},
    twitch, AdminResponse, Message, Response, Source, UserResponse,
};
use tokio::sync::{broadcast, mpsc, RwLock};

type AsyncState = Arc<RwLock<State>>;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    std::env::set_var("RUST_LOG", "warn,togglebot=trace");
    env_logger::init();

    let config = settings::load_config().await?;
    let state = settings::load_state().await?;
    let state = Arc::new(RwLock::new(state));

    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
    let shutdown_rx2 = shutdown_tx.subscribe();

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();

        info!("bot shutting down");
        shutdown_tx.send(()).ok();
    });

    let (queue_tx, mut queue_rx) = mpsc::channel(100);

    discord::start(&config.discord, queue_tx.clone(), shutdown_rx).await?;
    twitch::start(&config.twitch, queue_tx, shutdown_rx2).await?;

    while let Some((message, reply)) = queue_rx.recv().await {
        let res = if message.admin {
            handle_admin_message(state.clone(), message.content)
                .await
                .map(Response::Admin)
        } else {
            handle_user_message(state.clone(), message)
                .await
                .map(Response::User)
        };

        match res {
            Ok(resp) => {
                reply.send(resp).ok();
            }
            Err(e) => {
                error!("error during event handling: {}", e);
            }
        }
    }

    Ok(())
}

async fn handle_user_message(state: AsyncState, message: Message) -> Result<UserResponse> {
    Ok(match message.content.to_lowercase().as_ref() {
        "!help" | "!bot" => {
            info!("user: received `help` command");
            UserResponse::Help
        }
        "!commands" => {
            info!("user: received `commands` command");
            UserResponse::Commands(list_command_names(state, message.source).await)
        }
        "!links" => {
            info!("user: received `links` command");
            UserResponse::Links(match message.source {
                Source::Discord => &[
                    ("Website", "https://togglebit.io"),
                    ("GitHub", "https://github.com/togglebyte"),
                    ("Twitch", "https://twitch.tv/togglebit"),
                ],
                Source::Twitch => &[
                    ("Website", "https://togglebit.io"),
                    ("GitHub", "https://github.com/togglebyte"),
                    ("Discord", "https://discord.gg/qtyDMat"),
                ],
            })
        }
        "!schedule" => {
            info!("user: received `schedule` command");

            let state = state.read().await;

            UserResponse::Schedule {
                start: state.schedule.format_start(),
                finish: state.schedule.format_finish(),
                off_days: state
                    .off_days
                    .iter()
                    .map(|weekday| {
                        match weekday {
                            Weekday::Mon => "Monday",
                            Weekday::Tue => "Tuesday",
                            Weekday::Wed => "Wednesday",
                            Weekday::Thu => "Thursday",
                            Weekday::Fri => "Friday",
                            Weekday::Sat => "Saturday",
                            Weekday::Sun => "Sunday",
                        }
                        .to_owned()
                    })
                    .collect(),
            }
        }
        name => {
            if let Some(name) = name.strip_prefix('!') {
                state
                    .read()
                    .await
                    .custom_commands
                    .get(name)
                    .and_then(|content| content.get(&message.source))
                    .map(|content| {
                        info!("user: received custom `{}` command", name);
                        content
                    })
                    .cloned()
                    .map_or(UserResponse::Unknown, UserResponse::Custom)
            } else {
                UserResponse::Unknown
            }
        }
    })
}

async fn handle_admin_message(state: AsyncState, content: String) -> Result<AdminResponse> {
    let mut parts = content.split_whitespace();
    let command = if let Some(cmd) = parts.next() {
        cmd
    } else {
        bail!("got message without content")
    };

    Ok(
        match (
            command.to_lowercase().as_ref(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) {
            ("!help", None, None, None, None) => {
                info!("admin: received `help` command");
                AdminResponse::Help
            }
            ("!schedule", Some("set"), Some(field), Some(range_begin), Some(range_end)) => {
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
            ("!off_days", Some(action), Some(weekday), None, None) => {
                info!("admin: received `off_days` command");

                let res = || async {
                    update_off_days(
                        state,
                        action.parse()?,
                        weekday
                            .parse()
                            .map_err(|_| anyhow!("unknown weekday `{}`", weekday))?,
                    )
                    .await
                };

                AdminResponse::OffDays(res().await)
            }
            ("!custom_commands", Some("list"), None, None, None) => {
                AdminResponse::CustomCommands(list_commands(state).await.map(Some))
            }
            ("!custom_commands", Some(action), Some(source), Some(name), _) => {
                info!("admin: received `custom_commands` command");

                let content = content
                    .splitn(5, char::is_whitespace)
                    .filter(|c| !c.is_empty())
                    .nth(4);

                let res = || async {
                    update_commands(state, action.parse()?, source.parse()?, name, content).await
                };

                AdminResponse::CustomCommands(res().await.map(|_| None))
            }
            _ => AdminResponse::Unknown,
        },
    )
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

const RESERVED_COMMANDS: &[&str] = &["help", "bot", "commands", "links", "schedule"];

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

async fn list_command_names(state: AsyncState, source: Source) -> Result<Vec<String>> {
    Ok(state
        .read()
        .await
        .custom_commands
        .iter()
        .filter_map(|(name, sources)| {
            if sources.contains_key(&source) {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect())
}

async fn list_commands(state: AsyncState) -> Result<Vec<(String, Source, String)>> {
    Ok(state
        .read()
        .await
        .custom_commands
        .iter()
        .flat_map(|(name, sources)| {
            sources
                .iter()
                .map(move |(source, content)| (name.clone(), *source, content.clone()))
        })
        .collect())
}
