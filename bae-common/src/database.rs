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

#[derive(
    Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Hash, sqlx::Type, Serialize, Deserialize,
)]
#[sqlx(transparent, type_name = "text")]
pub struct Author(pub String);

impl From<String> for Author {
    fn from(author: String) -> Self {
        Author(author)
    }
}

#[derive(
    Clone, Ord, PartialOrd, Eq, PartialEq, Debug, Hash, sqlx::Type, Serialize, Deserialize,
)]
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
    accessible_only: bool,
    executor: E,
) -> Result<Option<BlogPost>> {
    let no_accessible_filtering = !accessible_only;

    query_as!(
        BlogPostRecord,
        "SELECT url, title, description, author, markdown, html, reading_time_minutes, \
            accessible, publication_date, array_remove(array_agg(tag), NULL) as tags \
        FROM blog_post NATURAL LEFT JOIN tag \
        WHERE url=$1 AND ($2 OR (accessible OR \
            (publication_date IS NOT NULL \
            AND publication_date <= now()))) \
        GROUP BY url",
        url,
        no_accessible_filtering,
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

pub async fn get_tags<'c, E: PgExecutor<'c>>(
    published_only: bool,
    executor: E,
) -> Result<Vec<Tag>> {
    let no_public_filtering = !published_only;

    query_scalar!(
        "SELECT tag \
        FROM tag NATURAL LEFT JOIN blog_post WHERE \
            ($1 OR (publication_date IS NOT NULL \
                AND publication_date <= now())) \
        GROUP BY tag \
        ORDER BY tag ASC",
        no_public_filtering,
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

pub async fn update_blog_post<'c>(
    original_url: Option<&str>,
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
    let original_url = original_url.unwrap_or(url);

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

    // Remove old tags
    query!(
        "DELETE FROM tag \
        WHERE tag = ANY($1) AND url = $2",
        &tags.iter().map(|Tag(tag)| tag.clone()).collect::<Vec<_>>(),
        original_url,
    )
    .execute(&mut **transaction)
    .await?;

    // Update blog post
    query!(
        "UPDATE blog_post \
        SET url=$1, title=$2, description=$3, author=$4, markdown=$5, html=$6, \
            reading_time_minutes=$7, accessible=$8, publication_date=$9 \
        WHERE url = $10",
        url,
        title,
        description,
        author.0,
        markdown.as_ref(),
        html,
        i32::try_from(reading_time.num_minutes()).map_err(|_| Error::InvalidInput)?,
        accessible,
        publication_date.as_ref(),
        original_url,
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
    use crate::database::{Author, Tag};
    use chrono::{DateTime, Duration};
    use itertools::Itertools;
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

    struct ExpectedBlogPosts {
        public: BlogPost,
        accessible: BlogPost,
        not_accessible: BlogPost,
        public_in_future: BlogPost,
        accessible_public_in_future: BlogPost,
        long_post: BlogPost,
    }

    impl ExpectedBlogPosts {
        /// All the blog posts the test fixtures insert, assuming `authors`,
        /// `blog_posts`, and `tags` fixtures are ran.
        fn new() -> Self {
            ExpectedBlogPosts {
                public: BlogPost {
                    url: "public".to_string(),
                    title: "Test (Public)".to_string(),
                    description: "No description".to_string(),
                    author: Author("Quiet".to_string()),
                    markdown: Some("test *bold*".to_string()),
                    html: "test <b>bold</b>".to_string(),
                    tags: vec![Tag("public".to_string()), Tag("post".to_string())],
                    reading_time: Duration::minutes(1),
                    accessible: false,
                    publication_date: Some(DateTime::UNIX_EPOCH),
                },
                accessible: BlogPost {
                    url: "accessible".to_string(),
                    title: "Test (Accessible)".to_string(),
                    description: "No description".to_string(),
                    author: Author("Quiet".to_string()),
                    markdown: Some("test2".to_string()),
                    html: "test2".to_string(),
                    tags: vec![Tag("post".to_string())],
                    reading_time: Duration::minutes(1),
                    accessible: true,
                    publication_date: None,
                },
                not_accessible: BlogPost {
                    url: "not_accessible".to_string(),
                    title: "Test (Not Accessible)".to_string(),
                    description: "No description".to_string(),
                    author: Author("Quiet".to_string()),
                    markdown: Some("test3".to_string()),
                    html: "test3".to_string(),
                    tags: vec![Tag("post".to_string())],
                    reading_time: Duration::minutes(1),
                    accessible: false,
                    publication_date: None,
                },
                public_in_future: BlogPost {
                    url: "public_in_future".to_string(),
                    title: "Test (Public in future)".to_string(),
                    description: "No description".to_string(),
                    author: Author("Quiet".to_string()),
                    markdown: Some("test4".to_string()),
                    html: "test4".to_string(),
                    tags: vec![Tag("post".to_string())],
                    reading_time: Duration::minutes(1),
                    accessible: false,
                    publication_date: Some(DateTime::from_timestamp(10_000_000_000, 0).unwrap()),
                },
                accessible_public_in_future: BlogPost {
                    url: "accessible_public_in_future".to_string(),
                    title: "Test (Accessible, Public in future)".to_string(),
                    description: "No description".to_string(),
                    author: Author("Quiet".to_string()),
                    markdown: Some("test5".to_string()),
                    html: "test5".to_string(),
                    tags: vec![Tag("post".to_string())],
                    reading_time: Duration::minutes(1),
                    accessible: true,
                    publication_date: Some(DateTime::from_timestamp(10_000_000_001, 0).unwrap()),
                },
                long_post: BlogPost {
                    url: "long_post".to_string(),
                    title: "Test (Longer blog post)".to_string(),
                    description: "No description".to_string(),
                    author: Author("gpluscb".to_string()),
                    markdown: Some(include_str!("../test_fixtures/lorem.txt").to_string()),
                    html: include_str!("../test_fixtures/lorem.txt").to_string(),
                    tags: vec![
                        Tag("public".to_string()),
                        Tag("lorem-ipsum".to_string()),
                        Tag("post".to_string()),
                    ],
                    reading_time: Duration::minutes(60),
                    accessible: true,
                    publication_date: Some(DateTime::from_timestamp(1, 0).unwrap()),
                },
            }
        }

        /// In `blog_posts` insertion order.
        fn all(&self) -> [&BlogPost; 6] {
            [
                &self.public,
                &self.accessible,
                &self.not_accessible,
                &self.public_in_future,
                &self.accessible_public_in_future,
                &self.long_post,
            ]
        }
    }

    #[sqlx::test(fixtures(path = "../test_fixtures", scripts("authors", "blog_posts", "tags")))]
    pub async fn blog_post_tests(pool: PgPool) -> super::Result<()> {
        // Test if all the data looks alright

        let expected_blog_posts = ExpectedBlogPosts::new();

        for expected_blog_post in expected_blog_posts.all() {
            let url = &expected_blog_post.url;

            let actual_blog_post = super::get_blog_post(url, false, &pool)
                .await?
                .unwrap_or_else(|| panic!("blog post '{url}' not found"));

            assert_eq!(&actual_blog_post, expected_blog_post);
        }

        // Test if the function get_blog_post with accessible_only correctly filters out
        // inaccessible posts

        for expected_blog_post in expected_blog_posts.all() {
            if expected_blog_post.is_accessible_or_public() {
                assert_eq!(
                    &super::get_blog_post(&expected_blog_post.url, true, &pool)
                        .await?
                        .expect("Accessible blog post was not returned by get_blog_post"),
                    expected_blog_post,
                );
            } else {
                assert!(
                    super::get_blog_post(&expected_blog_post.url, true, &pool)
                        .await?
                        .is_none(),
                    "Blog post '{}' found even though it was not accessible",
                    expected_blog_post.url,
                )
            }
        }

        // Test get_blog_posts for all blog posts, in particular order

        let all_blog_posts = super::get_blog_posts(None, None, false, &pool).await?;
        assert!(expected_blog_posts
            .all()
            .into_iter()
            .all(|blog_post| all_blog_posts.contains(blog_post)));

        assert_eq!(all_blog_posts.len(), expected_blog_posts.all().len());
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

        let expected_public_blog_posts: Vec<_> = expected_blog_posts
            .all()
            .into_iter()
            .filter(|post| post.is_public())
            .sorted_unstable_by_key(|post| post.publication_date.unwrap())
            .rev()
            .cloned()
            .collect();

        assert_eq!(
            super::get_blog_posts(None, None, true, &pool).await?,
            expected_public_blog_posts,
        );

        Ok(())
    }

    #[sqlx::test(fixtures(path = "../test_fixtures", scripts("authors", "blog_posts", "tags")))]
    async fn tags_test(pool: PgPool) -> super::Result<()> {
        let expected_blog_posts = ExpectedBlogPosts::new();

        let mut expected_all_tags: Vec<_> = expected_blog_posts
            .all()
            .into_iter()
            .flat_map(|post| post.tags.clone())
            .unique()
            .collect();

        expected_all_tags.sort_unstable();

        assert_eq!(expected_all_tags, super::get_tags(false, &pool).await?);

        Ok(())
    }

    // TODO: Add authors tests
    // TODO: Add insert/update tests
}
