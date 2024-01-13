use crate::model::BlogPost;
use sqlx::{query_as, SqliteExecutor};
use std::time::{Duration, SystemTime};
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
    tags: String,
    accessible: i64,
    date_of_publication: Option<i64>,
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
            date_of_publication,
        }: BlogPostRecord,
    ) -> Result<Self> {
        let tags = tags.split(',').map(String::from).collect();
        let accessible = accessible != 0;
        let date_of_publication = date_of_publication
            .map(|secs| SystemTime::UNIX_EPOCH + Duration::from_secs(secs as u64));

        Ok(BlogPost {
            url,
            title,
            markdown,
            html,
            tags,
            accessible,
            date_of_publication,
        })
    }
}

pub async fn get_blog_post<'c, E: SqliteExecutor<'c>>(url: &str, executor: E) -> Result<BlogPost> {
    query_as!(
        BlogPostRecord,
        "SELECT url, title, markdown, html, tags, accessible, date_of_publication \
         FROM blog_post \
         WHERE url=?",
        url
    )
    .fetch_one(executor)
    .await?
    .try_into()
}

pub async fn get_all_blog_posts<'c, E: SqliteExecutor<'c>>(executor: E) -> Result<Vec<BlogPost>> {
    query_as!(
        BlogPostRecord,
        "SELECT url, title, markdown, html, tags, accessible, date_of_publication \
        FROM blog_post"
    )
    .fetch_all(executor)
    .await?
    .into_iter()
    .map(BlogPost::try_from)
    .collect()
}

pub async fn get_public_blog_posts<'c, E: SqliteExecutor<'c>>(
    executor: E,
) -> Result<Vec<BlogPost>> {
    query_as!(
        BlogPostRecord,
        "SELECT url, title, markdown, html, tags, accessible, date_of_publication \
        FROM blog_post \
        WHERE accessible IS NOT 0 AND date_of_publication NOT NULL"
    )
    .fetch_all(executor)
    .await?
    .into_iter()
    .map(BlogPost::try_from)
    .collect()
}
