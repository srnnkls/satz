//! The Markdown frontend, which reads Markdown with tables, raw HTML,
//! directives and YAML frontmatter, and the directive-aware document editor.

mod attributes;
mod builder;
mod directive;
mod frontmatter;
mod parse;
mod yaml;

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

pub use frontmatter::{Entry, Frontmatter};
pub use parse::Markdown;

use crate::tree::Origin;

/// Malformed input met while parsing a document or its frontmatter.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{origin}: {message}")]
pub struct Problem {
    /// Where the problem starts.
    pub origin: Origin,
    /// What is wrong.
    pub message: String,
}

/// A parse result with the problems met along the way; parsing never fails
/// outright.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed<T> {
    /// What was parsed.
    pub value: T,
    /// Problems in source order.
    pub problems: Vec<Problem>,
}

/// One expansion step: the directive that invoked a component and the
/// component line that produced the item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    /// The invoking directive.
    pub directive: Origin,
    /// The component line that produced the item.
    pub component: Origin,
}

/// The expansion steps behind an item, outermost first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Expansion(pub Vec<Call>);

/// A mutable node of a [`Document`], named with the tree's node types.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Item {
    /// The node type.
    pub kind: String,
    /// Keyed attributes.
    pub attrs: BTreeMap<String, String>,
    /// The literal content without block children: a paragraph's Markdown,
    /// a heading's title, a code block's contents, a directive's label.
    pub text: String,
    /// Block children.
    pub children: Vec<Item>,
    /// Where the item was parsed from.
    pub origin: Origin,
    /// How expansion produced it.
    pub expansion: Expansion,
}

impl Item {
    /// A new item with no attributes, text, children or expansion.
    #[must_use]
    pub fn new(kind: impl Into<String>, origin: Origin) -> Self {
        Self {
            kind: kind.into(),
            attrs: BTreeMap::new(),
            text: String::new(),
            children: Vec::new(),
            origin,
            expansion: Expansion::default(),
        }
    }
}

/// How directives render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderFormat {
    /// Leave directives as written.
    Keep,
    /// Render directives as XML elements.
    Xml,
    /// Render directives as plain Markdown.
    Markdown,
}

/// A render format name other than `keep`, `xml` or `markdown`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown render format {0:?}; expected keep, xml or markdown")]
pub struct UnknownFormat(pub String);

impl FromStr for RenderFormat {
    type Err = UnknownFormat;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "keep" => Ok(Self::Keep),
            "xml" => Ok(Self::Xml),
            "markdown" => Ok(Self::Markdown),
            other => Err(UnknownFormat(other.to_owned())),
        }
    }
}

impl fmt::Display for RenderFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Keep => "keep",
            Self::Xml => "xml",
            Self::Markdown => "markdown",
        })
    }
}

/// A parsed file whose items keep their origins. Its [`Display`](fmt::Display)
/// writes it back: untouched items byte for byte, edited items in canonical
/// form.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Document {
    /// The file it was parsed from.
    pub file: String,
    /// The top-level items.
    pub children: Vec<Item>,
}

impl Document {
    /// An empty document for `file`.
    #[must_use]
    pub fn new(file: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            children: Vec::new(),
        }
    }

    /// Parses `text` of `file` into an editable document.
    #[must_use]
    #[expect(
        clippy::todo,
        reason = "the document parser is ported from henia's markup package"
    )]
    pub fn parse(file: &str, text: &str) -> Parsed<Self> {
        let _ = (file, text);
        todo!()
    }

    /// Renders the document's directives in place.
    ///
    /// # Errors
    /// Returns the [`Problem`] of a directive that cannot render in `format`.
    #[expect(
        clippy::todo,
        reason = "rendering is ported from henia's markup package"
    )]
    pub fn render(&mut self, format: RenderFormat) -> Result<(), Problem> {
        let _ = format;
        todo!()
    }

    /// The document with each directive named `name` replaced by its
    /// content.
    #[must_use]
    #[expect(
        clippy::todo,
        reason = "unwrapping is ported from henia's markup package"
    )]
    pub fn unwrap_directive(&self, name: &str) -> Self {
        let _ = name;
        todo!()
    }
}

impl fmt::Display for Document {
    #[expect(clippy::todo, reason = "writing is ported from henia's markup package")]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _ = f;
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_formats_round_trip_their_names() {
        for format in [
            RenderFormat::Keep,
            RenderFormat::Xml,
            RenderFormat::Markdown,
        ] {
            assert_eq!(format.to_string().parse(), Ok(format));
        }
        assert_eq!(
            "html".parse::<RenderFormat>(),
            Err(UnknownFormat("html".to_owned()))
        );
    }

    #[test]
    fn problems_are_errors_that_name_their_origin() {
        let problem = Problem {
            origin: Origin::new("a.md", None).unwrap(),
            message: "unclosed directive".to_owned(),
        };
        let error: Box<dyn std::error::Error> = Box::new(problem);
        assert_eq!(error.to_string(), "a.md: unclosed directive");
    }
}
