pub mod templates;

use crate::server::blog::templates::BlogPostTemplate;
use crate::server::{Error, Result};
use crate::{database, AppState};
use askama::Template;
use axum::extract::State;
use axum::response::Html;
use axum::Router;
use axum_extra::routing::{RouterExt, TypedPath};
use serde::Deserialize;
use sqlx::PgPool;
use templates::HomeTemplate;

pub fn router() -> Router<AppState> {
    Router::new().typed_get(home).typed_get(blog_post)
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/", rejection(Error))]
pub struct HomePath {}

pub async fn home(HomePath {}: HomePath, State(database): State<PgPool>) -> Result<Html<String>> {
    let blog_posts = database::get_public_blog_posts(&database).await?;

    let html = HomeTemplate { blog_posts }.render()?;
    Ok(Html(html))
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/:post_url", rejection(Error))]
pub struct BlogPostPath {
    post_url: String,
}

pub async fn blog_post(
    BlogPostPath { post_url }: BlogPostPath,
    State(database): State<PgPool>,
) -> Result<Html<String>> {
    let blog_post = database::get_accessible_blog_post(&post_url, &database)
        .await?
        .ok_or(Error::NotFound)?;

    let html = BlogPostTemplate { blog_post }.render()?;
    Ok(Html(html))
}
