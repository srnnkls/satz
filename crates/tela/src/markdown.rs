//! The Markdown frontend: node trees, frontmatter and the directive-aware
//! document editor, registered as a [`Frontend`](crate::frontend::Frontend).

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use crate::frontend::{Parsed, Problem};
use crate::tree::Origin;

/// One frontmatter key, or one element of a list value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The key.
    pub key: String,
    /// The scalar value as written.
    pub value: String,
    /// The YAML short tag, such as `!!str` or `!!int`.
    pub tag: String,
    /// The element's index when the value is a list.
    pub index: Option<usize>,
}

/// A file's leading YAML block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frontmatter {
    /// The entries in source order.
    pub entries: Vec<Entry>,
    /// The lines the block spans, fences included; body lines shift by this.
    pub lines: u32,
}

/// Splits the frontmatter off `text`, if it opens with one.
pub type FrontmatterFn = fn(file: &str, text: &str) -> Parsed<Option<Frontmatter>>;

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
pub struct Backtrace(pub Vec<Call>);

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
    pub backtrace: Backtrace,
}

impl Item {
    /// A new item with no attributes, text, children or backtrace.
    #[must_use]
    pub fn new(kind: impl Into<String>, origin: Origin) -> Self {
        Self {
            kind: kind.into(),
            attrs: BTreeMap::new(),
            text: String::new(),
            children: Vec::new(),
            origin,
            backtrace: Backtrace::default(),
        }
    }
}

/// A parsed file whose items keep their origins; writing it back leaves
/// untouched source intact.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Document {
    /// The file it was parsed from.
    pub file: String,
    /// The top-level items.
    pub children: Vec<Item>,
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

/// Parses `text` of `file` into an editable document.
pub type ParseDocumentFn = fn(file: &str, text: &str) -> Parsed<Document>;

/// Writes a document back: untouched items byte for byte, edited items in
/// canonical form.
pub type WriteFn = fn(document: &Document) -> String;

/// Renders the document's directives in place.
pub type RenderFn = fn(document: &mut Document, format: RenderFormat) -> Result<(), Problem>;

/// Replaces each directive named `name` by its content.
pub type UnwrapFn = fn(document: &Document, name: &str) -> Document;

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
}
