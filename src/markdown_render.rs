use comrak::{
    markdown_to_html, ExtensionOptionsBuilder, Options, ParseOptionsBuilder, RenderOptionsBuilder,
};

pub fn render_md_to_html(markdown: &str) -> String {
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

    markdown_to_html(markdown, &options)
}
