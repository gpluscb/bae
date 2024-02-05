use bae_common::blog::{Author, MdOrHtml, PartialBlogPost, Tag};
use bae_common::database;
use bae_common::highlighting::Theme;
use bae_common::markdown_render::{CodeBlockHighlighter, StandardClassNameGenerator};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use color_eyre::eyre::{OptionExt, WrapErr};
use comrak::nodes::{AstNode, NodeValue};
use comrak::{Arena, ExtensionOptionsBuilder, ParseOptionsBuilder, RenderOptionsBuilder};
use serde::Deserialize;
use sqlx::PgPool;
use std::fs::File;
use std::path::{Path, PathBuf};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

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
}

async fn upload_blog_post(md_file: &Path, new_author: bool) -> color_eyre::Result<()> {
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
            .wrap_err("Building ExtensionOptions failed")?,
        parse: ParseOptionsBuilder::default()
            .smart(true)
            .build()
            .wrap_err("Building ParseOptions failed")?,
        render: RenderOptionsBuilder::default()
            .unsafe_(true)
            .build()
            .wrap_err("Building RenderOptions failed")?,
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

    let front_matter_str = take_front_matter(root).ok_or_eyre("No front matter in markdown")?;

    let front_matter: FrontMatter = serde_json::from_str(&front_matter_str)
        .wrap_err("Front matter was not the correct json format")?;

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
        &CodeBlockHighlighter::standard_config()
            .wrap_err("Getting standard CodeBlockHighlighter config failed")?,
    );

    let database_url = std::env::var("DATABASE_URL").wrap_err("DATABASE_URL env var error")?;
    let database = PgPool::connect(&database_url)
        .await
        .wrap_err("Could not connect to database")?;

    let mut transaction = database.begin().await?;
    database::insert_blog_post(full_post, new_author, &mut transaction)
        .await
        .wrap_err("Inserting blog post failed before transaction commit")?;

    transaction
        .commit()
        .await
        .wrap_err("Inserting blog post failed")
}
