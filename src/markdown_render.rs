use comrak::adapters::SyntaxHighlighterAdapter;
use comrak::{markdown_to_html_with_plugins, Options, PluginsBuilder, RenderPluginsBuilder};
use std::collections::HashMap;
use std::io::Write;
use thiserror::Error;
use tracing::{error, warn};
use tree_sitter::QueryError;
use tree_sitter_highlight::{Highlight, HighlightConfiguration, HighlightEvent, Highlighter};

pub const HIGHLIGHT_NAMES: [&str; 29] = [
    "attribute",
    "carriage-return",
    "comment",
    "constant",
    "constant.builtin",
    "constructor",
    "constructor.builtin",
    "embedded",
    "escape",
    "function",
    "function.builtin",
    "keyword",
    "number",
    "module",
    "operator",
    "property",
    "property.builtin",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "punctuation.special",
    "string",
    "string.special",
    "tag",
    "type",
    "type.builtin",
    "variable",
    "variable.builtin",
    "variable.parameter",
];

#[derive()]
pub struct CodeBlockHighlighter {
    pub languages: HashMap<&'static str, HighlightConfiguration>,
}

impl CodeBlockHighlighter {
    pub fn standard_config() -> Result<Self, QueryError> {
        let rust = || {
            let mut rust = HighlightConfiguration::new(
                tree_sitter_rust::language(),
                tree_sitter_rust::HIGHLIGHT_QUERY,
                tree_sitter_rust::INJECTIONS_QUERY,
                "",
            )?;
            rust.configure(&HIGHLIGHT_NAMES);
            Ok(rust)
        };

        let mut languages = HashMap::new();
        languages.insert("rust", rust()?);
        languages.insert("rs", rust()?);

        Ok(CodeBlockHighlighter { languages })
    }
}

impl SyntaxHighlighterAdapter for CodeBlockHighlighter {
    fn write_highlighted(
        &self,
        output: &mut dyn Write,
        lang: Option<&str>,
        code: &str,
    ) -> std::io::Result<()> {
        let Some(lang) = lang else {
            output.write_all(code.as_bytes())?;
            return Ok(());
        };

        let Some(config) = self.languages.get(lang) else {
            warn!(lang, "Unrecognised language, falling back to plain text");
            output.write_all(code.as_bytes())?;
            return Ok(());
        };

        #[derive(Debug, Error)]
        enum TryHighlightError {
            #[error("IO error: {0}")]
            Io(#[from] std::io::Error),
            #[error("tree-sitter-highlight error: {0}")]
            Highlight(#[from] tree_sitter_highlight::Error),
        }

        fn try_highlight(
            config: &HighlightConfiguration,
            output: &mut dyn Write,
            code: &str,
        ) -> Result<(), TryHighlightError> {
            let mut highlighter = Highlighter::new();
            // Collect early so we can fall back to full plain text in case of error
            // instead of having highlighted bits already in the output
            let highlights: Vec<_> = highlighter
                .highlight(config, code.as_bytes(), None, |_| None)?
                .collect::<Result<_, _>>()?;

            for highlight in highlights {
                match highlight {
                    HighlightEvent::Source { start, end } => {
                        output.write_all(code[start..end].as_bytes())?;
                    }
                    HighlightEvent::HighlightStart(Highlight(i)) => {
                        write!(output, r#"<span class="highlight.{}">"#, HIGHLIGHT_NAMES[i])?;
                    }
                    HighlightEvent::HighlightEnd => {
                        output.write_all(b"</span>")?;
                    }
                }
            }

            Ok(())
        }

        match try_highlight(config, output, code) {
            Ok(()) => Ok(()),
            Err(TryHighlightError::Io(err)) => Err(err),
            Err(TryHighlightError::Highlight(err)) => {
                error!(%err, "Error trying to highlight, falling back to plain text");
                output.write_all(code.as_bytes())
            }
        }
    }

    fn write_pre_tag(
        &self,
        output: &mut dyn Write,
        attributes: HashMap<String, String>,
    ) -> std::io::Result<()> {
        write!(output, r#"<pre class="code-block""#)?;
        for (attr, value) in attributes {
            write!(output, r#" {attr}="{value}""#)?;
        }
        write!(output, ">")
    }

    fn write_code_tag(
        &self,
        output: &mut dyn Write,
        attributes: HashMap<String, String>,
    ) -> std::io::Result<()> {
        write!(output, "<code")?;
        for (attr, value) in attributes {
            write!(output, r#" {attr}="{value}""#)?;
        }
        write!(output, ">")
    }
}

pub fn render_md_to_html(
    markdown: &str,
    options: &Options,
    highlighter: &CodeBlockHighlighter,
) -> String {
    // TODO: eliminate unwraps
    let plugins = PluginsBuilder::default()
        .render(
            RenderPluginsBuilder::default()
                .codefence_syntax_highlighter(Some(highlighter))
                .build()
                .unwrap(),
        )
        .build()
        .unwrap();

    markdown_to_html_with_plugins(markdown, options, &plugins)
}

// TODO: Tests
