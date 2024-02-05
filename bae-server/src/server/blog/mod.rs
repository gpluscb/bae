pub mod templates;

use crate::server::{Error, Result};
use crate::{AppState, StandardCodeBlockHighlighter};
use askama::Template;
use axum::extract::State;
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use axum_extra::routing::{RouterExt, TypedPath};
use bae_common::blog::Tag;
use bae_common::database;
use bae_common::markdown_render::render_md_to_html;
use serde::Deserialize;
use sqlx::PgPool;
use std::sync::Arc;
use templates::{BlogPostTemplate, HomeTemplate, TaggedTemplate, TagsTemplate, TestTemplate};

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
    let markdown = r#"Hi **bold** _italic_

| Table | Yeah |
| ----- | ---- |
| Thing | Woo  |
| Row   | No 2 |

And have some code!!

```rs
// This is just some code I copied from wherever in my project
pub async fn get_public_blog_posts_for_tag<'c, E: PgExecutor<'c>>(
    tag: &Tag,
    executor: E,
) -> Result<Vec<BlogPost>> {
    query_as!(
        BlogPostRecord,
        "SELECT url, title, description, author, markdown, html, reading_time_minutes, \
            accessible, publication_date, array_remove(array_agg(tag), NULL) as tags \
        FROM blog_post NATURAL JOIN tag \
        WHERE publication_date IS NOT NULL \
            AND publication_date <= now() \
        GROUP BY url \
        HAVING bool_or(tag=$1) \
        ORDER BY publication_date DESC",
        tag.0,
    )
    .fetch(executor)
    .map_err(Error::from)
    .map(|result| result.and_then(BlogPost::try_from))
    .try_collect()
    .await
}
```"#;

    let test_md_rendered = render_md_to_html(markdown, &comrak_options, &highlighter);
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
#[typed_path("/blog/:post_url", rejection(Error))]
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
