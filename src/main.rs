#![deny(rust_2018_idioms, clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::map_err_ignore)]

use std::{sync::Arc, time::Duration};

use anyhow::Result;
use togglebot::{
    discord,
    handler::{self, Access},
    settings::{self, Commands as CommandSettings},
    state::{self, State},
    statistics::{self, Stats},
    twitch, AdminResponse, Message, OwnerResponse, Response,
};
use tokio::sync::{mpsc, RwLock};
use tokio_shutdown::Shutdown;
use tracing::{error, warn, Level};
use tracing_subscriber::{filter::Targets, prelude::*};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            Targets::new()
                .with_target(env!("CARGO_CRATE_NAME"), Level::TRACE)
                .with_target("docsearch", Level::TRACE)
                .with_default(Level::WARN),
        )
        .init();

    let config = settings::load()?;

    let command_settings = Arc::new(config.commands);
    let state = state::load()?;
    let state = Arc::new(RwLock::new(state));

    let statistics = statistics::load()?;
    let statistics = Arc::new(RwLock::new(statistics));
    let statistics2 = Arc::clone(&statistics);

    // Sync statistics to the file system once a day
    tokio::spawn(async move {
        const ONE_DAY: Duration = Duration::from_secs(60 * 60 * 24);

        // We directly save once at startup. This allows some automatic cleanups by going through
        // the deserializer -> serialize cycle once.
        if let Err(e) = statistics::save(&*statistics2.read().await).await {
            error!(error = ?e, "periodic statistics saving failed");
        }

        tokio::time::sleep(ONE_DAY).await;
    });

    let shutdown = Shutdown::new()?;

    let (queue_tx, mut queue_rx) = mpsc::channel(100);

    discord::start(
        &config.discord,
        Arc::clone(&command_settings),
        queue_tx.clone(),
        shutdown.clone(),
    )
    .await?;
    twitch::start(
        &config.twitch,
        Arc::clone(&command_settings),
        queue_tx,
        shutdown,
    )
    .await?;

    while let Some((message, reply)) = queue_rx.recv().await {
        let res = async {
            match handler::access(&config.discord, Arc::clone(&state), &message.author).await {
                Access::Standard => {
                    handle_user_message(&command_settings, &state, &statistics, &message).await
                }
                Access::Admin => {
                    handle_admin_message(&command_settings, &state, &statistics, &message).await
                }
                Access::Owner => {
                    handle_owner_message(&command_settings, &state, &statistics, &message).await
                }
            }
        };

        match res.await {
            Ok(resp) => {
                reply.send(resp).ok();
            }
            Err(e) => {
                error!(error = ?e, "error during event handling");
            }
        }
    }

    if let Err(e) = statistics::save(&*statistics.read().await).await {
        error!(error = ?e, "failed saving statistics to file system");
    }

    Ok(())
}

async fn handle_user_message(
    settings: &Arc<CommandSettings>,
    state: &Arc<RwLock<State>>,
    statistics: &Arc<RwLock<Stats>>,
    message: &Message,
) -> Result<Response> {
    handler::user_message(
        Arc::clone(settings),
        Arc::clone(state),
        Arc::clone(statistics),
        &message.content,
        message.source,
    )
    .await
    .map(Response::User)
}

async fn handle_admin_message(
    settings: &Arc<CommandSettings>,
    state: &Arc<RwLock<State>>,
    statistics: &Arc<RwLock<Stats>>,
    message: &Message,
) -> Result<Response> {
    match handler::admin_message(Arc::clone(state), Arc::clone(statistics), &message.content)
        .await?
    {
        AdminResponse::Unknown => handle_user_message(settings, state, statistics, message).await,
        resp => Ok(Response::Admin(resp)),
    }
}

async fn handle_owner_message(
    settings: &Arc<CommandSettings>,
    state: &Arc<RwLock<State>>,
    statistics: &Arc<RwLock<Stats>>,
    message: &Message,
) -> Result<Response> {
    match handler::owner_message(Arc::clone(state), &message.content, message.mention).await? {
        OwnerResponse::Unknown => handle_admin_message(settings, state, statistics, message).await,
        resp => Ok(Response::Owner(resp)),
    }
}
