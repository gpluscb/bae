-- Add migration script here
create table if not exists public.blog_post
(
    url              text    not null
        constraint blog_post_pk
            primary key,
    title            text    not null,
    markdown         text,
    html             text    not null,
    accessible       boolean not null,
    publication_date timestamp
);
