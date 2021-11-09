#![deny(rust_2018_idioms, clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::map_err_ignore)]

use std::sync::Arc;

use anyhow::Result;
use togglebot::{discord, handler, settings, twitch, Response};
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{error, info, warn, Level};
use tracing_subscriber::{filter::Targets, prelude::*};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            Targets::new()
                .with_target(env!("CARGO_PKG_NAME"), Level::TRACE)
                .with_default(Level::WARN),
        )
        .init();

    let config = settings::load_config()?;
    let state = settings::load_state()?;
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
            handler::admin_message(state.clone(), message.content)
                .await
                .map(Response::Admin)
        } else {
            handler::user_message(state.clone(), message)
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
