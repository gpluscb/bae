pub mod templates;

use crate::model::ServerPathExt;
use crate::server::util::Xml;
use crate::server::{Error, Result};
use crate::{AppState, BaseUri};
use askama::Template;
use axum::extract::Request;
use axum::extract::State;
use axum::response::Html;
use axum::Router;
use axum_extra::extract::Query;
use axum_extra::routing::{RouterExt, TypedPath};
use bae_common::database;
use bae_common::database::{Author, Tag};
use rss::{CategoryBuilder, ChannelBuilder, GuidBuilder, ItemBuilder, SourceBuilder};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use templates::{BlogPostTemplate, HomeTemplate, TaggedTemplate, TagsTemplate};

pub fn router() -> Router<AppState> {
    Router::new()
        .typed_get(home)
        .typed_get(blog_post)
        .typed_get(tagged)
        .typed_get(tags)
        .typed_get(rss)
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/blog", rejection(Error))]
pub struct HomePath {}

pub async fn home(HomePath {}: HomePath, State(database): State<PgPool>) -> Result<Html<String>> {
    let blog_posts = database::get_blog_posts(None, None, true, &database).await?;

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
    let blog_post = database::get_blog_post(&post_url, true, &database)
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
    let blog_posts = database::get_blog_posts(None, Some(&[tag.clone()]), true, &database).await?;

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

#[derive(Copy, Clone, TypedPath, Deserialize)]
#[typed_path("/blog/rss.xml", rejection(Error))]
pub struct RssPath {}

#[derive(Clone, Serialize, Deserialize)]
pub struct RssQueryParams {
    #[serde(default)]
    tags: Vec<Tag>,
    #[serde(default)]
    authors: Vec<Author>,
}

pub async fn rss(
    RssPath {}: RssPath,
    Query(params): Query<RssQueryParams>,
    State(database): State<PgPool>,
    State(BaseUri(base_uri)): State<BaseUri>,
    request: Request,
) -> Result<Xml<String>> {
    let current_path = request.uri().path();

    let tags = (!params.tags.is_empty()).then_some(params.tags);
    let authors = (!params.authors.is_empty()).then_some(params.authors);

    let blog_posts =
        database::get_blog_posts(authors.as_deref(), tags.as_deref(), true, &database).await?;

    let mut channel = ChannelBuilder::default();

    channel
        .title("Bae")
        .link(format!("{base_uri}{}", HomePath {}))
        .description("The RSS feed for the blog part of bae (blog and eh).")
        .language("en-gb".to_string())
        .managing_editor("marrueeee@gmail.com".to_string())
        .webmaster("marrueeee@gmail.com".to_string())
        .last_build_date("TODO".to_string())
        .category(CategoryBuilder::default().name("Programming").build())
        .category(CategoryBuilder::default().name("IT").build())
        .category(CategoryBuilder::default().name("Technology").build())
        .generator("https://crates.io/crates/rss".to_string())
        .ttl((60 * 24).to_string())
        .image(None); // TODO

    for blog_post in blog_posts {
        let full_url = format!("{base_uri}{}", blog_post.full_path());

        let categories = blog_post
            .tags
            .into_iter()
            .map(|tag| {
                CategoryBuilder::default()
                    .domain(format!("{base_uri}{}", tag.full_path()))
                    .name(tag.0)
                    .build()
            })
            .collect::<Vec<_>>();

        let guid = GuidBuilder::default()
            .value(&full_url)
            .permalink(true)
            .build();

        let pub_date = blog_post
            .publication_date
            .ok_or(Error::InvalidDatabaseReturn)?
            .to_rfc2822();

        let source = SourceBuilder::default()
            .url(format!("{base_uri}{current_path}"))
            .title("Bae".to_string())
            .build();

        let item = ItemBuilder::default()
            .title(blog_post.title)
            .link(full_url)
            .description(blog_post.description)
            .author(blog_post.author.0)
            .categories(categories)
            .guid(guid)
            .pub_date(pub_date)
            .source(source)
            .content(blog_post.html)
            .build();

        channel.item(item);
    }

    Ok(Xml(channel.build().to_string()))
}
