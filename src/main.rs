#![deny(rust_2018_idioms, clippy::all, clippy::pedantic)]
#![allow(clippy::map_err_ignore)]

use std::sync::Arc;

use anyhow::Result;
use togglebot::{
    api::{Message, request::Request, response::Response},
    db::connection::Connection,
    discord,
    handler::{self, Access},
    settings::{self, Commands as CommandSettings, Levels, LogStyle, Logging},
    state::{self, State},
    statistics::{self, Stats},
    twitch,
};
use tokio::sync::mpsc;
use tokio_shutdown::Shutdown;
use tracing::{Subscriber, error, trace};
use tracing_subscriber::{Layer, filter::Targets, prelude::*, registry::LookupSpan};

#[tokio::main]
async fn main() -> Result<()> {
    let config = settings::load()?;

    tracing_subscriber::registry()
        .with(config.tracing.logging.map(init_logging))
        .with(init_targets(config.tracing.levels))
        .init();

    let command_settings = Arc::new(config.commands);

    let state = {
        let mut conn = Connection::new()?;
        state::migrate(&mut conn)?;
        State::new(conn)
    };

    let statistics = {
        let mut conn = Connection::new()?;
        statistics::migrate(&mut conn)?;
        Stats::new(conn)
    };

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

                let access = handler::access(&config.discord, &state, &message.author);
                let res = handle_message(&command_settings, &state, &statistics, access, message).await;

                let Some(res) = res else { continue };

                match res {
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

async fn handle_message(
    settings: &Arc<CommandSettings>,
    state: &State,
    statistics: &Stats,
    access: Access,
    message: Message,
) -> Option<Result<Response>> {
    Some(match (access, message.content) {
        (Access::Owner, Request::Owner(request)) => {
            handler::owner_message(message.span, state, request)
                .await
                .map(Response::Owner)
        }
        (_, Request::Owner(_)) => {
            trace!("non-owner tried using a owner-only request");
            return None;
        }
        (Access::Owner | Access::Admin, Request::Admin(request)) => {
            handler::admin_message(message.span, state, statistics, request)
                .await
                .map(Response::Admin)
        }
        (_, Request::Admin(_)) => {
            trace!("non-admin tried using a admin-only request");
            return None;
        }
        (_, Request::User(request)) => handler::user_message(
            message.span,
            Arc::clone(settings),
            state,
            statistics,
            request,
            message.source,
        )
        .await
        .map(Response::User),
    })
}
