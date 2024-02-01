use crate::highlighting::{
    write_html_highlight_end, write_html_highlight_start, CssClassNameGenerator, HIGHLIGHT_NAMES,
};
use comrak::adapters::SyntaxHighlighterAdapter;
use comrak::{markdown_to_html_with_plugins, Options, PluginsBuilder, RenderPluginsBuilder};
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Write;
use thiserror::Error;
use tracing::{error, warn};
use tree_sitter::QueryError;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Default, Debug)]
pub struct StandardClassNameGenerator;

impl CssClassNameGenerator for StandardClassNameGenerator {
    fn class_for_highlight(&self, highlight_name: &str, _highlight_idx: usize) -> Option<Cow<str>> {
        Some(Cow::Owned(format!("highlight.{highlight_name}")))
    }
}

#[derive()]
pub struct CodeBlockHighlighter<G> {
    pub languages: HashMap<&'static str, HighlightConfiguration>,
    pub class_name_generator: G,
}

impl CodeBlockHighlighter<StandardClassNameGenerator> {
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

        Ok(CodeBlockHighlighter {
            languages,
            class_name_generator: StandardClassNameGenerator,
        })
    }
}

impl<G: CssClassNameGenerator + Send + Sync> SyntaxHighlighterAdapter for CodeBlockHighlighter<G> {
    fn write_highlighted(
        &self,
        out: &mut dyn Write,
        lang: Option<&str>,
        code: &str,
    ) -> std::io::Result<()> {
        let Some(lang) = lang else {
            out.write_all(code.as_bytes())?;
            return Ok(());
        };

        let Some(config) = self.languages.get(lang) else {
            warn!(lang, "Unrecognised language, falling back to plain text");
            out.write_all(code.as_bytes())?;
            return Ok(());
        };

        #[derive(Debug, Error)]
        enum TryHighlightError {
            #[error("IO error: {0}")]
            Io(#[from] std::io::Error),
            #[error("tree-sitter-highlight error: {0}")]
            Highlight(#[from] tree_sitter_highlight::Error),
        }

        fn try_highlight<G: CssClassNameGenerator>(
            config: &HighlightConfiguration,
            class_name_generator: &G,
            out: &mut dyn Write,
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
                        out.write_all(code[start..end].as_bytes())?;
                    }
                    HighlightEvent::HighlightStart(highlight) => {
                        write_html_highlight_start(
                            out,
                            highlight,
                            "span",
                            &HashMap::new(),
                            class_name_generator,
                        )?;
                    }
                    HighlightEvent::HighlightEnd => {
                        write_html_highlight_end(out, "span")?;
                    }
                }
            }

            Ok(())
        }

        match try_highlight(config, &self.class_name_generator, out, code) {
            Ok(()) => Ok(()),
            Err(TryHighlightError::Io(err)) => Err(err),
            Err(TryHighlightError::Highlight(err)) => {
                error!(%err, "Error trying to highlight, falling back to plain text");
                out.write_all(code.as_bytes())
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

pub fn render_md_to_html<G: CssClassNameGenerator + Send + Sync>(
    markdown: &str,
    options: &Options,
    highlighter: &CodeBlockHighlighter<G>,
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
