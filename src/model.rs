use crate::server::blog::{BlogPostPath, TaggedPath};
use serde::Deserialize;
use sqlx::types::chrono::{DateTime, Utc};
use std::fmt::{Display, Formatter};
use std::time::Duration;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct BlogPost {
    pub url: String,
    pub title: String,
    pub description: String,
    pub author: Author,
    pub markdown: Option<String>,
    pub html: String,
    pub tags: Vec<Tag>,
    pub reading_time: Duration,
    pub accessible: bool,
    pub publication_date: Option<DateTime<Utc>>,
}

#[derive(Clone, Eq, PartialEq, Debug, Deserialize)]
pub struct Author(pub String);

#[derive(Clone, Eq, PartialEq, Debug, Deserialize)]
pub struct Tag(pub String);

impl BlogPost {
    pub fn full_path(&self) -> BlogPostPath {
        BlogPostPath {
            post_url: self.url.clone(),
        }
    }

    pub fn is_public(&self) -> bool {
        self.accessible
            && self
                .publication_date
                .map_or(false, |date| date <= Utc::now())
    }
}

impl Display for Tag {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Tag {
    pub fn full_path(&self) -> TaggedPath {
        TaggedPath { tag: self.clone() }
    }
}

impl Display for Author {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
