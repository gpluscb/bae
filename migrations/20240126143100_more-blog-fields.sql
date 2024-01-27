create table author
(
    author text not null
        constraint author_pk
            primary key
);

alter table blog_post
    add description text default 'No description' not null;

alter table blog_post
    add author text;

alter table blog_post
    add reading_time interval default '1 minute' not null;

alter table blog_post
    add constraint blog_post_author_author_fk
        foreign key (author) references author;
