pub mod model;
pub mod server;

use axum::extract::FromRef;
use bae_common::database;
use bae_common::markdown_render::{CodeBlockHighlighter, StandardClassNameGenerator};
use serde::Deserialize;
use sqlx::PgPool;
use tokio::net::TcpListener;
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing::debug;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub type StandardCodeBlockHighlighter = CodeBlockHighlighter<StandardClassNameGenerator>;

#[derive(Clone, Eq, PartialEq, Debug, Deserialize)]
struct Env {
    base_uri: String,
    socket_address: String,
    database_url: String,
}

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct BaseUri(pub String);

#[derive(Clone, FromRef)]
pub struct AppState {
    database: PgPool,
    base_uri: BaseUri,
}

#[tokio::main]
async fn main() {
    _ = dotenv::dotenv();
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "bae_server=debug,bae_common=debug,tower_http=debug,axum::rejection=trace,sqlx=debug".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let env: Env = envy::from_env().expect("Deserializing environment variables failed");

    let database = PgPool::connect(&env.database_url)
        .await
        .expect("Could not connect to database");

    database::migrate(&database)
        .await
        .expect("Database migration failed");

    let app_state = AppState {
        database,
        base_uri: BaseUri(env.base_uri),
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
