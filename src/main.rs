#![deny(rust_2018_idioms, clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::map_err_ignore)]

use std::{str::FromStr, sync::Arc};

use anyhow::{anyhow, bail, Result};
use chrono::prelude::*;
use log::{error, info, warn};
use togglebot::{
    discord,
    settings::{self, State},
    AdminResponse, Response, UserResponse,
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

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();

        info!("bot shutting down");
        shutdown_tx.send(()).ok();
    });

    let (queue_tx, mut queue_rx) = mpsc::channel(100);

    discord::start(&config.discord, queue_tx, shutdown_rx).await?;

    while let Some((message, reply)) = queue_rx.recv().await {
        let res = if message.admin {
            handle_admin_message(state.clone(), message.content)
                .await
                .map(Response::Admin)
        } else {
            handle_user_message(state.clone(), message.content)
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

async fn handle_user_message(state: AsyncState, content: String) -> Result<UserResponse> {
    Ok(match content.as_ref() {
        "!help" => {
            info!("user: received `help` command");
            UserResponse::Help
        }
        "!links" => {
            info!("user: received `links` command");
            UserResponse::Links(&[
                ("Website", "https://togglebit.io"),
                ("GitHub", "https://github.com/togglebyte"),
                ("Twitch", "https://twitch.tv/togglebit"),
                ("Discord", "https://discord.gg/qtyDMat"),
            ])
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
        _ => UserResponse::Unknown,
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
            command,
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
