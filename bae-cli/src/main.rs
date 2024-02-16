mod cli_io;
mod diff;

use bae_common::blog::BlogPost;
use bae_common::database;
use bae_common::database::{Author, Tag};
use bae_common::highlighting::Theme;
use bae_common::markdown_render::{
    render_md_to_html, CodeBlockHighlighter, RenderResult, StandardClassNameGenerator,
};
use chrono::{DateTime, Duration, Utc};
use clap::{Parser, Subcommand};
use color_eyre::eyre::{eyre, OptionExt, WrapErr};
use serde::Deserialize;
use sqlx::PgPool;
use std::fs::File;
use std::path::{Path, PathBuf};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Probably slightly low-ball estimate but that's fine, it's a technical blog.
const AVERAGE_READING_WPM: usize = 200;

#[derive(Clone, Eq, PartialEq, Debug, Subcommand)]
enum Command {
    GenerateHighlightCss {
        #[arg(short, long)]
        input_theme: PathBuf,
        #[arg(short, long)]
        output_file: PathBuf,
    },
    UploadBlogPost {
        #[arg(short, long)]
        md_file: PathBuf,
        #[arg(long)]
        new_author: bool,
    },
    UpdateBlogPost {
        #[arg(short, long)]
        md_file: PathBuf,
        #[arg(short, long)]
        original_url: Option<String>,
        #[arg(long)]
        new_author: bool,
    },
}

#[derive(Clone, Eq, PartialEq, Debug, Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    _ = dotenv::dotenv();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "bae_cli=debug,bae_common=debug,tower_http=debug,axum::rejection=trace,sqlx=debug".into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    match args.command {
        Command::GenerateHighlightCss {
            input_theme,
            output_file,
        } => generate_highlight_css(&input_theme, &output_file),
        Command::UploadBlogPost {
            md_file,
            new_author,
        } => upload_blog_post(&md_file, new_author).await,
        Command::UpdateBlogPost {
            md_file,
            original_url,
            new_author,
        } => update_blog_post(&md_file, original_url.as_deref(), new_author).await,
    }
}

fn generate_highlight_css(input_theme: &Path, output_file: &Path) -> color_eyre::Result<()> {
    let theme: Theme =
        serde_json::from_reader(File::open(input_theme).wrap_err("Opening input file failed")?)
            .wrap_err("Deserializing json to theme failed")?;

    let mut output_file = File::options()
        .write(true)
        .truncate(true)
        .create(true)
        .open(output_file)
        .wrap_err("Opening output file failed")?;

    theme
        .write_css_with_class_names(
            &mut output_file,
            &StandardClassNameGenerator::standard_generator(),
        )
        .wrap_err("Writing css failed")?;

    Ok(())
}

#[derive(Clone, Eq, PartialEq, Debug, Deserialize)]
struct FrontMatter {
    pub url: String,
    pub title: String,
    pub description: String,
    pub author: Author,
    pub tags: Vec<Tag>,
    pub accessible: bool,
    pub publication_date: Option<DateTime<Utc>>,
    #[serde(default)]
    pub reading_time_minutes: Option<u32>,
}

fn md_options() -> pulldown_cmark::Options {
    use pulldown_cmark::Options as Opt;

    Opt::ENABLE_TABLES
        | Opt::ENABLE_FOOTNOTES
        | Opt::ENABLE_STRIKETHROUGH
        | Opt::ENABLE_TASKLISTS
        | Opt::ENABLE_SMART_PUNCTUATION
        | Opt::ENABLE_HEADING_ATTRIBUTES
        | Opt::ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS
}

fn reading_time(contents: &str) -> Duration {
    Duration::minutes((contents.split_whitespace().count() / AVERAGE_READING_WPM) as i64)
}

fn full_blog_post_from_md(markdown: String) -> color_eyre::Result<BlogPost> {
    let options = md_options();

    let RenderResult { metadata, html } = render_md_to_html(
        &markdown,
        options,
        &CodeBlockHighlighter::standard_config()
            .wrap_err("Getting standard CodeBlockHighlighter config failed")?,
    )
    .wrap_err("Rendering markdown failed")?;

    let metadata =
        metadata.ok_or_eyre("Blog post did not have correct pluses delimited metadata")?;

    let FrontMatter {
        url,
        title,
        description,
        author,
        tags,
        accessible,
        publication_date,
        reading_time_minutes,
    } = serde_json::from_str(&metadata).wrap_err("Front matter could not be parsed")?;

    let reading_time = reading_time_minutes
        .map(|minutes| Duration::minutes(minutes as i64))
        .unwrap_or_else(|| reading_time(&markdown));

    Ok(BlogPost {
        url,
        title,
        description,
        author,
        markdown: Some(markdown),
        html,
        tags,
        reading_time,
        accessible,
        publication_date,
    })
}

async fn connect_database() -> color_eyre::Result<PgPool> {
    let database_url = std::env::var("DATABASE_URL").wrap_err("DATABASE_URL env var error")?;

    PgPool::connect(&database_url)
        .await
        .wrap_err("Could not connect to database")
}

async fn upload_blog_post(md_file: &Path, new_author: bool) -> color_eyre::Result<()> {
    let markdown = std::fs::read_to_string(md_file)?;

    let full_post = full_blog_post_from_md(markdown)?;

    if full_post.publication_date.is_some()
        && !cli_io::prompt(
            "You are attempting to upload a blog post that has a publishing date. \
            Usually you might want to look at how it renders before deciding a publication date. \
            Continue?",
        )?
    {
        return Err(eyre!("User aborted"));
    }

    let database = connect_database().await?;

    let mut transaction = database.begin().await?;
    database::insert_blog_post(&full_post, new_author, &mut transaction)
        .await
        .wrap_err("Inserting blog post failed before transaction commit")?;

    transaction
        .commit()
        .await
        .wrap_err("Inserting blog post failed")
}

async fn update_blog_post(
    md_file: &Path,
    original_url: Option<&str>,
    new_author: bool,
) -> color_eyre::Result<()> {
    let markdown = std::fs::read_to_string(md_file)?;

    let full_post = full_blog_post_from_md(markdown)?;

    let database = connect_database().await?;

    let old_full_post =
        database::get_blog_post(original_url.unwrap_or(&full_post.url), false, &database)
            .await?
            .ok_or_eyre("Post with original url not found")?;

    if let Some(old_full_post_md) = old_full_post.markdown {
        println!("Markdown Diff:");
        println!();
        diff::print_diff(&old_full_post_md, full_post.markdown.as_ref().unwrap());
        println!();
    }

    println!("HTML Diff:");
    println!();
    diff::print_diff(&old_full_post.html, &full_post.html);
    println!();

    if !cli_io::prompt("Continue with update?").wrap_err("Prompting user failed")? {
        return Err(eyre!("User aborted"));
    }

    let mut transaction = database.begin().await?;
    database::update_blog_post(original_url, &full_post, new_author, &mut transaction).await?;

    transaction
        .commit()
        .await
        .wrap_err("Updating blog post failed")
}
