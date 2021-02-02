use chrono::Weekday;
use log::info;

use super::AsyncState;
use crate::{Source, UserResponse};

pub fn help() -> UserResponse {
    info!("user: received `help` command");
    UserResponse::Help
}

pub async fn commands(state: AsyncState, source: Source) -> UserResponse {
    info!("user: received `commands` command");
    UserResponse::Commands(Ok(list_command_names(state, source).await))
}

async fn list_command_names(state: AsyncState, source: Source) -> Vec<String> {
    state
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
        .collect()
}

pub fn links(source: Source) -> UserResponse {
    info!("user: received `links` command");
    UserResponse::Links(match source {
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

pub async fn schedule(state: AsyncState) -> UserResponse {
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

pub async fn custom(state: AsyncState, source: Source, name: &str) -> UserResponse {
    if let Some(name) = name.strip_prefix('!') {
        state
            .read()
            .await
            .custom_commands
            .get(name)
            .and_then(|content| content.get(&source))
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
