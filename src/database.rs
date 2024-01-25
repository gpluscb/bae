use crate::model::{BlogPost, Tag};
use futures::{StreamExt, TryStreamExt};
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
         WHERE url=$1 AND (accessible OR \
            (publication_date IS NOT NULL \
            AND publication_date <= now() at time zone('utc'))) \
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
        "SELECT url, title, markdown, html, accessible, publication_date, array_remove(array_agg(tag), NULL) as tags \
        FROM blog_post NATURAL LEFT JOIN tag \
        GROUP BY url \
        ORDER BY publication_date DESC"
    )
    .fetch(executor)
    .map_err(Error::from)
    .map(|result| result.and_then(|record| record.try_into()))
    .try_collect()
    .await
}

pub async fn get_public_blog_posts<'c, E: PgExecutor<'c>>(executor: E) -> Result<Vec<BlogPost>> {
    query_as!(
        BlogPostRecord,
        "SELECT url, title, markdown, html, accessible, publication_date, array_remove(array_agg(tag), NULL) as tags \
        FROM blog_post NATURAL LEFT JOIN tag \
        WHERE publication_date IS NOT NULL \
            AND publication_date <= now() at time zone('utc') \
        GROUP BY url \
        ORDER BY publication_date DESC"
    )
    .fetch(executor)
    .map_err(Error::from)
    .map(|result| result.and_then(|record| record.try_into()))
    .try_collect()
    .await
}

pub async fn get_public_blog_posts_for_tag<'c, E: PgExecutor<'c>>(
    tag: &Tag,
    executor: E,
) -> Result<Vec<BlogPost>> {
    query_as!(
        BlogPostRecord,
        "SELECT url, title, markdown, html, accessible, publication_date, array_agg(tag) as tags \
        FROM blog_post NATURAL JOIN tag \
        WHERE publication_date IS NOT NULL \
            AND publication_date <= now() at time zone('utc') \
            AND tag=$1 \
        GROUP BY url \
        ORDER BY publication_date DESC",
        tag.0,
    )
    .fetch(executor)
    .map_err(Error::from)
    .map(|result| result.and_then(|record| record.try_into()))
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

#[cfg(test)]
mod tests {
    use crate::model::BlogPost;
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

    #[sqlx::test(fixtures(path = "../test_fixtures", scripts("blog_posts")))]
    pub async fn blog_post_tests(pool: PgPool) -> super::Result<()> {
        // Test if all the data looks alright

        let public_post = super::get_blog_post("public", &pool)
            .await?
            .expect("blog post 'public' not found");

        let expected_public_post = BlogPost {
            url: "public".to_string(),
            title: "Test (Public)".to_string(),
            markdown: Some("test *bold*".to_string()),
            html: "test <b>bold</b>".to_string(),
            tags: vec![],
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
            markdown: Some("test2".to_string()),
            html: "test2".to_string(),
            tags: vec![],
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
            markdown: Some("test3".to_string()),
            html: "test3".to_string(),
            tags: vec![],
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
            markdown: Some("test4".to_string()),
            html: "test4".to_string(),
            tags: vec![],
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
            markdown: Some("test5".to_string()),
            html: "test5".to_string(),
            tags: vec![],
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
            markdown: long_post.markdown.clone(),
            html: long_post.html.clone(),
            tags: vec![],
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

        // Test get_all_blog_posts, in particular order

        let all_blog_posts = super::get_all_blog_posts(&pool).await?;
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

        // Test get_public_blog_posts, in particular order

        let public_blog_posts = super::get_public_blog_posts(&pool).await?;
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
}
