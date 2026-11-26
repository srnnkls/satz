//! Structural queries in tq: the schema, environment, and the query
//! and row types its reader and matcher exchange.
//!
//! `:matches` and `/regexp/` values compile with the [`regex`] crate. The
//! syntax is RE2's with these differences:
//!
//! - `\d`, `\w`, `\s` and `\b` are Unicode-aware: `\d` matches `٣` and `\w`
//!   matches `é`; write `[0-9]` or `(?-u:\w)` for RE2's ASCII classes.
//! - Inside a class, `&&`, `--` and `~~` are intersection, difference and
//!   symmetric difference: `[a-z&&[^aeiou]]` is a consonant, where RE2 reads
//!   the class literally.
//! - `\<` and `\>` are start- and end-of-word boundaries; RE2 rejects them.
//! - `\Q…\E` quoting and `\C` are rejected; RE2 accepts them.

mod check;
mod define;
mod group;
mod lex;
mod matcher;
mod pattern;
mod read;
mod rows;
mod sort;
mod view;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;
use std::time::SystemTime;

pub(crate) use self::define::Defines;
pub(crate) use self::lex::{Kind, Token};
pub(crate) use self::matcher::lines;
pub(crate) use self::read::{Reader, suggest, unexpected};

use self::group::Group;
use self::pattern::{Check, Operand, Pattern, Relate};
use self::sort::SortKey;
use crate::similarity::{Embed, EmbedError, EmbeddingError};
use crate::tree::{NodeId, Tree};

/// The node type of a root that holds files: patterns never match it.
pub const CORPUS: &str = "corpus";

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
#[non_exhaustive]
pub struct Unit {
    /// The unit's node type, such as `skill`.
    pub kind: String,
    /// The key naming a unit, such as `id`.
    pub id_key: String,
    /// The key that qualifies an id as `qualifier:id`, such as `package`.
    pub qualifier_key: Option<String>,
}

impl Unit {
    /// Units of node type `kind`, named by their `id_key`.
    #[must_use]
    pub fn new(kind: impl Into<String>, id_key: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            id_key: id_key.into(),
            qualifier_key: None,
        }
    }

    /// The unit with ids qualified by `key`.
    #[must_use]
    pub fn with_qualifier_key(mut self, key: impl Into<String>) -> Self {
        self.qualifier_key = Some(key.into());
        self
    }
}

/// A node type a caller registers.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TypeSpec {
    /// The type name.
    pub name: String,
    /// Its keys beyond the text keys every type takes.
    pub keys: Vec<String>,
}

impl TypeSpec {
    /// The node type `name` taking `keys`.
    #[must_use]
    pub fn new(name: impl Into<String>, keys: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            name: name.into(),
            keys: keys.into_iter().map(Into::into).collect(),
        }
    }
}

/// What a caller adds to the core schema.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
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
#[non_exhaustive]
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

impl Default for Schema {
    fn default() -> Self {
        Self::core()
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

impl Table {
    /// Whether the table is keyed or indexed.
    #[must_use]
    pub fn kind(&self) -> TableKind {
        match self {
            Self::Map(_) => TableKind::Map,
            Self::List(_) => TableKind::List,
        }
    }
}

/// The shape of a [`Table`]. It displays as the phrase `a table` or
/// `a list`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TableKind {
    /// Keyed rows.
    Map,
    /// Indexed rows.
    List,
}

impl fmt::Display for TableKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Map => "a table",
            Self::List => "a list",
        })
    }
}

/// How a query reads a `$param`. It displays as the phrase `text` or
/// `a number`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamKind {
    /// As text: matched or printed.
    Text,
    /// As a number: compared, or a similarity threshold or limit.
    Number,
}

impl fmt::Display for ParamKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Text => "text",
            Self::Number => "a number",
        })
    }
}

/// What a query reads besides the tree: everything the caller knows in
/// advance, and the clock.
#[derive(Debug, Clone)]
#[non_exhaustive]
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
pub enum ReadErrorKind {
    /// The source does not follow the grammar.
    Syntax,
    /// A pattern names a node type the schema lacks.
    UnknownType,
    /// A pattern names a key its type does not take.
    UnknownKey,
    /// A variable, capture or param is used before it is bound.
    Unbound,
    /// A relation is used, but the schema registers no unit.
    NoUnit,
}

/// A query the reader rejected, at a byte offset of its source.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message} at offset {offset}")]
pub struct ReadError {
    kind: ReadErrorKind,
    offset: usize,
    message: String,
    hint: Option<String>,
}

impl ReadError {
    /// An error of `kind` at `offset`.
    #[must_use]
    pub(crate) fn new(kind: ReadErrorKind, offset: usize, message: impl Into<String>) -> Self {
        Self {
            kind,
            offset,
            message: message.into(),
            hint: None,
        }
    }

    pub(crate) fn syntax(offset: usize, message: impl Into<String>) -> Self {
        Self::new(ReadErrorKind::Syntax, offset, message)
    }

    /// The error moved to `offset`, as when its source is a slice of a
    /// larger text.
    pub(crate) fn at(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }

    /// The error with a suggested fix.
    #[must_use]
    pub(crate) fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// What kind of mistake it is.
    #[must_use]
    pub fn kind(&self) -> ReadErrorKind {
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
    /// The model returned an embedding with a non-finite component.
    #[error("the semantic model returned an unusable embedding")]
    Embedding(#[from] EmbeddingError),
    /// The query reads a `$param` the environment does not set.
    #[error("${0} has no value")]
    MissingParam(String),
    /// The query compares a `$param` as a number, but its value is text.
    #[error("${name} is {value:?}, but the query compares it as a number")]
    TextParam {
        /// The param.
        name: String,
        /// Its text value.
        value: String,
    },
    /// Joining the patterns produced more combinations than a run allows.
    #[error("the query produces too many combinations; anchor it to a narrower pattern")]
    TooMany,
}

/// A query read against a schema.
///
/// A query is immutable and shareable: [`Query::run`] keeps document order,
/// memos and caches in per-run state and never writes to the tree.
#[derive(Debug, Clone)]
pub struct Query {
    source: String,
    members: Vec<Pattern>,
    absent: Vec<Pattern>,
    captures: Vec<String>,
    values: BTreeSet<String>,
    numbers: BTreeSet<String>,
    semantic: bool,
    group: Option<Group>,
    params: BTreeMap<String, ParamKind>,
    sorts: Vec<SortKey>,
    printed: Vec<String>,
    unit: Option<Unit>,
}

impl Query {
    /// Reads `source` against `schema`, ordering rows by `sort` (`?var`,
    /// `@value` or `@capture.key`, descending with a leading `-`).
    ///
    /// Regular expressions follow the [`regex`] crate, which differs from
    /// RE2 as the [module docs](crate::query) list.
    ///
    /// # Errors
    /// Returns [`ReadError`] when the source is not a valid query for `schema`;
    /// an invalid `sort` is reported at the offset just past the source.
    pub fn read(source: &str, schema: &Schema, sort: Option<&str>) -> Result<Self, ReadError> {
        let mut reader = Reader::new(source, schema)?;
        if let Some(sort) = sort {
            reader.sorts.push(SortKey::parse(sort, source.len())?);
        }
        let mut query = reader.query(Kind::Eof)?;
        source.clone_into(&mut query.source);
        Ok(query)
    }

    /// The source the query was read from.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The capture names in order of first appearance; a query of one
    /// pattern without captures has the implicit capture `""`.
    #[must_use]
    pub fn captures(&self) -> &[String] {
        &self.captures
    }

    /// The variables a listing shows with each row: the group's key
    /// variables, or else the sort variables.
    #[must_use]
    pub fn shown_vars(&self) -> Vec<&str> {
        match &self.group {
            Some(group) => group.keys.iter().filter_map(|k| k.input.var()).collect(),
            None => self.sorts.iter().filter_map(SortKey::var).collect(),
        }
    }

    /// Whether the query compares texts under a semantic model, through
    /// `(similar …)`.
    #[must_use]
    pub fn is_semantic(&self) -> bool {
        self.semantic
    }

    /// The `$params` the query reads, and how it reads each.
    #[must_use]
    pub fn params(&self) -> &BTreeMap<String, ParamKind> {
        &self.params
    }

    pub(crate) fn set_source(&mut self, source: &str) {
        source.clone_into(&mut self.source);
    }

    /// Whether `capture` captures a value rather than nodes.
    pub(crate) fn is_value(&self, capture: &str) -> bool {
        self.values.contains(capture)
    }

    /// Whether a required pattern cannot match in `environment`: a
    /// similarity threshold it needs is 0 or below.
    pub(crate) fn impossible(&self, environment: &Environment) -> bool {
        fn walk(pattern: &Pattern, environment: &Environment) -> bool {
            let disabled = pattern.attrs.iter().any(|attr| match &attr.check {
                Check::Constrain {
                    relate: Relate::Similarity(similarity),
                    ..
                } => environment.number(&similarity.threshold) <= 0.0,
                _ => false,
            });
            if disabled {
                return true;
            }
            if pattern.alts.iter().any(|alt| !walk(alt, environment)) {
                return false;
            }
            let required_link = pattern
                .chains
                .iter()
                .flat_map(|chain| &chain.links)
                .any(|link| !link.pattern.optional() && walk(&link.pattern, environment));
            required_link || !pattern.alts.is_empty()
        }
        self.members
            .iter()
            .any(|p| !p.optional() && walk(p, environment))
    }

    /// Matches the query over a shared tree; it never writes to nodes.
    ///
    /// Data tables join the tree as `row` nodes after its last node, under a
    /// [`CORPUS`] root; nodes of that type never match.
    ///
    /// # Errors
    /// Returns [`RunError`] when a similarity pattern has no model or the
    /// model fails, a `$param` is unset or text where a number is due, or the
    /// patterns join into too many combinations.
    pub fn run(&self, tree: &Tree, environment: &Environment) -> Result<Vec<Row>, RunError> {
        if self.semantic && environment.embed.is_none() {
            return Err(RunError::NoModel);
        }
        for (name, &kind) in &self.params {
            match environment.params.get(name) {
                None => return Err(RunError::MissingParam(name.clone())),
                Some(Value::Text(value)) if kind == ParamKind::Number => {
                    return Err(RunError::TextParam {
                        name: name.clone(),
                        value: value.clone(),
                    });
                }
                Some(_) => {}
            }
        }
        let data = view::data_tree(&environment.data);
        let view = view::View::new(tree, data.as_ref());
        let mut matcher = matcher::Matcher::new(self, environment, view);
        let bindings = match (self.members.as_slice(), self.absent.is_empty()) {
            ([member], true) => matcher.everywhere(member, self.captures[0].is_empty()),
            _ => matcher.join(),
        }?;
        if let Some(failure) = matcher.failure.take() {
            return Err(failure);
        }
        let rows = match &self.group {
            Some(group) => matcher.group_rows(group, &bindings),
            None => matcher.rows(bindings),
        };
        Ok(matcher
            .sort(rows)
            .into_iter()
            .map(|row| matcher.publish(row))
            .collect())
    }
}

impl Environment {
    /// The number `operand` stands for; a param that is unset or text is 0.
    fn number(&self, operand: &Operand) -> f64 {
        match operand {
            Operand::Number(n) => *n,
            Operand::Param(name) => match self.params.get(name) {
                Some(Value::Number(n)) => *n,
                _ => 0.0,
            },
        }
    }
}

/// A node a capture holds: a node of the tree or a data table row.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Hit {
    /// A node of the queried tree.
    Node(NodeId),
    /// A row of an environment data table, by its attributes: `table`,
    /// `value`, and `key` for a keyed table or `index` for a list.
    Row(BTreeMap<String, String>),
}

/// What a capture holds in one row; captures that bound nothing are absent
/// from the row.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Capture {
    /// The nodes of one match: one, or several under `*` or `+`.
    Nodes(Vec<Hit>),
    /// The distinct nodes a group collected from its matches, in document
    /// order.
    Collected(Vec<Hit>),
    /// A value of one match, a score, or an aggregate.
    Value(Value),
    /// The distinct values a group collected from its matches.
    Values(Vec<Value>),
}

/// One match: its captures and variable bindings.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct Row {
    /// Captures by name, without the `@`.
    pub captures: BTreeMap<String, Capture>,
    /// Variable bindings by name, without the `?`; a group row binds only
    /// its key variables.
    pub vars: BTreeMap<String, String>,
}

impl Row {
    /// The value a capture holds, if it holds one value.
    #[must_use]
    pub fn value(&self, name: &str) -> Option<&Value> {
        match self.captures.get(name)? {
            Capture::Value(value) => Some(value),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
