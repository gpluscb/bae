use crate::model::{BlogPost, Tag};
use sqlx::types::chrono::NaiveDateTime;
use sqlx::{query_as, query_scalar, PgExecutor};
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
    publication_date: Option<NaiveDateTime>,
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
        let tags = tags.unwrap_or(Vec::new()).into_iter().map(Tag).collect();
        let publication_date = publication_date.map(|timestamp| timestamp.and_utc());

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
        "SELECT url, title, markdown, html, accessible, publication_date, array_remove(array_agg(tag), NULL) as tags \
         FROM blog_post NATURAL LEFT JOIN tag \
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
        "SELECT url, title, markdown, html, accessible, publication_date, array_remove(array_agg(tag), NULL) as tags \
         FROM blog_post NATURAL LEFT JOIN tag \
         WHERE url=$1 AND accessible \
         GROUP BY url",
        url
    )
    .fetch_optional(executor)
    .await?
    .map(BlogPost::try_from)
    .transpose()
}

// TODO: Can we get around all these fetch_all -> into_iter -> collects?
pub async fn get_all_blog_posts<'c, E: PgExecutor<'c>>(executor: E) -> Result<Vec<BlogPost>> {
    query_as!(
        BlogPostRecord,
        "SELECT url, title, markdown, html, accessible, publication_date, array_remove(array_agg(tag), NULL) as tags \
        FROM blog_post NATURAL LEFT JOIN tag \
        GROUP BY url \
        ORDER BY publication_date DESC"
    )
    .fetch_all(executor)
    .await?
    .into_iter()
    .map(BlogPost::try_from)
    .collect()
}

pub async fn get_public_blog_posts<'c, E: PgExecutor<'c>>(executor: E) -> Result<Vec<BlogPost>> {
    query_as!(
        BlogPostRecord,
        "SELECT url, title, markdown, html, accessible, publication_date, array_remove(array_agg(tag), NULL) as tags \
        FROM blog_post NATURAL LEFT JOIN tag \
        WHERE accessible AND publication_date IS NOT NULL \
            AND publication_date <= now() at time zone('utc') \
        GROUP BY url \
        ORDER BY publication_date DESC"
    )
    .fetch_all(executor)
    .await?
    .into_iter()
    .map(BlogPost::try_from)
    .collect()
}

pub async fn get_public_blog_posts_for_tag<'c, E: PgExecutor<'c>>(
    tag: &Tag,
    executor: E,
) -> Result<Vec<BlogPost>> {
    query_as!(
        BlogPostRecord,
        "SELECT url, title, markdown, html, accessible, publication_date, array_agg(tag) as tags \
        FROM blog_post NATURAL JOIN tag \
        WHERE accessible AND publication_date IS NOT NULL \
            AND publication_date <= now() at time zone('utc') \
            AND tag=$1 \
        GROUP BY url \
        ORDER BY publication_date DESC",
        tag.0,
    )
    .fetch_all(executor)
    .await?
    .into_iter()
    .map(BlogPost::try_from)
    .collect()
}

pub async fn get_tags<'c, E: PgExecutor<'c>>(executor: E) -> Result<Vec<Tag>> {
    let tags = query_scalar!(
        "SELECT DISTINCT tag \
        FROM tag \
        ORDER BY tag ASC"
    )
    .fetch_all(executor)
    .await?
    .into_iter()
    .map(Tag)
    .collect();

    Ok(tags)
}

// TODO: Tests
