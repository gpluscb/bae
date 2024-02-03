alter table blog_post
    rename column reading_time to reading_time_minutes;

alter table blog_post
    alter column reading_time_minutes drop default,
    alter column reading_time_minutes type integer using extract(minutes from reading_time_minutes);
