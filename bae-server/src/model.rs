use crate::server::blog::{BlogPostPath, TaggedPath};
use axum_extra::routing::TypedPath;
use bae_common::blog::{BlogPost, Tag};

pub trait ServerPathExt {
    type Path: TypedPath;

    fn full_path(&self) -> Self::Path;
}

impl ServerPathExt for BlogPost {
    type Path = BlogPostPath;

    fn full_path(&self) -> Self::Path {
        BlogPostPath {
            post_url: self.url.clone(),
        }
    }
}

impl ServerPathExt for Tag {
    type Path = TaggedPath;

    fn full_path(&self) -> Self::Path {
        TaggedPath { tag: self.clone() }
    }
}
