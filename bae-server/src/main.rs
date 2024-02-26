pub mod model;
pub mod server;

use axum::extract::{FromRef, Host};
use axum::handler::HandlerWithoutStateExt;
use axum::http::{StatusCode, Uri};
use axum::response::Redirect;
use axum::BoxError;
use axum_server::tls_rustls::RustlsConfig;
use axum_server::Handle;
use bae_common::database;
use bae_common::markdown_render::{CodeBlockHighlighter, StandardClassNameGenerator};
use serde::Deserialize;
use sqlx::PgPool;
use std::future::Future;
use std::io::Write;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::time::Duration;
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing::{debug, info};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub type StandardCodeBlockHighlighter = CodeBlockHighlighter<StandardClassNameGenerator>;

#[derive(Clone, Eq, PartialEq, Debug, Deserialize)]
struct Env {
    base_uri: String,
    ip_address: IpAddr,
    http_port: u16,
    https_port: u16,
    database_url: String,
    static_path: PathBuf,
    tls_cert_path: PathBuf,
    tls_key_path: PathBuf,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
struct Ports {
    http: u16,
    https: u16,
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

    let ip_addr = env.ip_address;
    let ports = Ports {
        http: env.http_port,
        https: env.https_port,
    };

    let tls_config = RustlsConfig::from_pem_file(env.tls_cert_path, env.tls_key_path)
        .await
        .expect("Parsing certificate from pem file failed");

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

    let handle = Handle::new();
    let shutdown_future = shutdown_signal(handle.clone());

    // Spawn server to redirect HTTP to HTTPS
    tokio::spawn(redirect_to_https_server(ip_addr, ports, shutdown_future));

    let tracing_layer = TraceLayer::new_for_http();
    let app = server::router(&env.static_path)
        .layer(tracing_layer)
        .with_state(app_state);

    let addr = SocketAddr::new(ip_addr, ports.https);
    info!("Starting HTTPS server on {addr}");
    axum_server::bind_rustls(addr, tls_config)
        .serve(app.into_make_service())
        .await
        .expect("Error serving");
}

// https://github.com/tokio-rs/axum/blob/4d65ba0215b57797193ec49245d32d4dd79bb701/examples/tls-graceful-shutdown/src/main.rs
async fn redirect_to_https_server<F>(ip: IpAddr, ports: Ports, signal: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    fn make_https(host: String, uri: Uri, ports: Ports) -> Result<Uri, BoxError> {
        let mut parts = uri.into_parts();

        parts.scheme = Some(axum::http::uri::Scheme::HTTPS);

        if parts.path_and_query.is_none() {
            parts.path_and_query = Some("/".parse().unwrap());
        }

        let https_host = host.replace(&ports.http.to_string(), &ports.https.to_string());
        parts.authority = Some(https_host.parse()?);

        Ok(Uri::from_parts(parts)?)
    }

    let redirect = move |Host(host): Host, uri: Uri| async move {
        match make_https(host, uri, ports) {
            Ok(uri) => Ok(Redirect::permanent(&uri.to_string())),
            Err(error) => {
                tracing::error!(%error, "Failed to convert URI to HTTPS");
                Err(StatusCode::BAD_REQUEST)
            }
        }
    };

    let addr = SocketAddr::new(ip, ports.http);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Setting up listener for HTTP to HTTPS redirect failed");
    info!("HTTP to HTTPS redirect server listening on {addr}");
    axum::serve(listener, redirect.into_make_service())
        .with_graceful_shutdown(signal)
        .await
        .unwrap();
}

// https://github.com/tokio-rs/axum/blob/d1fb14ead1063efe31ae3202e947ffd569875c0b/examples/graceful-shutdown/src/main.rs
async fn shutdown_signal(handle: Handle) {
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

    info!("Shutdown signal received, shutting down");
    handle.graceful_shutdown(Some(Duration::from_secs(10)))
}
