use crate::markdown_render::{render_md_to_html, CodeBlockHighlighter};
use crate::server::blog::{BlogPostPath, TaggedPath};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use std::fmt::{Display, Formatter};

/// Probably slightly low-ball estimate but that's fine, it's a technical blog.
const AVERAGE_READING_WPM: usize = 200;

#[derive(Clone, Eq, PartialEq, Debug, Deserialize)]
pub enum MdOrHtml {
    Markdown(String),
    Html(String),
}

#[derive(Clone, Eq, PartialEq, Debug, Deserialize)]
pub struct PartialBlogPost {
    pub url: String,
    pub title: String,
    pub description: String,
    pub author: Author,
    pub contents: MdOrHtml,
    pub tags: Vec<Tag>,
    pub accessible: bool,
    pub publication_date: Option<DateTime<Utc>>,
}

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

fn generate_reading_time(content: &str) -> Duration {
    Duration::minutes((content.split_whitespace().count() / AVERAGE_READING_WPM) as i64)
}

impl MdOrHtml {
    pub fn contents(&self) -> &str {
        match self {
            MdOrHtml::Markdown(contents) => contents,
            MdOrHtml::Html(contents) => contents,
        }
    }

    pub fn markdown(&self) -> Option<&str> {
        if let MdOrHtml::Markdown(md) = self {
            Some(md)
        } else {
            None
        }
    }

    pub fn html(&self) -> Option<&str> {
        if let MdOrHtml::Html(html) = self {
            Some(html)
        } else {
            None
        }
    }
}

impl PartialBlogPost {
    pub fn generate_blog_post(self, highlighter: &CodeBlockHighlighter) -> BlogPost {
        let PartialBlogPost {
            url,
            title,
            description,
            author,
            contents,
            tags,
            accessible,
            publication_date,
        } = self;

        let reading_time = generate_reading_time(contents.contents());
        let (markdown, html) = match contents {
            MdOrHtml::Markdown(md) => {
                let html = render_md_to_html(&md, highlighter);
                (Some(md), html)
            }
            MdOrHtml::Html(html) => (None, html),
        };

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
        }
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
