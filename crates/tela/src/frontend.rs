//! Frontends turn a file format into a [`Tree`]; the registry picks one by
//! file extension. The query and lint engines read only trees, so a format
//! joins tela as one more [`Frontend`].

use std::path::Path;

use crate::tree::{Origin, Tree};

/// Malformed input a frontend met; parsing still yields a result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// Where the problem starts.
    pub origin: Origin,
    /// What is wrong, as `problem` nodes carry it.
    pub message: String,
}

/// A parse result with the problems met along the way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed<T> {
    /// What was parsed.
    pub value: T,
    /// Problems in source order.
    pub problems: Vec<Problem>,
}

/// Parses `text` of `file` into a tree under a file node, with `problem`
/// nodes for malformed input.
pub type ParseFn = fn(file: &str, text: &str) -> Parsed<Tree>;

/// A file format: its name, the extensions it claims and its parser.
#[derive(Debug, Clone, Copy)]
pub struct Frontend {
    /// The format's name, such as `markdown`.
    pub name: &'static str,
    /// Lower-case extensions without the dot, such as `md`.
    pub extensions: &'static [&'static str],
    /// The parser.
    pub parse: ParseFn,
}

/// Why a registry was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FrontendError {
    /// Two frontends share a name.
    #[error("frontend {0:?} is registered twice")]
    DuplicateName(&'static str),
    /// Two frontends claim one extension.
    #[error("extension {extension:?} is claimed by {first:?} and {second:?}")]
    DuplicateExtension {
        /// The extension.
        extension: &'static str,
        /// The frontend registered first.
        first: &'static str,
        /// The frontend registered second.
        second: &'static str,
    },
}

/// The frontends a caller reads files with, and the one for files whose
/// extension none claims.
#[derive(Debug, Clone)]
pub struct Frontends {
    frontends: Vec<Frontend>,
    fallback: Option<Frontend>,
}

impl Frontends {
    /// A registry of `frontends`, with `fallback` for named files of an
    /// unclaimed extension.
    ///
    /// # Errors
    /// Returns [`FrontendError::DuplicateName`] when two frontends share a
    /// name and [`FrontendError::DuplicateExtension`] when two claim one
    /// extension.
    pub fn new(
        frontends: Vec<Frontend>,
        fallback: Option<Frontend>,
    ) -> Result<Self, FrontendError> {
        for (i, frontend) in frontends.iter().enumerate() {
            for earlier in &frontends[..i] {
                if earlier.name == frontend.name {
                    return Err(FrontendError::DuplicateName(frontend.name));
                }
                if let Some(&extension) = frontend
                    .extensions
                    .iter()
                    .find(|extension| earlier.extensions.contains(extension))
                {
                    return Err(FrontendError::DuplicateExtension {
                        extension,
                        first: earlier.name,
                        second: frontend.name,
                    });
                }
            }
        }
        Ok(Self {
            frontends,
            fallback,
        })
    }

    /// The registered frontends, in registration order.
    #[must_use]
    pub fn all(&self) -> &[Frontend] {
        &self.frontends
    }

    /// The frontend named `name`.
    #[must_use]
    pub fn by_name(&self, name: &str) -> Option<&Frontend> {
        self.frontends.iter().find(|frontend| frontend.name == name)
    }

    /// The frontend claiming `path`'s extension, compared case-insensitively;
    /// a directory walk reads only these files.
    #[must_use]
    pub fn by_path(&self, path: &Path) -> Option<&Frontend> {
        let extension = path.extension()?.to_str()?;
        self.frontends.iter().find(|frontend| {
            frontend
                .extensions
                .iter()
                .any(|claimed| claimed.eq_ignore_ascii_case(extension))
        })
    }

    /// The frontend for a file the caller named: the one claiming its
    /// extension, else the fallback.
    #[must_use]
    pub fn for_named(&self, path: &Path) -> Option<&Frontend> {
        self.by_path(path).or(self.fallback.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::Node;

    fn parse_as(kind: &'static str) -> impl Fn(&str) -> Parsed<Tree> {
        move |file| Parsed {
            value: Tree::new(Node::new(kind, Origin::new(file, None).unwrap())),
            problems: vec![],
        }
    }

    fn markdown(file: &str, _: &str) -> Parsed<Tree> {
        parse_as("markdown")(file)
    }

    fn plain(file: &str, _: &str) -> Parsed<Tree> {
        parse_as("plain")(file)
    }

    const MARKDOWN: Frontend = Frontend {
        name: "markdown",
        extensions: &["md", "markdown"],
        parse: markdown,
    };

    const PLAIN: Frontend = Frontend {
        name: "plain",
        extensions: &["txt"],
        parse: plain,
    };

    fn registry() -> Frontends {
        Frontends::new(vec![MARKDOWN, PLAIN], Some(PLAIN)).unwrap()
    }

    fn name(frontend: Option<&Frontend>) -> Option<&'static str> {
        frontend.map(|f| f.name)
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
        let strict = Frontends::new(vec![MARKDOWN], None).unwrap();
        assert_eq!(name(strict.for_named(Path::new("notes.org"))), None);
    }

    #[test]
    fn a_frontend_parses_through_its_entry() {
        let frontends = registry();
        let parsed = (frontends.by_name("markdown").unwrap().parse)("a.md", "# A");
        assert_eq!(parsed.value.root().node().kind, "markdown");
    }

    #[test]
    fn names_and_extensions_are_unique() {
        assert_eq!(
            Frontends::new(vec![MARKDOWN, MARKDOWN], None).err(),
            Some(FrontendError::DuplicateName("markdown"))
        );
        let rival = Frontend {
            name: "commonmark",
            extensions: &["md"],
            parse: markdown,
        };
        assert_eq!(
            Frontends::new(vec![MARKDOWN, rival], None).err(),
            Some(FrontendError::DuplicateExtension {
                extension: "md",
                first: "markdown",
                second: "commonmark",
            })
        );
    }
}
