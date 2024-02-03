pub mod templates;

use crate::markdown_render::render_md_to_html;
use crate::model::Tag;
use crate::server::blog::templates::{
    BlogPostTemplate, TaggedTemplate, TagsTemplate, TestTemplate,
};
use crate::server::{Error, Result};
use crate::{database, AppState, StandardCodeBlockHighlighter};
use askama::Template;
use axum::extract::State;
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use axum_extra::routing::{RouterExt, TypedPath};
use serde::Deserialize;
use sqlx::PgPool;
use std::sync::Arc;
use templates::HomeTemplate;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/blog/test", get(test))
        .typed_get(home)
        .typed_get(blog_post)
        .typed_get(tagged)
        .typed_get(tags)
}

pub async fn test(
    State(comrak_options): State<Arc<comrak::Options>>,
    State(highlighter): State<Arc<StandardCodeBlockHighlighter>>,
) -> Result<Html<String>> {
    let markdown = format!(
        r#"Hi **bold** _italic_

| Table | Yeah |
| ----- | ---- |
| Thing | Woo  |
| Line  | No 2 |

```rs
fn number_example() {{
    // Also a nice comment example
    let i = 20;
}}

{}
```"#,
        include_str!("mod.rs")
    );

    let test_md_rendered = render_md_to_html(&markdown, &comrak_options, &highlighter);
    let html = TestTemplate { test_md_rendered }.render()?;

    Ok(Html(html))
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/blog", rejection(Error))]
pub struct HomePath {}

pub async fn home(HomePath {}: HomePath, State(database): State<PgPool>) -> Result<Html<String>> {
    let blog_posts = database::get_public_blog_posts(&database).await?;

    let html = HomeTemplate { blog_posts }.render()?;
    Ok(Html(html))
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/blog/post/:post_url", rejection(Error))]
pub struct BlogPostPath {
    pub post_url: String,
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

#[derive(TypedPath, Deserialize)]
#[typed_path("/blog/tagged/:tag", rejection(Error))]
pub struct TaggedPath {
    pub tag: Tag,
}

pub async fn tagged(
    TaggedPath { tag }: TaggedPath,
    State(database): State<PgPool>,
) -> Result<Html<String>> {
    let blog_posts = database::get_public_blog_posts_for_tag(&tag, &database).await?;

    let html = TaggedTemplate { tag, blog_posts }.render()?;
    Ok(Html(html))
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/blog/tags", rejection(Error))]
pub struct TagsPath {}

pub async fn tags(TagsPath {}: TagsPath, State(database): State<PgPool>) -> Result<Html<String>> {
    let tags = database::get_tags(&database).await?;

    let html = TagsTemplate { tags }.render()?;
    Ok(Html(html))
}
