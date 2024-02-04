use bae_common::blog::{Author, MdOrHtml, PartialBlogPost, Tag};
use bae_common::database;
use bae_common::highlighting::Theme;
use bae_common::markdown_render::{CodeBlockHighlighter, StandardClassNameGenerator};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use comrak::nodes::{AstNode, NodeValue};
use comrak::{Arena, ExtensionOptionsBuilder, ParseOptionsBuilder, RenderOptionsBuilder};
use serde::Deserialize;
use sqlx::PgPool;
use std::fs::File;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tree_sitter::QueryError;

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
}

#[derive(Clone, Eq, PartialEq, Debug, Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[tokio::main]
async fn main() {
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
        } => generate_highlight_css(&input_theme, &output_file)
            .expect("Could not generate highlight css"),
        Command::UploadBlogPost {
            md_file,
            new_author,
        } => upload_blog_post(&md_file, new_author)
            .await
            .expect("Could not upload blog post"),
    }
}

fn generate_highlight_css(input_theme: &Path, output_file: &Path) -> std::io::Result<()> {
    let theme: Theme = serde_json::from_reader(File::open(input_theme)?)?;

    let mut output_file = File::options()
        .write(true)
        .truncate(true)
        .create(true)
        .open(output_file)?;

    theme.write_css_with_class_names(
        &mut output_file,
        &StandardClassNameGenerator::standard_generator(),
    )?;

    Ok(())
}

#[derive(Debug, Error)]
enum UploadBlogPostError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid front matter: {0}")]
    InvalidFrontMatter(serde_json::Error),
    #[error("No front matter in markdown")]
    NoFrontMatter,
    #[error("Standard config returned error: {0}")]
    BadStandardConfig(QueryError),
    #[error("Set DATABASE_URL environment variable")]
    NoDatabaseUrl,
    #[error("SQLx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("Database error: {0}")]
    Database(#[from] database::Error),
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
}

async fn upload_blog_post(md_file: &Path, new_author: bool) -> Result<(), UploadBlogPostError> {
    let markdown = std::fs::read_to_string(md_file)?;

    // TODO: very duped, server shouldn't need comrak anyway in the future
    let options = comrak::Options {
        extension: ExtensionOptionsBuilder::default()
            .front_matter_delimiter(Some("---".to_string()))
            .strikethrough(true)
            .tagfilter(true)
            .table(true)
            .autolink(true)
            .tasklist(true)
            .superscript(true)
            .footnotes(true)
            .multiline_block_quotes(true)
            .build()
            .expect("Building ExtensionOptions failed"),
        parse: ParseOptionsBuilder::default().smart(true).build().unwrap(),
        render: RenderOptionsBuilder::default()
            .unsafe_(true)
            .build()
            .expect("Building RenderOptions failed"),
    };

    let arena = Arena::new();

    let root = comrak::parse_document(&arena, &markdown, &options);

    fn take_front_matter<'a: 'b, 'b>(node: &'a AstNode<'b>) -> Option<String> {
        if let NodeValue::FrontMatter(front_matter) = &mut node.data.borrow_mut().value {
            Some(std::mem::take(front_matter))
        } else {
            node.children().find_map(take_front_matter)
        }
    }

    let front_matter_str = take_front_matter(root).ok_or(UploadBlogPostError::NoFrontMatter)?;

    let front_matter: FrontMatter =
        serde_json::from_str(&front_matter_str).map_err(UploadBlogPostError::InvalidFrontMatter)?;

    let partial_post = PartialBlogPost {
        url: front_matter.url,
        title: front_matter.title,
        description: front_matter.description,
        author: front_matter.author,
        contents: MdOrHtml::Markdown(markdown),
        tags: front_matter.tags,
        accessible: front_matter.accessible,
        publication_date: front_matter.publication_date,
    };

    let full_post = partial_post.generate_blog_post(
        &options,
        &CodeBlockHighlighter::standard_config().map_err(UploadBlogPostError::BadStandardConfig)?,
    );

    let database_url =
        std::env::var("DATABASE_URL").map_err(|_| UploadBlogPostError::NoDatabaseUrl)?;
    let database = PgPool::connect(&database_url)
        .await
        .expect("Could not connect to database");

    let mut transaction = database.begin().await?;
    database::insert_blog_post(full_post, new_author, &mut transaction)
        .await
        .map_err(UploadBlogPostError::from)
}
