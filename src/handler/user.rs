use std::sync::Arc;

use anyhow::bail;
use reqwest::StatusCode;
use serde::Deserialize;
use time::OffsetDateTime;
use tracing::{info, instrument};

use super::AsyncCommandSettings;
use crate::{
    api::{
        Source,
        response::{self, CrateInfo, CrateSearch},
    },
    state::State,
};

#[instrument(skip_all)]
pub fn help() -> response::User {
    info!("received `help` command");
    response::User::Help
}

#[instrument(skip_all)]
pub async fn commands(state: &State, source: Source) -> response::User {
    info!("received `commands` command");
    response::User::Commands(state.list_custom_command_names(source).await)
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
        #[cfg(test)]
        let resp = crate_test_response();
        #[cfg(not(test))]
        let resp = {
            let link = format!("https://crates.io/api/v1/crates/{name}");
            reqwest::Client::builder()
                .user_agent("ToggleBot (https://github.com/dnaka91/togglebot)")
                .build()?
                .get(&link)
                .send()
                .await?
        };

        Ok(match resp.status() {
            StatusCode::OK => CrateSearch::Found(resp.json::<ApiResponse>().await?.crate_),
            StatusCode::NOT_FOUND => CrateSearch::NotFound(format!("Crate `{name}` doesn't exist")),
            s => bail!("unexpected status code {s:?}"),
        })
    };

    response::User::Crate(res.await)
}

#[cfg(test)]
fn crate_test_response() -> reqwest::Response {
    http::Response::new(
        serde_json::json! {{
            "crate": {
                "name": "anyhow",
                "updated_at": "2024-10-22T17:51:36.413602+00:00",
                "downloads": 237_256_036,
                "newest_version": "1.0.91",
                "description": "Flexible concrete Error type built on std::error::Error",
                "documentation": "https://docs.rs/anyhow",
                "repository": "https://github.com/dtolnay/anyhow",
            }
        }}
        .to_string(),
    )
    .into()
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
pub async fn custom(state: &State, source: Source, name: &str) -> Option<response::User> {
    state
        .get_custom_command(source, name)
        .await
        .transpose()
        .map(|res| {
            if res.is_ok() {
                info!("user: received custom `{name}` command");
            }
            response::User::Custom(res)
        })
}
