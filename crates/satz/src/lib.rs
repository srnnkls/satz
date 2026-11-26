//! Structural query and lint over text.
//!
//! A [`Frontend`] parses a file format into a [`Tree`] of nodes, a
//! [`query::Query`] written in sq matches that tree, and a lint
//! [`lint::Plan`] turns the rows of its rules' queries into
//! [`lint::Diagnostic`]s. The core reads only trees; Markdown
//! ([`markdown::Markdown`]) and plain text ([`plain::Plain`]) are its
//! frontends. The sq language and the lint module format are documented in
//! the repository's `docs/query.md` and `docs/lint.md`.
//!
//! # Frontends and trees
//!
//! [`Frontends`] is the registry a program reads files with: it picks the
//! frontend claiming a path's extension ([`Frontends::by_path`]), or, for a
//! file the user named, falls back to a named frontend
//! ([`Frontends::for_named`]). [`Frontend::parse`] never fails: malformed
//! input becomes `problem` nodes, listed by [`Tree::problems`].
//!
//! A [`Tree`] is an arena addressed by [`NodeId`] and read through
//! [`NodeRef`] handles. Every node carries its [`Origin`], the file and
//! [`Position`] every reported location comes from. A file node's `path`
//! attribute, which `(file :path …)` reads, is the caller's to set.
//!
//! # Queries
//!
//! [`query::Query::read`] reads sq against a [`query::Schema`];
//! [`query::Query::run`] matches it over a shared tree in a
//! [`query::Environment`], which holds everything the
//! caller knows in advance: `$param` values, data tables, the semantic model
//! and the clock. A query never writes to the tree, so one tree serves
//! concurrent runs.
//!
//! ```
//! use std::path::Path;
//! use std::sync::Arc;
//! use std::time::SystemTime;
//!
//! use satz::markdown::Markdown;
//! use satz::plain::Plain;
//! use satz::Frontends;
//! use satz::query::{Capture, Environment, Hit, Query, Schema};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let frontends = Frontends::new(vec![Arc::new(Markdown::new()), Arc::new(Plain)], Some("plain"))?;
//! let markdown = frontends.by_path(Path::new("guide.md")).ok_or("no frontend")?;
//! let tree = markdown.parse("guide.md", "# Guide\n\n## Usage\n\nRun it.\n\n## Usage\n\nAgain.\n");
//!
//! let query = Query::read("(heading :level 2) @h", &Schema::core(), None)?;
//! let rows = query.run(&tree, &Environment::new(SystemTime::now()))?;
//! let lines: Vec<u32> = rows
//!     .iter()
//!     .filter_map(|row| match row.captures.get("h")? {
//!         Capture::Nodes(hits) => match hits.first()? {
//!             Hit::Node(id) => tree.get(*id)?.node().origin.line(),
//!             _ => None,
//!         },
//!         _ => None,
//!     })
//!     .collect();
//! assert_eq!(lines, [3, 7]);
//! # Ok(())
//! # }
//! ```
//!
//! [`query::Schema::core`] knows the node types every frontend produces.
//! [`query::Schema::new`] extends it through a [`query::SchemaSpec`]:
//! further node types and keys, and a unit, the node type that the relations
//! `reaches`, `inbound`, `to` and `from` work over; without a unit, a query
//! using them does not read. The links between units are
//! precomputed: [`query::Environment::targets`] maps a link target to its
//! unit node, and [`query::Environment::typed`] gives attributes typed
//! values.
//!
//! # Lint
//!
//! A [`lint::Config`] holds module [`lint::Layer`]s in load order, the
//! program's [`lint::Spec`] (its schema, the extra module frontmatter keys it
//! declares, and the labels of its rule sources) and the user's
//! [`lint::Settings`]: inline rules, per-rule overrides and disabled rules.
//! [`lint::Plan::compile`] loads satz's std modules, then the layers and the
//! inline rules, and applies the settings. [`lint::Plan::run`] runs every
//! rule, or a [`lint::Selection`], over one [`lint::Corpus`]: the files
//! grafted under one root, and the environment.
//!
//! ````
//! use std::sync::Arc;
//! use std::time::SystemTime;
//!
//! use satz::Frontend;
//! use satz::lint::{Config, Corpus, Layer, ModuleFile, Plan, Selection, Settings, Spec};
//! use satz::markdown::Markdown;
//! use satz::query::Environment;
//!
//! const MODULE: &str = r#"
//! ## no-make
//!
//! ```sq
//! (rule no-make
//!   :message "use the mise task, not {@c.text}"
//!   (code :inline true :matches /^make\b/) @c)
//! ```
//! "#;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let layer = Layer::from_files(
//!     Some("team".to_owned()),
//!     vec![ModuleFile::new("build.md", MODULE)],
//! )?;
//! let config = Config::new(vec![layer], Spec::default(), Settings::default())?;
//! let plan = Plan::compile(&config)?;
//!
//! let tree = Markdown::new().parse("guide.md", "# Guide\n\nRun `make release`.\n");
//! let corpus = Corpus::new([tree], Environment::new(SystemTime::now()))?;
//! let found = plan.run(&corpus, &Selection::All)?;
//! assert_eq!(found.len(), 1);
//! assert_eq!(found[0].rule().as_str(), "no-make");
//! assert_eq!(found[0].message(), "use the mise task, not make release");
//! assert_eq!(found[0].origin().line(), Some(3));
//! # Ok(())
//! # }
//! ````
//!
//! A module is named by its path without `.md`, under its layer's package:
//! the rule above comes from `team/build`. Its rules live in `sq` code
//! blocks; `hq` blocks read the same. [`lint::Layer::read_dir`] builds a
//! layer from a directory. Module frontmatter keys the [`lint::Spec`]
//! declares come back raw per rule in [`lint::Plan::rules`]; any other
//! unknown key fails the compile. [`lint::Plan::examples`] lists every
//! rule's Matches and Passes examples, and [`lint::Plan::run_example`] runs
//! one against a corpus the caller builds from its text.
//!
//! # Semantic similarity
//!
//! `(similar ?t θ)` and the `semantic-content` rule embed texts through the
//! [`similarity::Embed`] in [`query::Environment::embed`]. With the
//! `semantic` feature, `similarity::Model2Vec` loads a local `Model2Vec`
//! directory; any other model implements the trait:
//!
//! ```
//! use std::sync::Arc;
//! use std::time::SystemTime;
//!
//! use satz::markdown::Markdown;
//! use satz::Frontend;
//! use satz::query::{Environment, Query, Schema};
//! use satz::similarity::{Embed, EmbedError};
//!
//! /// Embeds a text as its counts of vowels.
//! #[derive(Debug)]
//! struct Vowels;
//!
//! impl Embed for Vowels {
//!     fn model(&self) -> &str {
//!         "vowels"
//!     }
//!
//!     fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
//!         Ok("aeiou".chars().map(|v| text.matches(v).count() as f32).collect())
//!     }
//! }
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let tree = Markdown::new().parse("a.md", "Banana bread.\n\nBanana cake.\n\nPlum.\n");
//! let mut environment = Environment::new(SystemTime::now());
//! environment.embed = Some(Arc::new(Vowels));
//! let query = Query::read(
//!     "(paragraph :node ?n :text ?t) @a (paragraph :node (after ?n) :text (similar ?t 0.9 @s)) @b",
//!     &Schema::core(),
//!     None,
//! )?;
//! assert_eq!(query.run(&tree, &environment)?.len(), 1);
//! # Ok(())
//! # }
//! ```
#![forbid(unsafe_code)]

pub mod edit;
pub mod frontend;
pub mod lint;
pub mod markdown;
pub mod plain;
pub mod query;
pub mod similarity;
mod template;
pub mod tree;

pub use frontend::{Frontend, Frontends};
pub use tree::{Node, NodeId, NodeRef, Origin, Position, Tree};
