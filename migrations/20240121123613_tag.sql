create table if not exists tag
(
    tag       text not null,
    blog_post text not null
        constraint blog_post_fk
            references blog_post,
    constraint tag_pk
        primary key (tag, blog_post)
);
