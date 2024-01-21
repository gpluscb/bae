use std::time::SystemTime;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct BlogPost {
    pub url: String,
    pub title: String,
    pub markdown: Option<String>,
    pub html: String,
    pub tags: Vec<String>,
    pub accessible: bool,
    pub publication_date: Option<SystemTime>,
}

impl BlogPost {
    pub fn is_public(&self) -> bool {
        self.accessible && self.publication_date.is_some()
    }
}
