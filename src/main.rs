#![deny(rust_2018_idioms, clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::map_err_ignore)]

use std::sync::Arc;

use anyhow::Result;
use togglebot::{
    discord,
    handler::{self, Access},
    settings,
    state::{self, State},
    twitch, AdminResponse, Message, OwnerResponse, Response,
};
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{error, info, warn, Level};
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

    let config = settings::load_config()?;
    let state = state::load()?;
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
        let res = async {
            match handler::access(&config, Arc::clone(&state), &message.author).await {
                Access::Standard => handle_user_message(&state, &message).await,
                Access::Admin => handle_admin_message(&state, &message).await,
                Access::Owner => handle_owner_message(&state, &message).await,
            }
        };

        match res.await {
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

async fn handle_user_message(state: &Arc<RwLock<State>>, message: &Message) -> Result<Response> {
    handler::user_message(Arc::clone(state), &message.content, message.source)
        .await
        .map(Response::User)
}

async fn handle_admin_message(state: &Arc<RwLock<State>>, message: &Message) -> Result<Response> {
    match handler::admin_message(Arc::clone(state), &message.content).await? {
        AdminResponse::Unknown => handle_user_message(state, message).await,
        resp => Ok(Response::Admin(resp)),
    }
}

async fn handle_owner_message(state: &Arc<RwLock<State>>, message: &Message) -> Result<Response> {
    match handler::owner_message(Arc::clone(state), &message.content, message.mention).await? {
        OwnerResponse::Unknown => handle_admin_message(state, message).await,
        resp => Ok(Response::Owner(resp)),
    }
}
