use crate::model::Blog;
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

struct BlogRecord {
    url: String,
    title: String,
    markdown: Option<String>,
    html: String,
    tags: String,
    accessible: i64,
    date_of_publication: Option<i64>,
}

impl TryFrom<BlogRecord> for Blog {
    type Error = Error;

    fn try_from(
        BlogRecord {
            url,
            title,
            markdown,
            html,
            tags,
            accessible,
            date_of_publication,
        }: BlogRecord,
    ) -> Result<Self> {
        let tags = tags.split(',').map(String::from).collect();
        let accessible = accessible != 0;
        let date_of_publication = date_of_publication
            .map(|secs| SystemTime::UNIX_EPOCH + Duration::from_secs(secs as u64));

        Ok(Blog {
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

pub async fn get_blog<'c, E: SqliteExecutor<'c>>(url: &str, executor: E) -> Result<Blog> {
    query_as!(
        BlogRecord,
        "SELECT url, title, markdown, html, tags, accessible, date_of_publication \
         FROM blog \
         WHERE url=?",
        url
    )
    .fetch_one(executor)
    .await?
    .try_into()
}

pub async fn get_all_blogs<'c, E: SqliteExecutor<'c>>(executor: E) -> Result<Vec<Blog>> {
    query_as!(
        BlogRecord,
        "SELECT url, title, markdown, html, tags, accessible, date_of_publication \
        FROM blog"
    )
    .fetch_all(executor)
    .await?
    .into_iter()
    .map(Blog::try_from)
    .collect()
}

pub async fn get_public_blogs<'c, E: SqliteExecutor<'c>>(executor: E) -> Result<Vec<Blog>> {
    query_as!(
        BlogRecord,
        "SELECT url, title, markdown, html, tags, accessible, date_of_publication \
        FROM blog \
        WHERE accessible != 0 AND date_of_publication != NULL"
    )
    .fetch_all(executor)
    .await?
    .into_iter()
    .map(Blog::try_from)
    .collect()
}
