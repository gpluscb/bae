pub mod blog;
pub mod templates;

use crate::highlighting::Theme;
use crate::{database, AppState, StandardCodeBlockHighlighter};
use askama::Template;
use axum::extract::rejection::PathRejection;
use axum::extract::State;
use axum::handler::HandlerWithoutStateExt;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::Router;
use axum_extra::response::Css;
use axum_extra::routing::{RouterExt, TypedPath};
use serde::Deserialize;
use std::sync::Arc;
use templates::{ErrorTemplate, HomeTemplate};
use thiserror::Error;
use tower_http::services::ServeDir;
use tracing::error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Not found")]
    NotFound,
    #[error("Path rejection: {0}")]
    Path(#[from] PathRejection),
    #[error("Askama error: {0}")]
    Askama(#[from] askama::Error),
    #[error("Database error: {0}")]
    Database(#[from] database::Error),
}

impl Error {
    pub fn status(&self) -> StatusCode {
        match self {
            Error::NotFound | Error::Path(_) => StatusCode::NOT_FOUND,
            Error::Askama(_) | Error::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = self.status();

        error!(error = %self, %status, "Replying with error");

        match (ErrorTemplate { status }.render()) {
            Ok(html) => Html(html).into_response(),
            Err(error) => {
                error!(%error, "Error trying to reply with error");
                (status, Html(format!("Error code {status}"))).into_response()
            }
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .typed_get(home)
        .typed_get(highlight_style_css)
        .merge(blog::router())
        .fallback_service(
            ServeDir::new("web_contents/static")
                .fallback((|| async { Error::NotFound }).into_service()),
        )
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/", rejection(Error))]
pub struct HomePath {}

pub async fn home(HomePath {}: HomePath) -> Result<Html<String>> {
    let html = HomeTemplate {}.render()?;
    Ok(Html(html))
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/highlight_style.css")]
pub struct StyleCssPath {}

pub async fn highlight_style_css(
    StyleCssPath {}: StyleCssPath,
    State(theme): State<Theme>,
    State(highlighter): State<Arc<StandardCodeBlockHighlighter>>,
) -> Result<Css<Vec<u8>>> {
    let mut css = Vec::new();
    // TODO: No unwrap
    theme
        .write_css_with_class_names(&mut css, &highlighter.class_name_generator)
        .unwrap();

    Ok(Css(css))
}
