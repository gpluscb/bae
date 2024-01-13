use std::time::SystemTime;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct BlogPost {
    pub url: String,
    pub title: String,
    pub markdown: Option<String>,
    pub html: String,
    pub tags: Vec<String>,
    pub accessible: bool,
    pub date_of_publication: Option<SystemTime>,
}

impl BlogPost {
    pub fn is_public(&self) -> bool {
        self.accessible && self.date_of_publication.is_some()
    }
}
