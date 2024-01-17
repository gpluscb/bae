pub mod database;
pub mod model;
pub mod server;

use axum::extract::FromRef;
use serde::Deserialize;
use sqlx::{migrate, query, SqlitePool};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::{debug, error};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Clone, Eq, PartialEq, Debug, Deserialize)]
struct Env {
    socket_address: String,
    database_url: String,
}

#[derive(Clone, Debug, FromRef)]
pub struct AppState {
    database: SqlitePool,
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

    let database = SqlitePool::connect(&env.database_url)
        .await
        .expect("Could not connect to database");

    migrate!()
        .run(&database)
        .await
        .expect("Database migration failed");

    query!("PRAGMA foreign_keys=ON")
        .execute(&database)
        .await
        .expect("foreign_keys query failed");

    let app_state = AppState { database };

    let tracing_layer = TraceLayer::new_for_http();
    let app = server::router().layer(tracing_layer).with_state(app_state);

    let listener = TcpListener::bind(env.socket_address)
        .await
        .expect("Binding TcpListener failed");

    debug!(
        "Listening on {}",
        listener.local_addr().expect("Cannot read local address")
    );

    if let Err(error) = axum::serve(listener, app).await {
        error!(%error, "Error serving");
    }
}
