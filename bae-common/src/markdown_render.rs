use crate::highlighting::{
    write_html_highlight_end, write_html_highlight_start, CssClassNameGenerator, HIGHLIGHT_NAMES,
};
use pulldown_cmark::{
    CodeBlockKind, CowStr, Event, MetadataBlockKind, Options, Parser, Tag, TagEnd,
};
use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Write;
use thiserror::Error;
use tracing::error;
use tree_sitter::QueryError;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct StandardClassNameGenerator {
    pub class_prefix: String,
}

impl CssClassNameGenerator for StandardClassNameGenerator {
    fn class_for_highlight(&self, highlight_name: &str, _highlight_idx: usize) -> Option<Cow<str>> {
        let mut output = self.class_prefix.clone();
        if !output.is_empty() {
            output.push('-');
        }
        output.push_str(&highlight_name.replace('.', "-"));
        Some(Cow::Owned(output))
    }
}

impl StandardClassNameGenerator {
    pub fn standard_generator() -> Self {
        StandardClassNameGenerator {
            class_prefix: "highlight".to_string(),
        }
    }
}

#[derive()]
pub struct Language {
    pub canonical_name: &'static str,
    pub config: HighlightConfiguration,
}

#[derive()]
pub struct CodeBlockHighlighter<G> {
    pub languages: HashMap<&'static str, Language>,
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
            Ok(Language {
                canonical_name: "Rust",
                config: rust,
            })
        };
        let js = || {
            let mut js = HighlightConfiguration::new(
                tree_sitter_javascript::language(),
                tree_sitter_javascript::HIGHLIGHT_QUERY,
                tree_sitter_javascript::INJECTION_QUERY,
                tree_sitter_javascript::LOCALS_QUERY,
            )?;
            js.configure(&HIGHLIGHT_NAMES);
            Ok(Language {
                canonical_name: "JavaScript",
                config: js,
            })
        };
        let cpp = || {
            let mut cpp = HighlightConfiguration::new(
                tree_sitter_cpp::language(),
                tree_sitter_cpp::HIGHLIGHT_QUERY,
                "",
                "",
            )?;
            cpp.configure(&HIGHLIGHT_NAMES);
            Ok(Language {
                canonical_name: "C++",
                config: cpp,
            })
        };

        let mut languages = HashMap::new();
        languages.insert("rust", rust()?);
        languages.insert("rs", rust()?);
        languages.insert("javascript", js()?);
        languages.insert("js", js()?);
        languages.insert("c++", cpp()?);
        languages.insert("cpp", cpp()?);

        Ok(CodeBlockHighlighter {
            languages,
            class_name_generator: StandardClassNameGenerator::standard_generator(),
        })
    }
}

#[derive(Debug, Error)]
pub enum HighlighterError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Unrecognised language {0}")]
    UnknownLanguage(String),
    #[error("tree-sitter-highlight error: {0}")]
    Highlight(#[from] tree_sitter_highlight::Error),
}

impl<G: CssClassNameGenerator> CodeBlockHighlighter<G> {
    pub fn write_code_block_open_html<W: Write>(
        &self,
        output: &mut W,
        lang: Option<&str>,
    ) -> Result<(), HighlighterError> {
        let canonical_name = if let Some(lang) = lang {
            self.languages
                .get(lang)
                .ok_or_else(|| HighlighterError::UnknownLanguage(lang.to_string()))?
                .canonical_name
        } else {
            "Plain Text"
        };

        write!(
            output,
            r#"<div class="code-block-div"><p class="language-display">{canonical_name}</p><pre class="code-block"><code class="code-block-code">"#,
        )
        .map_err(HighlighterError::from)
    }

    pub fn write_highlighted_code_html<W: Write>(
        &self,
        output: &mut W,
        lang: Option<&str>,
        code: &str,
    ) -> Result<(), HighlighterError> {
        let Some(lang) = lang else {
            output.write_all(code.as_bytes())?;
            return Ok(());
        };

        let language = self
            .languages
            .get(lang)
            .ok_or_else(|| HighlighterError::UnknownLanguage(lang.to_string()))?;

        let mut highlighter = Highlighter::new();
        // Collect early so we can fall back to full plain text in case of error
        // instead of having highlighted bits already in the output
        let highlights: Vec<_> = highlighter
            .highlight(&language.config, code.as_bytes(), None, |_| None)?
            .collect::<Result<_, _>>()?;

        for highlight in highlights {
            match highlight {
                HighlightEvent::Source { start, end } => {
                    output.write_all(code[start..end].as_bytes())?;
                }
                HighlightEvent::HighlightStart(highlight) => {
                    write_html_highlight_start(
                        output,
                        highlight,
                        "span",
                        &HashMap::new(),
                        &self.class_name_generator,
                    )?;
                }
                HighlightEvent::HighlightEnd => {
                    write_html_highlight_end(output, "span")?;
                }
            }
        }

        Ok(())
    }

    pub fn write_code_block_close_html<W: Write>(output: &mut W) -> std::io::Result<()> {
        output.write_all(
            br#"</code></pre></div>
        "#,
        )
    }

    pub fn write_code_block<W: Write>(
        &self,
        output: &mut W,
        lang: Option<&str>,
        code: &str,
    ) -> Result<(), HighlighterError> {
        self.write_code_block_open_html(output, lang)?;
        self.write_highlighted_code_html(output, lang, code)?;
        Self::write_code_block_close_html(output).map_err(HighlighterError::from)
    }
}

fn custom_render_code_blocks<
    'e: 'h,
    'h,
    G: CssClassNameGenerator,
    I: Iterator<Item = Event<'e>> + 'h,
>(
    iter: I,
    highlighter: &'h CodeBlockHighlighter<G>,
) -> impl Iterator<Item = Result<Event<'e>, HighlighterError>> + 'h {
    struct CodeBlock<'a> {
        lang: CowStr<'a>,
        code: String,
    }

    let mut current_code_block = None;

    iter.map(move |event| {
        Ok(match (event, &mut current_code_block) {
            (Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))), None) => {
                current_code_block = Some(CodeBlock {
                    lang,
                    code: String::new(),
                });
                None
            }
            (Event::Text(text), Some(CodeBlock { code, .. })) => {
                code.push_str(&text);
                None
            }
            (Event::End(TagEnd::CodeBlock), Some(_)) => {
                let CodeBlock { lang, code } = current_code_block.take().unwrap();

                let mut html = Vec::new();
                highlighter.write_code_block(
                    &mut html,
                    (!lang.is_empty()).then(|| &*lang),
                    &code,
                )?;

                Some(Event::Html(
                    String::from_utf8(html)
                        .expect("Generated invalid utf8 highlighting")
                        .into(),
                ))
            }
            (event, _) => Some(event),
        })
    })
    .filter_map(Result::transpose)
}

struct Metadata(Option<String>);

impl<'a, 'e> FromIterator<&'a Event<'e>> for Metadata {
    fn from_iter<T: IntoIterator<Item = &'a Event<'e>>>(iter: T) -> Self {
        let mut current_metadata_block = None;

        for event in iter {
            match (event, &mut current_metadata_block) {
                (Event::Start(Tag::MetadataBlock(MetadataBlockKind::PlusesStyle)), None) => {
                    current_metadata_block = Some(String::new());
                }
                (Event::End(TagEnd::MetadataBlock(MetadataBlockKind::PlusesStyle)), Some(_)) => {
                    return Metadata(current_metadata_block);
                }
                (Event::Text(text), Some(current_metadata)) => {
                    current_metadata.push_str(text);
                }
                _ => (),
            }
        }

        Metadata(None)
    }
}

pub struct RenderResult {
    pub metadata: Option<String>,
    pub html: String,
}

pub fn render_md_to_html<G: CssClassNameGenerator>(
    markdown: &str,
    options: Options,
    highlighter: &CodeBlockHighlighter<G>,
) -> Result<RenderResult, HighlighterError> {
    let parser = Parser::new_ext(markdown, options);

    let events: Vec<_> =
        custom_render_code_blocks(parser, highlighter).collect::<Result<_, _>>()?;

    let Metadata(metadata) = events.iter().collect();

    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, events.into_iter());

    Ok(RenderResult { metadata, html })
}

// TODO: Tests
