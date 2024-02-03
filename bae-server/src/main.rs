pub mod model;
pub mod server;

use axum::extract::FromRef;
use bae_common::highlighting::Theme;
use bae_common::markdown_render::{CodeBlockHighlighter, StandardClassNameGenerator};
use comrak::{ExtensionOptionsBuilder, ParseOptionsBuilder, RenderOptionsBuilder};
use serde::Deserialize;
use sqlx::{migrate, PgPool};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing::debug;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub type StandardCodeBlockHighlighter = CodeBlockHighlighter<StandardClassNameGenerator>;

#[derive(Clone, Eq, PartialEq, Debug, Deserialize)]
struct Env {
    socket_address: String,
    database_url: String,
}

#[derive(Clone, FromRef)]
pub struct AppState {
    database: PgPool,
    highlighter: Arc<StandardCodeBlockHighlighter>,
    comrak_options: Arc<comrak::Options>,
    light_highlight_theme: Theme,
}

#[tokio::main]
async fn main() {
    _ = dotenv::dotenv();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "bae=debug,tower_http=debug,axum::rejection=trace,sqlx=debug".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let env: Env = envy::from_env().expect("Deserializing environment variables failed");

    let database = PgPool::connect(&env.database_url)
        .await
        .expect("Could not connect to database");

    migrate!("../migrations")
        .run(&database)
        .await
        .expect("Database migration failed");

    let highlighter = Arc::new(
        CodeBlockHighlighter::standard_config()
            .expect("Loading CodeBlockHighlighter config failed"),
    );

    let comrak_options = Arc::new(comrak::Options {
        extension: ExtensionOptionsBuilder::default()
            .strikethrough(true)
            .tagfilter(true)
            .table(true)
            .autolink(true)
            .tasklist(true)
            .superscript(true)
            .footnotes(true)
            .multiline_block_quotes(true)
            .build()
            .expect("Building ExtensionOptions failed"),
        parse: ParseOptionsBuilder::default().smart(true).build().unwrap(),
        render: RenderOptionsBuilder::default()
            .unsafe_(true)
            .build()
            .expect("Building RenderOptions failed"),
    });

    let light_highlight_theme = serde_json::from_str(include_str!(
        "../../web_contents/highlighting_themes/light.json"
    ))
    .expect("Loading light highlight theme failed");

    let app_state = AppState {
        database,
        highlighter,
        comrak_options,
        light_highlight_theme,
    };

    let tracing_layer = TraceLayer::new_for_http();
    let app = server::router().layer(tracing_layer).with_state(app_state);

    let listener = TcpListener::bind(env.socket_address)
        .await
        .expect("Binding TcpListener failed");

    debug!(
        "Listening on {}",
        listener.local_addr().expect("Cannot read local address")
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Error serving");
}

// https://github.com/tokio-rs/axum/blob/d1fb14ead1063efe31ae3202e947ffd569875c0b/examples/graceful-shutdown/src/main.rs
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
