pub mod templates;

use crate::server::{Error, Result};
use crate::{database, AppState};
use askama::Template;
use axum::extract::State;
use axum::response::Html;
use axum::Router;
use axum_extra::routing::{RouterExt, TypedPath};
use serde::Deserialize;
use sqlx::SqlitePool;
use templates::HomeTemplate;

pub fn router() -> Router<AppState> {
    Router::new().typed_get(home)
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/", rejection(Error))]
pub struct HomePath {}

pub async fn home(_path: HomePath, State(db): State<SqlitePool>) -> Result<Html<String>> {
    let blogs = database::get_public_blogs(&db).await?;

    let html = HomeTemplate { blogs }.render()?;
    Ok(Html(html))
}
