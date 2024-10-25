use std::sync::Arc;

use anyhow::bail;
use reqwest::StatusCode;
use serde::Deserialize;
use time::OffsetDateTime;
use tracing::{info, instrument};

use super::{AsyncCommandSettings, AsyncState};
use crate::api::{
    response::{self, CrateInfo, CrateSearch},
    Source,
};

#[instrument(skip_all)]
pub fn help() -> response::User {
    info!("received `help` command");
    response::User::Help
}

#[instrument(skip_all)]
pub async fn commands(state: AsyncState, source: Source) -> response::User {
    info!("received `commands` command");
    response::User::Commands(Ok(list_command_names(state, source).await))
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
pub fn links(settings: &AsyncCommandSettings) -> response::User {
    info!("received `links` command");
    response::User::Links(Arc::clone(&settings.links))
}

#[instrument(skip_all)]
pub fn ban(target: &str) -> response::User {
    info!("received `ban` command");
    response::User::Ban(target.to_owned())
}

#[instrument(skip_all, name = "crate")]
pub async fn crate_(name: &str) -> response::User {
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

    response::User::Crate(res.await)
}

#[instrument(skip_all)]
pub fn today() -> response::User {
    fn th(value: impl Into<u16>) -> &'static str {
        match value.into() % 10 {
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

    response::User::Today(format!(
        "Today is {weekday}, {month} the {day}{day_th} of {year} in the UTC time zone. Did you \
         know, this is the {day_of_year}{day_of_year_th} day of the year and we're in the \
         {week_of_year}{week_of_year_th} week of the year. Amazing, isn't it?!"
    ))
}

pub fn ftoc(fahrenheit: f64) -> response::User {
    response::User::FahrenheitToCelsius({
        let celsius = (fahrenheit - 32.0) / 1.8;
        format!("{fahrenheit:.1}°F => {celsius:.1}°C")
    })
}

pub fn ctof(celsius: f64) -> response::User {
    response::User::CelsiusToFahrenheit({
        let fahrenheit = celsius * 1.8 + 32.0;
        format!("{celsius:.1}°C => {fahrenheit:.1}°F")
    })
}

#[instrument(skip_all)]
pub async fn custom(state: AsyncState, source: Source, name: &str) -> Option<response::User> {
    state
        .read()
        .await
        .custom_commands
        .get(name)
        .and_then(|content| content.get(&source))
        .inspect(|_| info!("user: received custom `{name}` command"))
        .cloned()
        .map(response::User::Custom)
}
