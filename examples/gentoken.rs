use std::collections::HashMap;

use anyhow::{Result, bail};
use reqwest::Url;
use togglebot::settings;
use twitch_api::{
    helix::Scope,
    twitch_oauth2::{TwitchToken, UserToken},
};

#[tokio::main]
async fn main() -> Result<()> {
    let settings = settings::load()?;
    let url = "http://localhost".parse()?;

    let mut builder = UserToken::builder(
        settings.twitch.client_id.into(),
        settings.twitch.client_secret.into(),
        url,
    )
    .force_verify(true)
    .set_scopes(vec![
        Scope::ChannelBot,
        Scope::UserReadChat,
        Scope::UserWriteChat,
    ]);

    let (url, _) = builder.generate_url();
    println!("visit this page: {url}\n");

    println!("paste result url:");
    let mut url = String::new();
    std::io::stdin().read_line(&mut url)?;
    let url = Url::parse(&url)?;

    let pairs = url.query_pairs().collect::<HashMap<_, _>>();

    if let Some((state, code)) = pairs.get("state").zip(pairs.get("code")) {
        let token = builder
            .get_user_token(&reqwest::Client::new(), state, code)
            .await?;

        println!("scopes: {}", token.scopes().join(", "));
        println!("access token: {}", token.access_token.as_str());
        println!("refresh token: {}", token.refresh_token.unwrap().as_str());
    } else if let Some((error, description)) =
        pairs.get("error").zip(pairs.get("error_description"))
    {
        bail!("got error from twitch:\n{error}: {description}");
    } else {
        bail!("invalid url");
    }

    Ok(())
}
