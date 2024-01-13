use crate::model::Blog;
use askama::Template;

#[derive(Template)]
#[template(path = "blog/home.html")]
pub struct HomeTemplate {
    pub blogs: Vec<Blog>,
}
