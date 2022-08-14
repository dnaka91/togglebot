use std::sync::Arc;

use anyhow::bail;
use cienli::ciphers::rot::{Rot, RotType};
use reqwest::StatusCode;
use serde::Deserialize;
use time::OffsetDateTime;
use tracing::{info, instrument};

use super::{AsyncCommandSettings, AsyncState};
use crate::{CrateInfo, CrateSearch, Source, UserResponse};

mod doc;

#[instrument(skip_all)]
pub fn help() -> UserResponse {
    info!("received `help` command");
    UserResponse::Help
}

#[instrument(skip_all)]
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

#[instrument(skip_all)]
pub fn links(settings: &AsyncCommandSettings) -> UserResponse {
    info!("received `links` command");
    UserResponse::Links(Arc::clone(&settings.links))
}

#[instrument(skip_all)]
pub fn ban(target: &str) -> UserResponse {
    info!("received `ban` command");
    UserResponse::Ban(target.to_owned())
}

#[instrument(skip_all, name = "crate")]
pub async fn crate_(name: &str) -> UserResponse {
    #[derive(Deserialize)]
    struct ApiResponse {
        #[serde(rename = "crate")]
        crate_: CrateInfo,
    }

    info!("received `crate` command");

    let res = async {
        let link = format!("https://crates.io/api/v1/crates/{name}");
        let resp = reqwest::Client::builder()
            .user_agent("ToggleBot (https://github.com/dnaka91/togglebot)")
            .build()?
            .get(&link)
            .send()
            .await?;

        Ok(match resp.status() {
            StatusCode::OK => CrateSearch::Found(resp.json::<ApiResponse>().await?.crate_),
            StatusCode::NOT_FOUND => CrateSearch::NotFound(format!("Crate `{name}` doesn't exist")),
            s => bail!("unexpected status code {s:?}"),
        })
    };

    UserResponse::Crate(res.await)
}

#[instrument(skip_all)]
pub async fn doc(path: &str) -> UserResponse {
    info!("received `doc` command");
    UserResponse::Doc(doc::find(path).await)
}

#[instrument(skip_all)]
pub fn today() -> UserResponse {
    fn th(value: impl Into<u16>) -> &'static str {
        match value.into() {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        }
    }

    info!("received `today` command");

    let date = OffsetDateTime::now_utc();
    let weekday = date.weekday();
    let month = date.month();
    let day = date.day();
    let day_th = th(day);
    let year = date.year();
    let day_of_year = date.ordinal();
    let day_of_year_th = th(day_of_year);
    let week_of_year = date.iso_week();
    let week_of_year_th = th(week_of_year);

    UserResponse::Today(format!(
        "Today is {weekday}, {month} the {day}{day_th} of {year} in the UTC time zone. \
        Did you know, this is the {day_of_year}{day_of_year_th} day of the year \
        and we're in the {week_of_year}{week_of_year_th} week of the year. \
        Amazing, isn't it?!"
    ))
}

#[instrument(skip_all)]
pub fn encipher(text: &str) -> UserResponse {
    info!("received `encipher` command");
    UserResponse::Encipher(Rot::new(text, RotType::Rot13).encipher())
}

#[instrument(skip_all)]
pub fn decipher(text: &str) -> UserResponse {
    info!("received `decipher` command");
    UserResponse::Encipher(Rot::new(text, RotType::Rot13).decipher())
}

#[instrument(skip_all)]
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
