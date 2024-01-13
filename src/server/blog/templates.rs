use crate::model::BlogPost;
use askama::Template;

#[derive(Template)]
#[template(path = "blog/home.html")]
pub struct HomeTemplate {
    pub blog_posts: Vec<BlogPost>,
}
