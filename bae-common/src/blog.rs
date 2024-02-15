use crate::database::{Author, Tag};
use chrono::{DateTime, Duration, Utc};
use std::fmt::{Display, Formatter};

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

impl BlogPost {
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

impl Display for Author {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
