//! Frontends turn a file format into a [`Tree`]; the registry picks one by
//! file extension. The query and lint engines read only trees, so a format
//! joins satz as one more [`Frontend`].

use std::fmt;
use std::iter::FusedIterator;
use std::path::Path;
use std::slice;
use std::sync::Arc;

use crate::query::{Schema, SchemaError, SchemaSpec};
use crate::tree::Tree;

/// A file format: its name, the extensions it claims and its parser.
///
/// A frontend owns whatever its parser needs, such as a configured parser
/// built once and reused for every file.
pub trait Frontend: Send + Sync + fmt::Debug {
    /// The format's name, such as `markdown`.
    fn name(&self) -> &str;

    /// The extensions it claims, without the dot, such as `md`.
    fn extensions(&self) -> &[&str];

    /// Parses `text` of `file` into a tree under a file node. Malformed input
    /// becomes `problem` nodes; parsing never fails outright.
    fn parse(&self, file: &str, text: &str) -> Tree;

    /// The node types and keys its trees hold beyond the core schema's.
    fn schema(&self) -> SchemaSpec {
        SchemaSpec::default()
    }
}

/// Why a registry was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum FrontendError {
    /// Two frontends share a name.
    #[error("frontend {0:?} is registered twice")]
    DuplicateName(String),
    /// Two frontends claim one extension, compared case-insensitively.
    #[error("extension {extension:?} is claimed by {first:?} and {second:?}")]
    DuplicateExtension {
        /// The extension.
        extension: String,
        /// The frontend registered first.
        first: String,
        /// The frontend registered second.
        second: String,
    },
    /// An extension is empty or contains a dot.
    #[error("frontend {frontend:?} claims the invalid extension {extension:?}")]
    InvalidExtension {
        /// The frontend.
        frontend: String,
        /// The extension.
        extension: String,
    },
    /// The fallback names no registered frontend.
    #[error("fallback frontend {0:?} is not registered")]
    UnknownFallback(String),
}

/// The frontends a caller reads files with, and the one for named files whose
/// extension none claims.
#[derive(Debug, Clone)]
pub struct Frontends {
    frontends: Vec<Arc<dyn Frontend>>,
    fallback: Option<usize>,
}

impl Frontends {
    /// A registry of `frontends`, falling back to the one named `fallback`.
    ///
    /// # Errors
    /// Returns [`FrontendError::DuplicateName`] or
    /// [`FrontendError::DuplicateExtension`] when two frontends share a name
    /// or claim one extension, [`FrontendError::InvalidExtension`] for an
    /// empty or dotted extension, and [`FrontendError::UnknownFallback`] when
    /// no registered frontend is named `fallback`.
    pub fn new(
        frontends: Vec<Arc<dyn Frontend>>,
        fallback: Option<&str>,
    ) -> Result<Self, FrontendError> {
        for (i, frontend) in frontends.iter().enumerate() {
            if let Some(extension) = frontend
                .extensions()
                .iter()
                .find(|e| e.is_empty() || e.contains('.'))
            {
                return Err(FrontendError::InvalidExtension {
                    frontend: frontend.name().to_owned(),
                    extension: (*extension).to_owned(),
                });
            }
            for earlier in &frontends[..i] {
                if earlier.name() == frontend.name() {
                    return Err(FrontendError::DuplicateName(frontend.name().to_owned()));
                }
                if let Some(extension) = frontend
                    .extensions()
                    .iter()
                    .find(|e| claims(earlier.as_ref(), e))
                {
                    return Err(FrontendError::DuplicateExtension {
                        extension: (*extension).to_owned(),
                        first: earlier.name().to_owned(),
                        second: frontend.name().to_owned(),
                    });
                }
            }
        }
        let fallback = fallback
            .map(|name| {
                frontends
                    .iter()
                    .position(|frontend| frontend.name() == name)
                    .ok_or_else(|| FrontendError::UnknownFallback(name.to_owned()))
            })
            .transpose()?;
        Ok(Self {
            frontends,
            fallback,
        })
    }

    /// The registered frontends, in registration order.
    #[must_use]
    pub fn iter(&self) -> FrontendIter<'_> {
        FrontendIter(self.frontends.iter())
    }

    /// The frontend named `name`.
    #[must_use]
    pub fn by_name(&self, name: &str) -> Option<&dyn Frontend> {
        self.iter().find(|frontend| frontend.name() == name)
    }

    /// The frontend claiming `path`'s extension, compared case-insensitively;
    /// a directory walk reads only these files.
    #[must_use]
    pub fn by_path(&self, path: &Path) -> Option<&dyn Frontend> {
        let extension = path.extension()?.to_str()?;
        self.iter().find(|frontend| claims(*frontend, extension))
    }

    /// The schema of the trees the frontends produce: the core schema with
    /// every frontend's types and keys; a core type a frontend registers
    /// takes its keys.
    ///
    /// # Errors
    /// Returns [`SchemaError::UnknownType`] when a frontend adds keys to a
    /// type no schema registers.
    pub fn schema(&self) -> Result<Schema, SchemaError> {
        let core = Schema::core();
        let merged = self
            .iter()
            .map(Frontend::schema)
            .fold(SchemaSpec::default(), SchemaSpec::merge);
        let mut spec = SchemaSpec {
            keys: merged.keys,
            numeric: merged.numeric,
            unit: merged.unit,
            ..SchemaSpec::default()
        };
        for kind in merged.types {
            if core.has_type(&kind.name) {
                spec.keys.entry(kind.name).or_default().extend(kind.keys);
            } else {
                spec.types.push(kind);
            }
        }
        Schema::new(spec)
    }

    /// The frontend for a file the caller named: the one claiming its
    /// extension, else the fallback.
    #[must_use]
    pub fn for_named(&self, path: &Path) -> Option<&dyn Frontend> {
        self.by_path(path)
            .or_else(|| self.fallback.map(|i| self.frontends[i].as_ref()))
    }
}

impl<'a> IntoIterator for &'a Frontends {
    type Item = &'a dyn Frontend;
    type IntoIter = FrontendIter<'a>;

    fn into_iter(self) -> FrontendIter<'a> {
        self.iter()
    }
}

/// The frontends of a [`Frontends`] registry, in registration order.
#[derive(Debug, Clone)]
pub struct FrontendIter<'a>(slice::Iter<'a, Arc<dyn Frontend>>);

impl<'a> Iterator for FrontendIter<'a> {
    type Item = &'a dyn Frontend;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(AsRef::as_ref)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl DoubleEndedIterator for FrontendIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(AsRef::as_ref)
    }
}

impl ExactSizeIterator for FrontendIter<'_> {}

impl FusedIterator for FrontendIter<'_> {}

fn claims(frontend: &dyn Frontend, extension: &str) -> bool {
    frontend
        .extensions()
        .iter()
        .any(|claimed| claimed.eq_ignore_ascii_case(extension))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::tree::{Node, Origin};

    #[derive(Debug)]
    struct Fake {
        name: &'static str,
        extensions: &'static [&'static str],
    }

    impl Frontend for Fake {
        fn name(&self) -> &str {
            self.name
        }

        fn extensions(&self) -> &[&str] {
            self.extensions
        }

        fn parse(&self, file: &str, _: &str) -> Tree {
            Tree::new(Node::new(self.name, Origin::new(file, None).unwrap()))
        }
    }

    fn fake(name: &'static str, extensions: &'static [&'static str]) -> Arc<dyn Frontend> {
        Arc::new(Fake { name, extensions })
    }

    fn registry() -> Frontends {
        Frontends::new(
            vec![
                fake("markdown", &["md", "markdown"]),
                fake("plain", &["txt"]),
            ],
            Some("plain"),
        )
        .unwrap()
    }

    fn name(frontend: Option<&dyn Frontend>) -> Option<&str> {
        frontend.map(Frontend::name)
    }

    #[derive(Debug)]
    struct Elements;

    impl Frontend for Elements {
        fn name(&self) -> &'static str {
            "html"
        }

        fn extensions(&self) -> &[&str] {
            &["html"]
        }

        fn parse(&self, file: &str, _: &str) -> Tree {
            Tree::new(Node::new("file", Origin::new(file, None).unwrap()))
        }

        fn schema(&self) -> SchemaSpec {
            SchemaSpec {
                types: vec![
                    crate::query::TypeSpec::new("element", ["tag", "depth"]),
                    crate::query::TypeSpec::new("link", ["rel"]),
                ],
                numeric: BTreeSet::from(["depth".to_owned()]),
                ..SchemaSpec::default()
            }
        }
    }

    #[test]
    fn a_frontend_schema_makes_its_types_and_keys_queryable() {
        let frontends =
            Frontends::new(vec![fake("plain", &["txt"]), Arc::new(Elements)], None).unwrap();
        let schema = frontends.schema().unwrap();
        let read = |source| crate::query::Query::read(source, &schema, None);
        read(r#"(element :tag "div" :depth 2..3)"#).unwrap();
        read(r#"(link :rel "next" :url ?u @u)"#).unwrap();
        assert!(crate::query::Query::read("(element)", &Schema::core(), None).is_err());
    }

    #[test]
    fn paths_resolve_by_extension_ignoring_case() {
        let frontends = registry();
        assert_eq!(
            name(frontends.by_path(Path::new("docs/a.md"))),
            Some("markdown")
        );
        assert_eq!(
            name(frontends.by_path(Path::new("README.MD"))),
            Some("markdown")
        );
        assert_eq!(name(frontends.by_path(Path::new("notes.org"))), None);
        assert_eq!(name(frontends.by_path(Path::new("Makefile"))), None);
    }

    #[test]
    fn named_files_fall_back_when_no_frontend_claims_them() {
        let frontends = registry();
        assert_eq!(
            name(frontends.for_named(Path::new("notes.org"))),
            Some("plain")
        );
        let strict = Frontends::new(vec![fake("markdown", &["md"])], None).unwrap();
        assert_eq!(name(strict.for_named(Path::new("notes.org"))), None);
    }

    #[test]
    fn a_frontend_parses_through_the_registry() {
        let tree = registry().by_name("markdown").unwrap().parse("a.md", "# A");
        assert_eq!(tree.root().node().kind, "markdown");
    }

    #[test]
    fn names_and_extensions_are_unique_ignoring_case() {
        let twice = Frontends::new(
            vec![fake("markdown", &["md"]), fake("markdown", &["mdx"])],
            None,
        );
        assert_eq!(
            twice.err(),
            Some(FrontendError::DuplicateName("markdown".to_owned()))
        );
        let rival = Frontends::new(
            vec![fake("markdown", &["md"]), fake("commonmark", &["MD"])],
            None,
        );
        assert_eq!(
            rival.err(),
            Some(FrontendError::DuplicateExtension {
                extension: "MD".to_owned(),
                first: "markdown".to_owned(),
                second: "commonmark".to_owned(),
            })
        );
    }

    #[test]
    fn extensions_and_fallback_are_validated() {
        let dotted = Frontends::new(vec![fake("markdown", &[".md"])], None);
        assert!(matches!(
            dotted,
            Err(FrontendError::InvalidExtension { .. })
        ));
        let missing = Frontends::new(vec![fake("markdown", &["md"])], Some("plain"));
        assert_eq!(
            missing.err(),
            Some(FrontendError::UnknownFallback("plain".to_owned()))
        );
    }
}
