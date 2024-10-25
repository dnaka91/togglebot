#![deny(rust_2018_idioms, clippy::all, clippy::pedantic)]
#![allow(clippy::map_err_ignore)]

use std::{sync::Arc, time::Duration};

use anyhow::{bail, Result};
use togglebot::{
    api::{request::Request, response::Response, Message},
    discord,
    handler::{self, Access},
    settings::{self, Commands as CommandSettings, Levels, LogStyle, Logging},
    state::{self, State},
    statistics::{self, Stats},
    twitch,
};
use tokio::sync::{mpsc, RwLock};
use tokio_shutdown::Shutdown;
use tracing::{error, Subscriber};
use tracing_subscriber::{filter::Targets, prelude::*, registry::LookupSpan, Layer};

#[tokio::main]
async fn main() -> Result<()> {
    let config = settings::load()?;

    tracing_subscriber::registry()
        .with(config.tracing.logging.map(init_logging))
        .with(init_targets(config.tracing.levels))
        .init();

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
        shutdown.clone(),
    )
    .await?;

    loop {
        tokio::select! {
            () = shutdown.handle() => break,
            item = queue_rx.recv() => {
                let Some((message, reply)) = item else { break };
                let res = async {
                    match handler::access(&config.discord, Arc::clone(&state), &message.author).await {
                        Access::Standard => {
                            handle_user_message(&command_settings, &state, &statistics, message).await
                        }
                        Access::Admin => {
                            handle_admin_message(&state, &statistics, message).await
                        }
                        Access::Owner => {
                            handle_owner_message(&state, message).await
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
        }
    }

    if let Err(e) = statistics::save(&*statistics.read().await).await {
        error!(error = ?e, "failed saving statistics to file system");
    }

    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
fn init_logging<S>(settings: Logging) -> impl Layer<S>
where
    for<'span> S: Subscriber + LookupSpan<'span>,
{
    let layer = tracing_subscriber::fmt::layer();

    match settings.style {
        LogStyle::Default => layer.boxed(),
        LogStyle::Compact => layer.compact().boxed(),
        LogStyle::Pretty => layer.pretty().boxed(),
    }
}

fn init_targets(settings: Levels) -> Targets {
    Targets::new()
        .with_default(settings.default)
        .with_target(env!("CARGO_CRATE_NAME"), settings.togglebot)
        .with_targets(settings.targets)
}

async fn handle_user_message(
    settings: &Arc<CommandSettings>,
    state: &Arc<RwLock<State>>,
    statistics: &Arc<RwLock<Stats>>,
    message: Message,
) -> Result<Response> {
    let Request::User(request) = message.content else {
        bail!("not a user request");
    };

    handler::user_message(
        message.span,
        Arc::clone(settings),
        Arc::clone(state),
        Arc::clone(statistics),
        request,
        message.source,
    )
    .await
    .map(Response::User)
}

async fn handle_admin_message(
    state: &Arc<RwLock<State>>,
    statistics: &Arc<RwLock<Stats>>,
    message: Message,
) -> Result<Response> {
    let Request::Admin(request) = message.content else {
        bail!("not an admin request");
    };

    handler::admin_message(
        message.span,
        Arc::clone(state),
        Arc::clone(statistics),
        request,
    )
    .await
    .map(Response::Admin)
}

async fn handle_owner_message(state: &Arc<RwLock<State>>, message: Message) -> Result<Response> {
    let Request::Owner(request) = message.content else {
        bail!("not an owner request");
    };

    handler::owner_message(message.span, Arc::clone(state), request)
        .await
        .map(Response::Owner)
}
