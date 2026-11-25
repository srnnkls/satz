//! The Markdown frontend, which reads Markdown with tables, raw HTML,
//! directives and YAML frontmatter, and the directive-aware document editor.

mod attributes;
mod builder;
mod directive;
mod document;
mod frontmatter;
mod mapping;
mod parse;
#[cfg(test)]
mod properties;
mod render;
#[cfg(test)]
mod testing;
mod write;
mod yaml;

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

pub use frontmatter::{Entry, Frontmatter};
pub use parse::Markdown;

use self::document::Snapshot;
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
///
/// A parsed item remembers its source, so that writing it back keeps its
/// bytes until it is edited; its clones share that memory. Equality
/// compares the public fields.
#[derive(Clone)]
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
    snapshot: Option<Arc<Snapshot>>,
    anchors: Vec<Arc<Snapshot>>,
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
            snapshot: None,
            anchors: Vec::new(),
        }
    }

    /// `items` marked as replacing this item: placed among its former
    /// siblings, they are written where it stood, after its leading source.
    #[must_use]
    pub fn replace(&self, items: impl IntoIterator<Item = Self>) -> Vec<Self> {
        let chain: Vec<Arc<Snapshot>> =
            self.snapshot.iter().chain(&self.anchors).cloned().collect();
        items
            .into_iter()
            .map(|mut item| {
                let anchored = item
                    .snapshot
                    .as_ref()
                    .is_some_and(|own| chain.iter().any(|a| Arc::ptr_eq(a, own)));
                if !anchored {
                    item.anchors.clone_from(&chain);
                }
                item
            })
            .collect()
    }
}

impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.attrs == other.attrs
            && self.text == other.text
            && self.children == other.children
            && self.origin == other.origin
            && self.expansion == other.expansion
    }
}

impl Eq for Item {}

impl fmt::Debug for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Item")
            .field("kind", &self.kind)
            .field("attrs", &self.attrs)
            .field("text", &self.text)
            .field("children", &self.children)
            .field("origin", &self.origin)
            .field("expansion", &self.expansion)
            .finish_non_exhaustive()
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
///
/// Container fences take canonical lengths, each one colon longer than the
/// longest fence it holds, and NUL reads as U+FFFD. An item that moved is re-indented to its new
/// place; an item from another document writes its own source.
#[derive(Clone)]
#[non_exhaustive]
pub struct Document {
    /// The file it was parsed from.
    pub file: String,
    /// The top-level items.
    pub children: Vec<Item>,
    snapshot: Option<Arc<Snapshot>>,
}

impl Document {
    /// An empty document for `file`.
    #[must_use]
    pub fn new(file: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            children: Vec::new(),
            snapshot: None,
        }
    }

    /// Parses the Markdown body `text` of `file` into an editable document;
    /// frontmatter is the caller's to strip. Malformed directives are
    /// problems, and the document holds what was read.
    #[must_use]
    pub fn parse(file: &str, text: &str) -> Parsed<Self> {
        document::parse(file, text)
    }

    /// Renders the document's directives in place: as normalized directive
    /// syntax, XML elements or plain Markdown. Each block that changes is
    /// replaced by a `text` item of its rendering.
    ///
    /// # Errors
    /// Returns the first [`Problem`] of a block that cannot render; such
    /// blocks keep their source while the others render.
    pub fn render(&mut self, format: RenderFormat) -> Result<(), Problem> {
        render::render(self, format)
    }

    /// Replaces each block directive named `name` with its content, and
    /// returns a document of those contents in order.
    #[must_use = "the returned document holds the unwrapped contents"]
    pub fn unwrap_directive(&mut self, name: &str) -> Self {
        render::unwrap(self, name)
    }
}

impl PartialEq for Document {
    fn eq(&self, other: &Self) -> bool {
        self.file == other.file && self.children == other.children
    }
}

impl Eq for Document {}

impl fmt::Debug for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Document")
            .field("file", &self.file)
            .field("children", &self.children)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&String::from_utf8_lossy(&write::document(self)))
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
