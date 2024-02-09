use crate::blog::BlogPost;
use chrono::{DateTime, Duration, Utc};
use futures::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use sqlx::migrate::Migrate;
use sqlx::{migrate, query, query_as, query_scalar, Acquire, PgExecutor, Postgres, Transaction};
use std::ops::Deref;
use thiserror::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Sqlx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("Database returned unexpected data")]
    UnexpectedData,
    #[error("Input was invalid")]
    InvalidInput,
}

#[derive(Clone, Eq, PartialEq, Debug, sqlx::Type, Serialize, Deserialize)]
#[sqlx(transparent, type_name = "text")]
pub struct Author(pub String);

impl From<String> for Author {
    fn from(author: String) -> Self {
        Author(author)
    }
}

#[derive(Clone, Eq, PartialEq, Debug, sqlx::Type, Serialize, Deserialize)]
#[sqlx(transparent, type_name = "text")]
pub struct Tag(pub String);

impl From<String> for Tag {
    fn from(tag: String) -> Self {
        Tag(tag)
    }
}

struct BlogPostRecord {
    url: String,
    title: String,
    description: String,
    author: Author,
    markdown: Option<String>,
    html: String,
    tags: Option<Vec<String>>,
    reading_time_minutes: i64,
    accessible: bool,
    publication_date: Option<DateTime<Utc>>,
}

impl TryFrom<BlogPostRecord> for BlogPost {
    type Error = Error;

    fn try_from(
        BlogPostRecord {
            url,
            title,
            description,
            author,
            markdown,
            html,
            tags,
            reading_time_minutes,
            accessible,
            publication_date,
        }: BlogPostRecord,
    ) -> Result<Self> {
        let tags = tags.unwrap_or_default().into_iter().map(Tag).collect();
        let reading_time =
            Duration::try_minutes(reading_time_minutes).ok_or(Error::UnexpectedData)?;

        Ok(BlogPost {
            url,
            title,
            description,
            author,
            markdown,
            html,
            tags,
            reading_time,
            accessible,
            publication_date,
        })
    }
}

pub async fn migrate<'a, A>(migrator: A) -> Result<()>
where
    A: Acquire<'a>,
    <A::Connection as Deref>::Target: Migrate,
{
    migrate!()
        .run(migrator)
        .await
        .map_err(sqlx::Error::from)
        .map_err(Error::from)
}

pub async fn get_blog_post<'c, E: PgExecutor<'c>>(
    url: &str,
    executor: E,
) -> Result<Option<BlogPost>> {
    query_as!(
        BlogPostRecord,
        "SELECT url, title, description, author, markdown, html, reading_time_minutes, \
            accessible, publication_date, array_remove(array_agg(tag), NULL) as tags \
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
        "SELECT url, title, description, author, markdown, html, reading_time_minutes, \
            accessible, publication_date, array_remove(array_agg(tag), NULL) as tags \
        FROM blog_post NATURAL LEFT JOIN tag \
        WHERE url=$1 AND (accessible OR \
            (publication_date IS NOT NULL \
            AND publication_date <= now())) \
        GROUP BY url",
        url
    )
    .fetch_optional(executor)
    .await?
    .map(BlogPost::try_from)
    .transpose()
}

pub async fn get_blog_posts<'c, E: PgExecutor<'c>>(
    authors: Option<&[Author]>,
    tags: Option<&[Tag]>,
    published_only: bool,
    executor: E,
) -> Result<Vec<BlogPost>> {
    let no_author_filtering = authors.is_none();
    let no_tag_filtering = tags.is_none();
    let no_public_filtering = !published_only;

    query_as!(
        BlogPostRecord,
        "SELECT url, title, description, author, markdown, html, reading_time_minutes, \
            accessible, publication_date, array_remove(array_agg(tag), NULL) as tags \
        FROM blog_post NATURAL LEFT JOIN tag \
        WHERE \
            ($1 OR author = ANY($2)) \
            AND ($3 OR (publication_date IS NOT NULL \
                AND publication_date <= now())) \
        GROUP BY url \
        HAVING $4 OR bool_or(tag = ANY($5)) \
        ORDER BY publication_date DESC",
        no_author_filtering,
        &authors.unwrap_or_default() as &[Author],
        no_public_filtering,
        no_tag_filtering,
        &tags.unwrap_or_default() as &[Tag],
    )
    .fetch(executor)
    .map_err(Error::from)
    .map(|result| result.and_then(BlogPost::try_from))
    .try_collect()
    .await
}

pub async fn get_tags<'c, E: PgExecutor<'c>>(executor: E) -> Result<Vec<Tag>> {
    query_scalar!(
        "SELECT DISTINCT tag \
        FROM tag \
        ORDER BY tag ASC"
    )
    .fetch(executor)
    .map_ok(Tag)
    .try_collect()
    .await
    .map_err(Error::from)
}

pub async fn insert_blog_post<'c>(
    BlogPost {
        url,
        title,
        description,
        author,
        markdown,
        html,
        tags,
        reading_time,
        accessible,
        publication_date,
    }: &BlogPost,
    new_author: bool,
    transaction: &mut Transaction<'c, Postgres>,
) -> Result<()> {
    // Insert author (if new)
    if new_author {
        query!(
            "INSERT INTO author (author) \
            VALUES ($1)",
            author.0,
        )
        .execute(&mut **transaction)
        .await?;
    }

    // Insert blog post
    query!(
        "INSERT INTO blog_post \
            (url, title, description, author, markdown, html, reading_time_minutes, \
                accessible, publication_date) \
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        url,
        title,
        description,
        author.0,
        markdown.as_ref(),
        html,
        i32::try_from(reading_time.num_minutes()).map_err(|_| Error::InvalidInput)?,
        accessible,
        publication_date.as_ref(),
    )
    .execute(&mut **transaction)
    .await?;

    // Insert tags
    for Tag(tag) in tags {
        query!(
            "INSERT INTO tag (tag, url) \
            VALUES ($1, $2)",
            tag,
            url,
        )
        .execute(&mut **transaction)
        .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::blog::BlogPost;
    use crate::database::Author;
    use chrono::Duration;
    use sqlx::types::chrono::Utc;
    use sqlx::PgPool;

    fn is_sorted<T: PartialOrd, I: Iterator<Item = T>>(iter: &mut I) -> bool {
        let Some(mut previous) = iter.next() else {
            return true;
        };
        iter.all(|next| {
            if previous <= next {
                previous = next;
                true
            } else {
                false
            }
        })
    }

    #[sqlx::test(fixtures(path = "../test_fixtures", scripts("authors", "blog_posts")))]
    pub async fn blog_post_tests(pool: PgPool) -> super::Result<()> {
        // Test if all the data looks alright

        let public_post = super::get_blog_post("public", &pool)
            .await?
            .expect("blog post 'public' not found");

        let expected_public_post = BlogPost {
            url: "public".to_string(),
            title: "Test (Public)".to_string(),
            description: "No description".to_string(),
            author: Author("Quiet".to_string()),
            markdown: Some("test *bold*".to_string()),
            html: "test <b>bold</b>".to_string(),
            tags: vec![],
            reading_time: Duration::minutes(1),
            accessible: false,
            publication_date: public_post.publication_date,
        };
        assert_eq!(public_post, expected_public_post);
        assert!(public_post.publication_date.unwrap() < Utc::now());

        let accessible_post = super::get_blog_post("accessible", &pool)
            .await?
            .expect("blog post 'accessible' not found");

        let expected_accessible_post = BlogPost {
            url: "accessible".to_string(),
            title: "Test (Accessible)".to_string(),
            description: "No description".to_string(),
            author: Author("Quiet".to_string()),
            markdown: Some("test2".to_string()),
            html: "test2".to_string(),
            tags: vec![],
            reading_time: Duration::minutes(1),
            accessible: true,
            publication_date: None,
        };
        assert_eq!(accessible_post, expected_accessible_post);

        let inaccessible_post = super::get_blog_post("not_accessible", &pool)
            .await?
            .expect("blog post 'not_accessible' not found");

        let expected_inaccessible_post = BlogPost {
            url: "not_accessible".to_string(),
            title: "Test (Not Accessible)".to_string(),
            description: "No description".to_string(),
            author: Author("Quiet".to_string()),
            markdown: Some("test3".to_string()),
            html: "test3".to_string(),
            tags: vec![],
            reading_time: Duration::minutes(1),
            accessible: false,
            publication_date: None,
        };
        assert_eq!(inaccessible_post, expected_inaccessible_post);

        let future_public_post = super::get_blog_post("public_in_future", &pool)
            .await?
            .expect("blog post 'public_in_future' not found");

        let expected_future_public_post = BlogPost {
            url: "public_in_future".to_string(),
            title: "Test (Public in future)".to_string(),
            description: "No description".to_string(),
            author: Author("Quiet".to_string()),
            markdown: Some("test4".to_string()),
            html: "test4".to_string(),
            tags: vec![],
            reading_time: Duration::minutes(1),
            accessible: false,
            publication_date: future_public_post.publication_date,
        };
        assert_eq!(future_public_post, expected_future_public_post);
        // I know technically this is not safe because we could hang or whatever
        // but the sql sets the date to 100 years in the future, so yea we should be fine
        assert!(future_public_post.publication_date.unwrap() > Utc::now());

        let accessible_future_public_post =
            super::get_blog_post("accessible_public_in_future", &pool)
                .await?
                .expect("blog post 'accessible_public_in_future' not found");

        let expected_accessible_future_public_post = BlogPost {
            url: "accessible_public_in_future".to_string(),
            title: "Test (Accessible, Public in future)".to_string(),
            description: "No description".to_string(),
            author: Author("Quiet".to_string()),
            markdown: Some("test5".to_string()),
            html: "test5".to_string(),
            tags: vec![],
            reading_time: Duration::minutes(1),
            accessible: true,
            publication_date: accessible_future_public_post.publication_date,
        };
        assert_eq!(
            accessible_future_public_post,
            expected_accessible_future_public_post,
        );
        // I know technically we could hang, but sql date should be 100 years in the future
        assert!(accessible_future_public_post.publication_date.unwrap() > Utc::now());

        let long_post = super::get_blog_post("long_post", &pool)
            .await?
            .expect("blog post 'long_post' not found");

        let expected_long_post = BlogPost {
            url: "long_post".to_string(),
            title: "Test (Longer blog post)".to_string(),
            description: "No description".to_string(),
            author: Author("gpluscb".to_string()),
            markdown: long_post.markdown.clone(),
            html: long_post.html.clone(),
            tags: vec![],
            reading_time: Duration::minutes(60),
            accessible: true,
            publication_date: long_post.publication_date,
        };
        assert_eq!(long_post, expected_long_post);
        assert!(expected_long_post.publication_date.unwrap() < Utc::now());
        assert_eq!(expected_long_post.markdown.as_ref().unwrap().len(), 10573);
        assert_eq!(expected_long_post.html.len(), 10573);

        assert!(public_post.publication_date.unwrap() < long_post.publication_date.unwrap());
        assert!(
            future_public_post.publication_date.unwrap()
                < accessible_future_public_post.publication_date.unwrap()
        );

        // Test if the function get_accessible_blog_post correctly filters out inaccessible ones

        assert_eq!(
            super::get_accessible_blog_post(&public_post.url, &pool)
                .await?
                .unwrap(),
            expected_public_post,
        );
        assert_eq!(
            super::get_accessible_blog_post(&accessible_post.url, &pool)
                .await?
                .unwrap(),
            expected_accessible_post,
        );
        assert!(
            super::get_accessible_blog_post(&inaccessible_post.url, &pool)
                .await?
                .is_none()
        );
        assert!(
            super::get_accessible_blog_post(&future_public_post.url, &pool)
                .await?
                .is_none()
        );
        assert_eq!(
            super::get_accessible_blog_post(&accessible_future_public_post.url, &pool)
                .await?
                .unwrap(),
            expected_accessible_future_public_post,
        );
        assert_eq!(
            super::get_accessible_blog_post(&long_post.url, &pool)
                .await?
                .unwrap(),
            expected_long_post,
        );

        // Test get_blog_posts for all blog posts, in particular order

        let all_blog_posts = super::get_blog_posts(None, None, false, &pool).await?;
        assert!([
            &expected_public_post,
            &expected_accessible_post,
            &expected_inaccessible_post,
            &expected_future_public_post,
            &expected_accessible_future_public_post,
            &expected_long_post
        ]
        .into_iter()
        .all(|blog_post| all_blog_posts.contains(blog_post)));
        assert_eq!(all_blog_posts.len(), 6);
        assert!(is_sorted(
            &mut all_blog_posts
                .iter()
                .flat_map(|post| post.publication_date)
                .rev()
        ));
        // We also assert that all posts without publication dates are at the start
        let mut first_publication_date_found = false;
        assert!(all_blog_posts.iter().all(|post| {
            if post.publication_date.is_some() {
                // Found a publication date
                first_publication_date_found = true;
                true
            } else {
                // Otherwise, all previous publication dates must have been Nones
                !first_publication_date_found
            }
        }));

        // Test get_blog_posts for public blog posts, in particular order

        let public_blog_posts = super::get_blog_posts(None, None, true, &pool).await?;
        assert!([&expected_public_post, &expected_long_post]
            .into_iter()
            .all(|post| public_blog_posts.contains(post)));
        assert_eq!(public_blog_posts.len(), 2);
        assert!(is_sorted(
            &mut public_blog_posts
                .iter()
                .map(|post| post.publication_date.unwrap())
                .rev()
        ));

        Ok(())
    }

    // TODO: Add tags tests
    // TODO: Add authors tests
    // TODO: Add insert tests
}
