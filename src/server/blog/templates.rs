use crate::model::{BlogPost, Tag};
use askama::Template;

#[derive(Template)]
#[template(path = "blog/home.html")]
pub struct HomeTemplate {
    pub blog_posts: Vec<BlogPost>,
}

#[derive(Template)]
#[template(path = "blog/blog_post.html")]
pub struct BlogPostTemplate {
    pub blog_post: BlogPost,
}

#[derive(Template)]
#[template(path = "blog/tagged.html")]
pub struct TaggedTemplate {
    pub tag: Tag,
    pub blog_posts: Vec<BlogPost>,
}
