mod server;
mod templates;

use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::{debug, error};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct AppState {}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "bae=debug,tower_http=debug,axum::rejection=trace".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let tracing_layer = TraceLayer::new_for_http();

    let app_state = AppState {};

    let app = server::router().layer(tracing_layer).with_state(app_state);

    let listener = TcpListener::bind("localhost:80")
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
