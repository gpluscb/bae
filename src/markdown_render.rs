use comrak::adapters::SyntaxHighlighterAdapter;
use comrak::{
    markdown_to_html_with_plugins, ExtensionOptionsBuilder, Options, ParseOptionsBuilder,
    PluginsBuilder, RenderOptionsBuilder, RenderPluginsBuilder,
};
use std::collections::HashMap;
use std::io::Write;
use tracing::warn;
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

// So the reason we don't use anything pre-made to roll our html is because we can't.
// If we used ClassedHtmlGenerator, that's not Send, so it's impossible to use with SyntaxHighlighterAdapter.
// Everything else in syntect's html module that doesn't output classes but coloured spans also doesn't work.
// That's because it assumes we have exactly one theme, and there is no way to modify the css output.
// Specifically comrak's SyntectAdapter too.
// So it's inevitably going to be a problem once we support dark mode.
// So yea syntect is frustrating, cannot recommend.
// The documentation is bad, the api is bad, and the code is fine at best too. Don't want to be mean but AAAAAAAAAAAAAA.
// It took me three days to start to arrive at the conclusion that we have to do this manually...
pub struct CodeBlockHighlighter {
    pub languages: HashMap<&'static str, HighlightConfiguration>,
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

        // TODO: eliminate unwraps
        let mut highlighter = Highlighter::new();
        let highlights = highlighter
            .highlight(config, code.as_bytes(), None, |_| None)
            .unwrap();

        for highlight in highlights {
            let highlight = highlight.unwrap();

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

pub fn render_md_to_html(markdown: &str, highlighter: &CodeBlockHighlighter) -> String {
    // TOOD: Make this once_cell or function argument
    let options = Options {
        extension: ExtensionOptionsBuilder::default()
            .strikethrough(true)
            .tagfilter(true)
            .table(true)
            .autolink(true)
            .tasklist(true)
            .superscript(true)
            .footnotes(true)
            .multiline_block_quotes(true)
            .build()
            .unwrap(),
        parse: ParseOptionsBuilder::default().smart(true).build().unwrap(),
        render: RenderOptionsBuilder::default()
            .unsafe_(true)
            .build()
            .unwrap(),
    };

    let plugins = PluginsBuilder::default()
        .render(
            RenderPluginsBuilder::default()
                .codefence_syntax_highlighter(Some(highlighter))
                .build()
                .unwrap(),
        )
        .build()
        .unwrap();

    markdown_to_html_with_plugins(markdown, &options, &plugins)
}

// TODO: Tests
