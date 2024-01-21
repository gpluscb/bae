use crate::server::blog::{BlogPostPath, TagPath};
use serde::Deserialize;
use std::fmt::{Display, Formatter};
use std::time::SystemTime;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct BlogPost {
    pub url: String,
    pub title: String,
    pub markdown: Option<String>,
    pub html: String,
    pub tags: Vec<Tag>,
    pub accessible: bool,
    pub publication_date: Option<SystemTime>,
}

#[derive(Clone, Eq, PartialEq, Debug, Deserialize)]
pub struct Tag(pub String);

impl Display for Tag {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Tag {
    pub fn full_path(&self) -> TagPath {
        TagPath { tag: self.clone() }
    }
}

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
                .map_or(false, |date| date <= SystemTime::now())
    }
}
