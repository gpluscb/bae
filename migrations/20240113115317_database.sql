create table if not exists blog
(
    url                 TEXT    not null
        constraint blog_pk
            primary key,
    date_of_publication INTEGER,
    title               TEXT    not null,
    contents            TEXT    not null,
    markdown            TEXT,
    html                TEXT    not null,
    tags                TEXT    not null,
    accessible          INTEGER not null
);