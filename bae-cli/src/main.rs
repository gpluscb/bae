use bae_common::highlighting::Theme;
use bae_common::markdown_render::StandardClassNameGenerator;
use clap::{Parser, Subcommand};
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
}

#[derive(Clone, Eq, PartialEq, Debug, Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

fn main() {
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
