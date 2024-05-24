use crate::markdown_render::CssClassNameGenerator;
use itertools::Itertools;
use serde::de::{Error as DeserializeError, Unexpected};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::io::Write;
use std::ops::Deref;
use tree_sitter_highlight::Highlight;

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

// TODO: Nicer deserialization/serialization
#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    #[serde(default)]
    pub a: Option<u8>,
}

#[derive(Copy, Clone, Eq, PartialEq, Default, Debug, Serialize, Deserialize)]
pub struct Style {
    #[serde(default)]
    pub color: Option<Color>,
    #[serde(default)]
    pub bg_color: Option<Color>,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub underline: bool,
    #[serde(default)]
    pub italic: bool,
}

// TODO: Background color?
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Theme {
    pub styles: BTreeMap<usize, Style>,
}

impl Color {
    pub fn to_css_color(&self) -> String {
        let Color { r, g, b, a } = self;

        if let Some(a) = a {
            format!("rgba({r}, {g}, {b}, {a})")
        } else {
            format!("rgb({r}, {g}, {b})")
        }
    }
}

impl Style {
    pub fn to_css_declarations(&self) -> Vec<String> {
        let mut out = Vec::new();

        if let Some(color) = self.color {
            out.push(format!("color: {};", color.to_css_color()));
        }
        if let Some(bg_color) = self.bg_color {
            out.push(format!("background-color: {};", bg_color.to_css_color()));
        }
        if self.underline {
            out.push("text-decoration: underline;".to_string());
        }
        if self.bold {
            out.push("font-weight: bold;".to_string());
        }
        if self.italic {
            out.push("font-style: italic;".to_string());
        }

        out
    }
}

impl Theme {
    pub fn write_css_with_class_names<W: Write, G: CssClassNameGenerator>(
        &self,
        out: &mut W,
        class_name_generator: &G,
    ) -> std::io::Result<()> {
        for (&highlight_idx, style) in &self.styles {
            if let Some(class) = class_name_generator
                .class_for_highlight(HIGHLIGHT_NAMES[highlight_idx], highlight_idx)
            {
                writeln!(out, ".{class} {{")?;

                for declaration in style.to_css_declarations() {
                    writeln!(out, "  {declaration}")?;
                }

                out.write_all(b"}\n")?;
            }
        }

        Ok(())
    }
}

impl Serialize for Theme {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        #[serde(transparent)]
        struct IntermediateStyle<'a>(Vec<IntermediateStyleRule<'a>>);

        #[derive(Serialize)]
        struct IntermediateStyleRule<'a> {
            highlight_names: Vec<&'a str>,
            style: Style,
        }

        let rules = self
            .styles
            .iter()
            .map(|(&idx, &style)| IntermediateStyleRule {
                highlight_names: vec![HIGHLIGHT_NAMES[idx]],
                style,
            })
            .collect();

        IntermediateStyle(rules).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Theme {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(transparent)]
        struct IntermediateStyle(Vec<IntermediateStyleRule>);

        #[derive(Deserialize)]
        struct IntermediateStyleRule {
            highlight_names: Vec<String>,
            style: Style,
        }

        let IntermediateStyle(rules) = IntermediateStyle::deserialize(deserializer)?;

        let styles = rules
            .into_iter()
            .flat_map(
                |IntermediateStyleRule {
                     highlight_names,
                     style,
                 }| {
                    highlight_names.into_iter().map(move |name| {
                        HIGHLIGHT_NAMES
                            .into_iter()
                            .find_position(|&highlight_name| highlight_name == name)
                            .ok_or_else(|| {
                                DeserializeError::invalid_value(
                                    Unexpected::Str(&name),
                                    &"Valid HIGHLIGHT_NAME",
                                )
                            })
                            .map(|(position, _)| (position, style))
                    })
                },
            )
            .collect::<Result<BTreeMap<_, _>, _>>()?;

        Ok(Theme { styles })
    }
}

pub fn write_html_highlight_start<W: Write + ?Sized, G: CssClassNameGenerator>(
    out: &mut W,
    Highlight(highlight_idx): Highlight,
    elem: &str,
    additional_attributes: &HashMap<String, Option<String>>,
    class_name_generator: &G,
) -> std::io::Result<()> {
    write!(out, "<{elem}")?;

    let additional_classes = additional_attributes.get("class");
    let mut classes_with_spaces = class_name_generator
        .class_for_highlight(HIGHLIGHT_NAMES[highlight_idx], highlight_idx)
        .into_iter()
        .chain(additional_classes.and_then(|string| Some(Cow::Borrowed(string.as_ref()?.deref()))))
        .intersperse(Cow::Borrowed(" "))
        .peekable();

    if classes_with_spaces.peek().is_some() {
        out.write_all(b" class=\"")?;

        for class_or_space in classes_with_spaces {
            out.write_all(class_or_space.as_bytes())?;
        }

        out.write_all(b"\"")?;
    }

    for (attr, value) in additional_attributes {
        if attr == "class" {
            continue;
        }

        write!(out, " {attr}")?;
        if let Some(value) = value {
            write!(out, r#"="{value}""#)?;
        }
    }

    out.write_all(b">")
}

pub fn write_html_highlight_end<W: Write + ?Sized>(out: &mut W, elem: &str) -> std::io::Result<()> {
    write!(out, "</{elem}>")
}

pub fn write_html_highlight_unescaped<W: Write + ?Sized, G: CssClassNameGenerator>(
    out: &mut W,
    highlight: Highlight,
    elem: &str,
    additional_attributes: &HashMap<String, Option<String>>,
    code: &str,
    class_name_generator: &G,
) -> std::io::Result<()> {
    write_html_highlight_start(
        out,
        highlight,
        elem,
        additional_attributes,
        class_name_generator,
    )?;
    out.write_all(code.as_bytes())?;
    write_html_highlight_end(out, elem)
}
