use anyhow::bail;
use reqwest::StatusCode;
use serde::Deserialize;
use time::Weekday;
use tracing::info;

use super::AsyncState;
use crate::{CrateInfo, CrateSearch, ScheduleResponse, Source, UserResponse};

mod doc;

pub fn help() -> UserResponse {
    info!("received `help` command");
    UserResponse::Help
}

pub async fn commands(state: AsyncState, source: Source) -> UserResponse {
    info!("received `commands` command");
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
    info!("received `links` command");
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
    info!("received `schedule` command");

    let state = state.read().await;
    let res = || {
        Ok(ScheduleResponse {
            start: state.schedule.format_start()?,
            finish: state.schedule.format_finish()?,
            off_days: state
                .off_days
                .iter()
                .map(|weekday| {
                    match weekday {
                        Weekday::Monday => "Monday",
                        Weekday::Tuesday => "Tuesday",
                        Weekday::Wednesday => "Wednesday",
                        Weekday::Thursday => "Thursday",
                        Weekday::Friday => "Friday",
                        Weekday::Saturday => "Saturday",
                        Weekday::Sunday => "Sunday",
                    }
                    .to_owned()
                })
                .collect(),
        })
    };

    UserResponse::Schedule(res())
}

pub fn ban(target: &str) -> UserResponse {
    info!("received `ban` command");
    UserResponse::Ban(target.to_owned())
}

pub async fn crate_(name: &str) -> UserResponse {
    #[derive(Deserialize)]
    struct ApiResponse {
        #[serde(rename = "crate")]
        crate_: CrateInfo,
    }

    info!("received `crate` command");

    let res = async {
        let link = format!("https://crates.io/api/v1/crates/{}", name);
        let resp = reqwest::Client::builder()
            .user_agent("ToggleBot (https://github.com/dnaka91/togglebot)")
            .build()?
            .get(&link)
            .send()
            .await?;

        Ok(match resp.status() {
            StatusCode::OK => CrateSearch::Found(resp.json::<ApiResponse>().await?.crate_),
            StatusCode::NOT_FOUND => {
                CrateSearch::NotFound(format!("Crate `{}` doesn't exist", name))
            }
            s => bail!("unexpected status code {:?}", s),
        })
    };

    UserResponse::Crate(res.await)
}

pub async fn doc(path: &str) -> UserResponse {
    info!("received `doc` command");
    UserResponse::Doc(doc::find(path).await)
}

pub async fn custom(state: AsyncState, source: Source, name: &str) -> UserResponse {
    state
        .read()
        .await
        .custom_commands
        .get(name)
        .and_then(|content| content.get(&source))
        .map(|content| {
            info!("user: received custom `{name}` command");
            content
        })
        .cloned()
        .map_or(UserResponse::Unknown, UserResponse::Custom)
}
