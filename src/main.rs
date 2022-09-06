#![deny(rust_2018_idioms, clippy::all, clippy::pedantic)]
#![warn(clippy::nursery)]
#![allow(clippy::map_err_ignore)]

use std::{sync::Arc, time::Duration};

use anyhow::Result;
use togglebot::{
    discord,
    handler::{self, Access},
    settings::{self, Commands as CommandSettings, Levels, LogStyle, Logging, Otlp},
    state::{self, State},
    statistics::{self, Stats},
    twitch, Message, Response,
};
use tokio::sync::{mpsc, RwLock};
use tokio_shutdown::Shutdown;
use tracing::{error, warn, Subscriber};
use tracing_subscriber::{filter::Targets, prelude::*, registry::LookupSpan, Layer};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let config = settings::load()?;

    tracing_subscriber::registry()
        .with(config.tracing.logging.map(init_logging))
        .with(config.tracing.otlp.map(init_tracing).transpose()?)
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
        shutdown,
    )
    .await?;

    while let Some((message, reply)) = queue_rx.recv().await {
        let res = async {
            match handler::access(&config.discord, Arc::clone(&state), &message.author).await {
                Access::Standard => {
                    handle_user_message(&command_settings, &state, &statistics, message).await
                }
                Access::Admin => {
                    handle_admin_message(&command_settings, &state, &statistics, message).await
                }
                Access::Owner => {
                    handle_owner_message(&command_settings, &state, &statistics, message).await
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

fn init_tracing<S>(settings: Otlp) -> Result<impl Layer<S>>
where
    for<'span> S: Subscriber + LookupSpan<'span>,
{
    use opentelemetry::{
        global, runtime,
        sdk::{trace, Resource},
    };
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_semantic_conventions::resource;

    global::set_error_handler(|error| {
        error!(target: "opentelemetry", %error);
    })?;

    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint(settings.endpoint),
        )
        .with_trace_config(trace::config().with_resource(Resource::new([
            resource::SERVICE_NAME.string(env!("CARGO_CRATE_NAME")),
            resource::SERVICE_VERSION.string(env!("CARGO_PKG_VERSION")),
        ])))
        .install_batch(runtime::Tokio)?;

    Ok(tracing_opentelemetry::layer().with_tracer(tracer))
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
    handler::user_message(
        message.span,
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
    message: Message,
) -> Result<Response> {
    if is_admin_command(&message.content) {
        handler::admin_message(
            message.span,
            Arc::clone(state),
            Arc::clone(statistics),
            &message.content,
        )
        .await
        .map(Response::Admin)
    } else {
        handle_user_message(settings, state, statistics, message).await
    }
}

async fn handle_owner_message(
    settings: &Arc<CommandSettings>,
    state: &Arc<RwLock<State>>,
    statistics: &Arc<RwLock<Stats>>,
    message: Message,
) -> Result<Response> {
    if is_owner_command(&message.content) {
        handler::owner_message(
            message.span,
            Arc::clone(state),
            &message.content,
            message.mention,
        )
        .await
        .map(Response::Owner)
    } else {
        handle_admin_message(settings, state, statistics, message).await
    }
}

fn is_admin_command(content: &str) -> bool {
    get_command(content).map_or(false, |cmd| {
        matches!(
            cmd.as_ref(),
            "admin_help"
                | "admin-help"
                | "adminhelp"
                | "ahelp"
                | "custom_commands"
                | "custom_command"
                | "stats"
        )
    })
}

fn is_owner_command(content: &str) -> bool {
    get_command(content).map_or(false, |cmd| {
        matches!(
            cmd.as_ref(),
            "owner_help" | "owner-help" | "ownerhelp" | "ohelp" | "admins" | "admin"
        )
    })
}

fn get_command(content: &str) -> Option<String> {
    content
        .split(char::is_whitespace)
        .next()
        .unwrap_or(content)
        .strip_prefix('!')
        .map(str::to_lowercase)
}
