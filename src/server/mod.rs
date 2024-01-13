use crate::templates::{ErrorTemplate, HomeTemplate};
use crate::AppState;
use askama::Template;
use axum::extract::rejection::PathRejection;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::Router;
use axum_extra::routing::{RouterExt, TypedPath};
use serde::Deserialize;
use thiserror::Error;
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
}

impl Error {
    pub fn status(&self) -> StatusCode {
        match self {
            Error::NotFound | Error::Path(_) => StatusCode::NOT_FOUND,
            Error::Askama(_) => StatusCode::INTERNAL_SERVER_ERROR,
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
        .fallback(|| async { Error::NotFound })
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/", rejection(Error))]
pub struct HomePath {}

pub async fn home(_path: HomePath) -> Result<Html<String>> {
    let html = HomeTemplate {}.render()?;
    Ok(Html(html))
}
