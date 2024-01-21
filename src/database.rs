use crate::model::BlogPost;
use sqlx::types::time::PrimitiveDateTime;
use sqlx::{query_as, PgExecutor};
use thiserror::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("Database returned unexpected data")]
    UnexpectedData,
}

struct BlogPostRecord {
    url: String,
    title: String,
    markdown: Option<String>,
    html: String,
    tags: Option<Vec<String>>,
    accessible: bool,
    publication_date: Option<PrimitiveDateTime>,
}

impl TryFrom<BlogPostRecord> for BlogPost {
    type Error = Error;

    fn try_from(
        BlogPostRecord {
            url,
            title,
            markdown,
            html,
            tags,
            accessible,
            publication_date,
        }: BlogPostRecord,
    ) -> Result<Self> {
        let tags = tags.unwrap_or(Vec::new());
        let publication_date =
            publication_date.map(|primitive_date_time| primitive_date_time.assume_utc().into());

        Ok(BlogPost {
            url,
            title,
            markdown,
            html,
            tags,
            accessible,
            publication_date,
        })
    }
}

pub async fn get_blog_post<'c, E: PgExecutor<'c>>(
    url: &str,
    executor: E,
) -> Result<Option<BlogPost>> {
    query_as!(
        BlogPostRecord,
        "SELECT url, title, markdown, html, accessible, publication_date, array_agg(tag) as tags \
         FROM blog_post NATURAL JOIN tag \
         WHERE url=$1 \
         GROUP BY url",
        url
    )
    .fetch_optional(executor)
    .await?
    .map(BlogPost::try_from)
    .transpose()
}

pub async fn get_accessible_blog_post<'c, E: PgExecutor<'c>>(
    url: &str,
    executor: E,
) -> Result<Option<BlogPost>> {
    query_as!(
        BlogPostRecord,
        "SELECT url, title, markdown, html, accessible, publication_date, array_agg(tag) as tags \
         FROM blog_post NATURAL JOIN tag \
         WHERE url=$1 AND accessible IS TRUE \
         GROUP BY url",
        url
    )
    .fetch_optional(executor)
    .await?
    .map(BlogPost::try_from)
    .transpose()
}

pub async fn get_all_blog_posts<'c, E: PgExecutor<'c>>(executor: E) -> Result<Vec<BlogPost>> {
    query_as!(
        BlogPostRecord,
        "SELECT url, title, markdown, html, accessible, publication_date, array_agg(tag) as tags \
        FROM blog_post NATURAL JOIN tag \
        GROUP BY url"
    )
    .fetch_all(executor)
    .await?
    .into_iter()
    .map(BlogPost::try_from)
    .collect()
}

pub async fn get_public_blog_posts<'c, E: PgExecutor<'c>>(executor: E) -> Result<Vec<BlogPost>> {
    // TODO: support publish date in the future
    query_as!(
        BlogPostRecord,
        "SELECT url, title, markdown, html, accessible, publication_date, array_agg(tag) as tags \
        FROM blog_post NATURAL JOIN tag \
        WHERE accessible IS NOT FALSE AND publication_date IS NOT NULL \
        GROUP BY url"
    )
    .fetch_all(executor)
    .await?
    .into_iter()
    .map(BlogPost::try_from)
    .collect()
}

// TODO: Tests
