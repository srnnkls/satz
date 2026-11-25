//! Structural queries in tq: the schema, environment, and the query
//! and row types its reader and matcher exchange.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;
use std::time::SystemTime;

use crate::similarity::{Embed, EmbedError};
use crate::tree::{NodeId, Tree};

/// Keys every node type takes.
const TEXT_KEYS: [&str; 10] = [
    "text", "contains", "matches", "words", "node", "position", "lines", "chars", "norm", "dynamic",
];

/// Keys compared as numbers.
const NUMERIC_KEYS: [&str; 4] = ["level", "words", "lines", "chars"];

/// Types that accept any key.
const OPEN_TYPES: [&str; 3] = ["_", "directive", "frontmatter"];

const CORE_TYPES: [(&str, &[&str]); 18] = [
    ("file", &["path"]),
    ("section", &["id", "title", "level"]),
    ("heading", &["level"]),
    ("paragraph", &[]),
    ("list", &["ordered"]),
    ("item", &[]),
    ("quote", &[]),
    ("table", &[]),
    ("code", &["lang", "inline", "ticks"]),
    ("row", &["table", "key", "value", "index", "header"]),
    ("cell", &["index", "column"]),
    ("link", &["url"]),
    ("image", &["url"]),
    ("entry", &["key", "value", "index", "tag"]),
    ("problem", &["kind", "message"]),
    ("directive", &[]),
    ("frontmatter", &[]),
    ("_", &[]),
];

/// The node type relations and file-summing aggregates work over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    /// The unit's node type, such as `skill`.
    pub kind: String,
    /// The key naming a unit, such as `id`.
    pub id_key: String,
    /// The key that qualifies an id as `qualifier:id`, such as `package`.
    pub qualifier_key: Option<String>,
}

/// A node type a caller registers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeSpec {
    /// The type name.
    pub name: String,
    /// Its keys beyond the text keys every type takes.
    pub keys: Vec<String>,
}

/// What a caller adds to the core schema.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SchemaSpec {
    /// New node types.
    pub types: Vec<TypeSpec>,
    /// Keys added to existing types, by type.
    pub keys: BTreeMap<String, BTreeSet<String>>,
    /// The unit, which must be a registered type.
    pub unit: Option<Unit>,
}

/// Why a [`SchemaSpec`] was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SchemaError {
    /// A type is registered twice, or registers a core type.
    #[error("node type {0:?} is already registered")]
    DuplicateType(String),
    /// A key or the unit names a type the schema lacks.
    #[error("unknown node type {0:?}")]
    UnknownType(String),
}

/// The node types and keys a query may name, and the unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    types: BTreeMap<String, BTreeSet<String>>,
    unit: Option<Unit>,
}

impl Schema {
    /// The types every frontend produces, with no unit.
    #[must_use]
    pub fn core() -> Self {
        let types = CORE_TYPES
            .iter()
            .map(|(name, keys)| {
                (
                    (*name).to_owned(),
                    keys.iter().map(|&k| k.to_owned()).collect(),
                )
            })
            .collect();
        Self { types, unit: None }
    }

    /// The core schema extended by `spec`.
    ///
    /// # Errors
    /// Returns [`SchemaError::DuplicateType`] when `spec` registers a type
    /// twice or registers a core type, and [`SchemaError::UnknownType`] when
    /// an added key or the unit names a type the schema lacks.
    pub fn new(spec: SchemaSpec) -> Result<Self, SchemaError> {
        let mut schema = Self::core();
        for kind in spec.types {
            if schema.types.contains_key(&kind.name) {
                return Err(SchemaError::DuplicateType(kind.name));
            }
            schema
                .types
                .insert(kind.name, kind.keys.into_iter().collect());
        }
        for (kind, keys) in spec.keys {
            schema
                .types
                .get_mut(&kind)
                .ok_or_else(|| SchemaError::UnknownType(kind.clone()))?
                .extend(keys);
        }
        if let Some(unit) = &spec.unit
            && !schema.types.contains_key(&unit.kind)
        {
            return Err(SchemaError::UnknownType(unit.kind.clone()));
        }
        schema.unit = spec.unit;
        Ok(schema)
    }

    /// Whether `kind` is a node type of this schema.
    #[must_use]
    pub fn has_type(&self, kind: &str) -> bool {
        self.types.contains_key(kind)
    }

    /// Whether nodes of `kind` take `key`.
    #[must_use]
    pub fn has_key(&self, kind: &str, key: &str) -> bool {
        OPEN_TYPES.contains(&kind)
            || TEXT_KEYS.contains(&key)
            || self.types.get(kind).is_some_and(|keys| keys.contains(key))
    }

    /// Whether `key` compares as a number.
    #[must_use]
    pub fn is_numeric(key: &str) -> bool {
        NUMERIC_KEYS.contains(&key)
    }

    /// The unit, if the caller registered one.
    #[must_use]
    pub fn unit(&self) -> Option<&Unit> {
        self.unit.as_ref()
    }
}

/// A query value: text or a number.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Text.
    Text(String),
    /// A number.
    Number(f64),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) => f.write_str(text),
            Self::Number(n) => write!(f, "{n}"),
        }
    }
}

/// A caller-typed attribute value that compares by its type.
#[derive(Debug, Clone, PartialEq)]
pub enum Attribute {
    /// A truth value; a bare key is true.
    Flag(bool),
    /// A number.
    Amount(f64),
    /// Space-separated elements, any of which may match.
    Members(Vec<String>),
}

/// Link targets resolved to unit nodes, keyed by the link's target.
pub type Targets = BTreeMap<String, NodeId>;

/// Typed attribute values by node, then by attribute key.
pub type Typed = BTreeMap<NodeId, BTreeMap<String, Attribute>>;

/// A data table rules read as `row` nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Table {
    /// Keyed rows in declaration order.
    Map(Vec<(String, String)>),
    /// Indexed rows.
    List(Vec<String>),
}

/// What a query reads besides the tree: everything the caller knows in
/// advance, and the clock.
#[derive(Debug, Clone)]
pub struct Environment {
    /// Link targets resolved to unit nodes.
    pub targets: Targets,
    /// Typed attribute values.
    pub typed: Typed,
    /// Values of `$params`; a lint rule's own params override these.
    pub params: BTreeMap<String, Value>,
    /// Data tables by name.
    pub data: BTreeMap<String, Table>,
    /// The model behind `(similar …)`, if any.
    pub embed: Option<Arc<dyn Embed>>,
    /// The time `(older days)` counts from.
    pub now: SystemTime,
}

impl Environment {
    /// An environment with nothing precomputed, counting from `now`.
    #[must_use]
    pub fn new(now: SystemTime) -> Self {
        Self {
            targets: Targets::new(),
            typed: Typed::new(),
            params: BTreeMap::new(),
            data: BTreeMap::new(),
            embed: None,
            now,
        }
    }
}

/// What kind of mistake a rejected query makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// The source does not follow the grammar.
    Syntax,
    /// A pattern names a node type the schema lacks.
    UnknownType,
    /// A pattern names a key its type does not take.
    UnknownKey,
    /// A variable, capture or param is used before it is bound.
    Unbound,
}

/// A query the reader rejected, at a byte offset of its source.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message} at offset {offset}")]
pub struct Error {
    kind: ErrorKind,
    offset: usize,
    message: String,
    hint: Option<String>,
}

impl Error {
    /// An error of `kind` at `offset`.
    #[must_use]
    pub fn new(kind: ErrorKind, offset: usize, message: impl Into<String>) -> Self {
        Self {
            kind,
            offset,
            message: message.into(),
            hint: None,
        }
    }

    /// The error with a suggested fix.
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// What kind of mistake it is.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// The byte offset in the query source.
    #[must_use]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// What is wrong.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// A suggested fix, if one is known.
    #[must_use]
    pub fn hint(&self) -> Option<&str> {
        self.hint.as_deref()
    }
}

/// A query that failed while matching.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RunError {
    /// A pattern uses `(similar …)` but the environment has no model.
    #[error("(similar …) needs a semantic model")]
    NoModel,
    /// The model failed to embed a text.
    #[error(transparent)]
    Embed(#[from] EmbedError),
}

/// A query read against a schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    source: String,
}

impl Query {
    /// Reads `source` against `schema`, ordering rows by `sort` (`?var` or
    /// `@capture.key`, descending with a leading `-`).
    ///
    /// # Errors
    /// Returns [`Error`] when the source is not a valid query for `schema`.
    #[expect(
        clippy::todo,
        reason = "the reader is ported from henia's pattern package"
    )]
    pub fn read(source: &str, schema: &Schema, sort: Option<&str>) -> Result<Self, Error> {
        let _ = (source, schema, sort);
        todo!()
    }

    /// The source the query was read from.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Matches the query over a shared tree; it never writes to nodes.
    ///
    /// # Errors
    /// Returns [`RunError`] when a similarity pattern has no model or the
    /// model fails.
    #[expect(
        clippy::todo,
        reason = "the matcher is ported from henia's pattern package"
    )]
    pub fn run(&self, tree: &Tree, environment: &Environment) -> Result<Vec<Row>, RunError> {
        let _ = (tree, environment);
        todo!()
    }
}

/// What a capture holds in one row.
#[derive(Debug, Clone, PartialEq)]
pub enum Capture {
    /// One node.
    Node(NodeId),
    /// The nodes of a quantified or grouped pattern.
    Nodes(Vec<NodeId>),
    /// A value, a score or an aggregate.
    Value(Value),
}

/// One match: its captures and variable bindings.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Row {
    /// Captures by name, without the `@`.
    pub captures: BTreeMap<String, Capture>,
    /// Variable bindings by name, without the `?`.
    pub vars: BTreeMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill_spec() -> SchemaSpec {
        SchemaSpec {
            types: vec![TypeSpec {
                name: "skill".to_owned(),
                keys: vec!["id".to_owned(), "package".to_owned()],
            }],
            keys: BTreeMap::from([("file".to_owned(), BTreeSet::from(["file".to_owned()]))]),
            unit: Some(Unit {
                kind: "skill".to_owned(),
                id_key: "id".to_owned(),
                qualifier_key: Some("package".to_owned()),
            }),
        }
    }

    #[test]
    fn core_schema_knows_no_unit_and_no_skill() {
        let core = Schema::core();
        assert!(core.unit().is_none());
        assert!(!core.has_type("skill"));
        assert!(core.has_key("heading", "level"));
        assert!(core.has_key("paragraph", "dynamic"));
        assert!(!core.has_key("link", "exists"));
        assert!(core.has_key("directive", "anything"));
    }

    #[test]
    fn a_caller_registers_the_unit_and_its_keys() {
        let schema = Schema::new(skill_spec()).unwrap();
        assert!(schema.has_key("skill", "package"));
        assert!(schema.has_key("file", "file"));
        assert_eq!(schema.unit().map(|u| u.kind.as_str()), Some("skill"));
    }

    #[test]
    fn registering_a_core_type_again_fails() {
        let spec = SchemaSpec {
            types: vec![TypeSpec {
                name: "heading".to_owned(),
                keys: vec![],
            }],
            ..SchemaSpec::default()
        };
        assert_eq!(
            Schema::new(spec),
            Err(SchemaError::DuplicateType("heading".to_owned()))
        );
    }

    #[test]
    fn keys_and_unit_must_name_known_types() {
        let mut spec = skill_spec();
        spec.types.clear();
        spec.keys.clear();
        assert_eq!(
            Schema::new(spec),
            Err(SchemaError::UnknownType("skill".to_owned()))
        );
    }

    #[test]
    fn values_print_like_henia() {
        assert_eq!(Value::Number(3.0).to_string(), "3");
        assert_eq!(Value::Number(0.25).to_string(), "0.25");
        assert_eq!(Value::Text("x".to_owned()).to_string(), "x");
    }
}
