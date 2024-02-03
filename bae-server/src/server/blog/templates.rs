use crate::model::ServerPathExt;
use askama::Template;
use bae_common::blog::{BlogPost, Tag};

#[derive(Template)]
#[template(path = "blog/test.html")]
pub struct TestTemplate {
    pub test_md_rendered: String,
}

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

#[derive(Template)]
#[template(path = "blog/tags.html")]
pub struct TagsTemplate {
    pub tags: Vec<Tag>,
}
