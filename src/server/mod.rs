use crate::templates::{ErrorTemplate, HomeTemplate};
use crate::AppState;
use askama::Template;
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
    #[error("Askama error: {0}")]
    Askama(#[from] askama::Error),
}

impl Error {
    pub fn status(&self) -> StatusCode {
        match self {
            Error::Askama(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        error!(error = %self, "Replying with error");

        let status = self.status();

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
    Router::new().typed_get(home)
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/")]
pub struct HomePath {}

pub async fn home(_path: HomePath) -> Result<Html<String>> {
    let html = HomeTemplate {}.render()?;
    Ok(Html(html))
}
