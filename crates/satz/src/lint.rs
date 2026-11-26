//! The lint engine: rule modules from ordered layers, compiled into a plan
//! that runs over one corpus and returns satz's own findings.
//!
//! A module is a Markdown file: its frontmatter sets `params` and `data`
//! for its rules, its `sq` code blocks hold `(rule …)`, `(define …)` and
//! `(import …)` forms, and `md` code blocks under a rule's `### Matches` or
//! `### Passes` heading are the rule's examples. satz's std modules are
//! `std/duplicates`, `std/similarity`, `std/structure` and `std/links`.

mod compile;
mod header;
mod load;
pub(crate) mod module;
mod run;

use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use self::compile::Compiled;
use crate::edit::Change;
use crate::query::{
    self, CORPUS, Environment, ParamKind, Query, ReadError, Schema, Table, TableKind, Value,
};
use crate::tree::{Node, Origin, Tree, TreeError};

/// A rule id: non-empty, without whitespace, such as `duplicate-heading`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleId(String);

/// A rule id that is empty or contains whitespace.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid rule id {0:?}")]
pub struct InvalidRuleId(pub String);

impl RuleId {
    /// Validates `id`.
    ///
    /// # Errors
    /// Returns [`InvalidRuleId`] when `id` is empty or contains whitespace.
    pub fn new(id: impl Into<String>) -> Result<Self, InvalidRuleId> {
        let id = id.into();
        if id.is_empty() || id.contains(char::is_whitespace) {
            return Err(InvalidRuleId(id));
        }
        Ok(Self(id))
    }

    /// The id as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for RuleId {
    type Err = InvalidRuleId;

    fn from_str(id: &str) -> Result<Self, Self::Err> {
        Self::new(id)
    }
}

impl fmt::Display for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for RuleId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for RuleId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

/// How a finding counts: errors always fail lint, warnings only when the
/// caller is strict. Severities order by weight, a warning below an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Fails only when strict.
    Warning,
    /// Always fails.
    Error,
}

/// A severity name other than `error` or `warning`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown severity {0:?}; expected error or warning")]
pub struct UnknownSeverity(pub String);

impl FromStr for Severity {
    type Err = UnknownSeverity;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "error" => Ok(Self::Error),
            "warning" => Ok(Self::Warning),
            other => Err(UnknownSeverity(other.to_owned())),
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Error => "error",
            Self::Warning => "warning",
        })
    }
}

/// The similarity behind a finding, with what its method reports.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Score {
    /// Word-shingle Jaccard, with the shared phrases.
    Jaccard {
        /// The score.
        value: f64,
        /// Shared phrases.
        shared: Vec<String>,
    },
    /// Word-shingle containment, with the shared phrases.
    Containment {
        /// The score.
        value: f64,
        /// Shared phrases.
        shared: Vec<String>,
    },
    /// Cosine under a semantic model.
    Cosine {
        /// The score.
        value: f64,
        /// The model.
        model: String,
    },
}

impl Score {
    /// The score, whatever the method.
    #[must_use]
    pub fn value(&self) -> f64 {
        match self {
            Self::Jaccard { value, .. }
            | Self::Containment { value, .. }
            | Self::Cosine { value, .. } => *value,
        }
    }
}

/// One finding.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    rule: RuleId,
    origin: Origin,
    severity: Severity,
    message: String,
    related: Vec<Origin>,
    score: Option<Score>,
    fixes: Vec<Change>,
}

impl Diagnostic {
    /// A finding of `rule` at `origin`.
    #[must_use]
    pub fn new(
        rule: RuleId,
        origin: Origin,
        severity: Severity,
        message: impl Into<String>,
    ) -> Self {
        Self {
            rule,
            origin,
            severity,
            message: message.into(),
            related: Vec::new(),
            score: None,
            fixes: Vec::new(),
        }
    }

    /// The finding with `related` origins.
    #[must_use]
    pub fn with_related(mut self, related: Vec<Origin>) -> Self {
        self.related = related;
        self
    }

    /// The finding with a similarity score.
    #[must_use]
    pub fn with_score(mut self, score: Score) -> Self {
        self.score = Some(score);
        self
    }

    /// The rule.
    #[must_use]
    pub fn rule(&self) -> &RuleId {
        &self.rule
    }

    /// Where the finding points.
    #[must_use]
    pub fn origin(&self) -> &Origin {
        &self.origin
    }

    /// Its severity.
    #[must_use]
    pub fn severity(&self) -> Severity {
        self.severity
    }

    /// Its message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Related origins, such as the original of a copy.
    #[must_use]
    pub fn related(&self) -> &[Origin] {
        &self.related
    }

    /// The similarity behind it, if any.
    #[must_use]
    pub fn score(&self) -> Option<&Score> {
        self.score.as_ref()
    }

    /// The finding with the changes that fix it.
    #[must_use]
    pub fn with_fixes(mut self, fixes: Vec<Change>) -> Self {
        self.fixes = fixes;
        self
    }

    /// The changes the rule's edits make to fix it.
    #[must_use]
    pub fn fixes(&self) -> &[Change] {
        &self.fixes
    }
}

/// A module file of a layer.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ModuleFile {
    /// The path within the layer, which names the module.
    pub path: String,
    /// The Markdown source.
    pub text: String,
}

impl ModuleFile {
    /// The module at `path` within its layer, with Markdown source `text`.
    #[must_use]
    pub fn new(path: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            text: text.into(),
        }
    }
}

/// A set of rule modules, optionally named as a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    package: Option<String>,
    files: Vec<ModuleFile>,
}

/// Why a layer could not be built.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LayerError {
    /// Two files share a path.
    #[error("module file {0:?} appears twice")]
    DuplicateFile(String),
    /// A directory or file could not be read.
    #[error("read {path:?}")]
    Read {
        /// The path that failed.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
}

impl Layer {
    /// A layer of `files`, named `package` when given.
    ///
    /// # Errors
    /// Returns [`LayerError::DuplicateFile`] when two files share a path.
    pub fn from_files(package: Option<String>, files: Vec<ModuleFile>) -> Result<Self, LayerError> {
        let mut seen = BTreeSet::new();
        for file in &files {
            if !seen.insert(file.path.as_str()) {
                return Err(LayerError::DuplicateFile(file.path.clone()));
            }
        }
        Ok(Self { package, files })
    }

    /// A layer of the Markdown files under `dir`, with paths relative to it.
    ///
    /// # Errors
    /// Returns [`LayerError::Read`] when the directory or a file cannot be
    /// read.
    pub fn read_dir(package: Option<String>, dir: impl AsRef<Path>) -> Result<Self, LayerError> {
        let dir = dir.as_ref();
        let mut files = Vec::new();
        collect_markdown(dir, dir, &mut files)?;
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Self::from_files(package, files)
    }

    /// The package name, if any.
    #[must_use]
    pub fn package(&self) -> Option<&str> {
        self.package.as_deref()
    }

    /// The module files.
    #[must_use]
    pub fn files(&self) -> &[ModuleFile] {
        &self.files
    }
}

fn collect_markdown(
    root: &Path,
    dir: &Path,
    files: &mut Vec<ModuleFile>,
) -> Result<(), LayerError> {
    let failed = |path: &Path, source| LayerError::Read {
        path: path.to_owned(),
        source,
    };
    for entry in std::fs::read_dir(dir).map_err(|e| failed(dir, e))? {
        let path = entry.map_err(|e| failed(dir, e))?.path();
        if path.is_dir() {
            collect_markdown(root, &path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "md") {
            let text = std::fs::read_to_string(&path).map_err(|e| failed(&path, e))?;
            let relative = path.strip_prefix(root).unwrap_or(&path);
            files.push(ModuleFile::new(
                relative.to_string_lossy().replace('\\', "/"),
                text,
            ));
        }
    }
    Ok(())
}

/// A rule declared in config rather than in a module.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct InlineRule {
    /// The rule id.
    pub id: RuleId,
    /// The sq query.
    pub query: String,
    /// The message template.
    pub message: String,
    /// The severity, warning when absent.
    pub severity: Option<Severity>,
    /// The capture the finding points at.
    pub at: Option<String>,
    /// The capture of related locations.
    pub related: Option<String>,
    /// Parameter values, over the environment's.
    pub params: BTreeMap<String, Value>,
    /// Data tables, over the environment's.
    pub data: BTreeMap<String, Table>,
}

impl InlineRule {
    /// The rule `id` reporting each row of `query` with `message`, a
    /// warning at its first capture.
    #[must_use]
    pub fn new(id: RuleId, query: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id,
            query: query.into(),
            message: message.into(),
            severity: None,
            at: None,
            related: None,
            params: BTreeMap::new(),
            data: BTreeMap::new(),
        }
    }
}

/// Per-rule overrides from config.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct RuleConfig {
    /// A replacement severity.
    pub severity: Option<Severity>,
    /// Parameter overrides.
    pub params: BTreeMap<String, Value>,
    /// Data table overrides.
    pub data: BTreeMap<String, Table>,
}

/// Names the caller gives satz's sources.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Labels {
    /// The source name of inline rules.
    pub inline: String,
    /// The prefix of std module names.
    pub std: String,
}

impl Default for Labels {
    fn default() -> Self {
        Self {
            inline: "inline".to_owned(),
            std: "std".to_owned(),
        }
    }
}

/// What the calling program declares: the schema its trees follow, its
/// extra module frontmatter keys and its labels.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Spec {
    /// The schema rule queries are read against.
    pub schema: Schema,
    /// Module frontmatter keys beyond satz's own, returned raw per rule.
    pub meta_keys: BTreeSet<String>,
    /// Source labels.
    pub labels: Labels,
}

/// What a user configures: rules, overrides and what to disable.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Settings {
    /// Inline rules.
    pub rules: Vec<InlineRule>,
    /// Per-rule overrides.
    pub config: BTreeMap<RuleId, RuleConfig>,
    /// Disabled rules.
    pub disable: BTreeSet<RuleId>,
    /// The frontmatter key a file disables rules with: one rule id, a
    /// comma-separated list of them, or a YAML list.
    pub disable_key: String,
    /// Whether rules comparing texts under a semantic model run; the
    /// caller loads the model into [`Environment::embed`].
    pub semantic: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            config: BTreeMap::new(),
            disable: BTreeSet::new(),
            disable_key: "satz.lint.disable".to_owned(),
            semantic: false,
        }
    }
}

/// Why a configuration was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// Two layers carry one package name.
    #[error("package {0:?} appears in two layers")]
    DuplicatePackage(String),
    /// Two inline rules share an id.
    #[error("inline rule {0} is declared twice")]
    DuplicateRule(RuleId),
    /// The per-file disable key is empty.
    #[error("disable_key is empty")]
    EmptyDisableKey,
}

/// A validated lint configuration: layers in order, then the caller's spec
/// and the user's settings.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    layers: Vec<Layer>,
    spec: Spec,
    settings: Settings,
}

impl Config {
    /// Validates `layers`, `spec` and `settings` together.
    ///
    /// # Errors
    /// Returns [`ConfigError::DuplicatePackage`] when two layers carry one
    /// package name, [`ConfigError::DuplicateRule`] when two inline rules share
    /// an id, and [`ConfigError::EmptyDisableKey`] for an empty disable key.
    pub fn new(layers: Vec<Layer>, spec: Spec, settings: Settings) -> Result<Self, ConfigError> {
        let mut packages = BTreeSet::new();
        for package in layers.iter().filter_map(Layer::package) {
            if !packages.insert(package) {
                return Err(ConfigError::DuplicatePackage(package.to_owned()));
            }
        }
        let mut ids = BTreeSet::new();
        for rule in &settings.rules {
            if !ids.insert(&rule.id) {
                return Err(ConfigError::DuplicateRule(rule.id.clone()));
            }
        }
        if settings.disable_key.is_empty() {
            return Err(ConfigError::EmptyDisableKey);
        }
        Ok(Self {
            layers,
            spec,
            settings,
        })
    }

    /// The layers, in load order.
    #[must_use]
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// The caller's spec.
    #[must_use]
    pub fn spec(&self) -> &Spec {
        &self.spec
    }

    /// The user's settings.
    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Reads the query `source` against the spec's schema, as
    /// [`Query::read`] does; its `(import …)` forms name the std modules and
    /// those of the layers.
    ///
    /// # Errors
    /// Returns [`ReadError`] when the source is not a valid query, or an
    /// import names a module that is missing or does not compile.
    pub fn query(&self, source: &str, sort: Option<&str>) -> Result<Query, ReadError> {
        let mut loader = load::Loader::new(&self.layers, &self.spec);
        let importer = move |name: &str| match &mut loader {
            Ok(loader) => loader.defines(name),
            Err(error) => Err(error.to_string()),
        };
        Query::read_importing(source, &self.spec.schema, sort, Some(Box::new(importer)))
    }
}

/// A compiled rule's identity and declared metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RuleMeta {
    /// The rule id.
    pub id: RuleId,
    /// The module that defines it.
    pub module: String,
    /// Its effective severity.
    pub severity: Severity,
    /// Raw values of the caller's declared meta keys: a key's scalar value,
    /// or its entry for this rule when the module sets a table by rule id.
    pub meta: BTreeMap<String, String>,
}

/// Whether an example must produce findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect {
    /// The rule must fire.
    Matches,
    /// The rule must stay silent.
    Passes,
}

/// A rule's documented example.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Example {
    /// The rule.
    pub rule: RuleId,
    /// The module that defines it.
    pub module: String,
    /// The example's line in the module.
    pub line: u32,
    /// What the example expects.
    pub expect: Expect,
    /// The example's Markdown.
    pub text: String,
    /// Params the example sets, typed as the rule declares them.
    pub params: BTreeMap<String, Value>,
    /// Data tables the example sets.
    pub data: BTreeMap<String, Table>,
}

/// The result of running one example.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Outcome {
    /// The example held.
    Pass,
    /// The rule reported findings on a `Passes` example, or none on a
    /// `Matches` one.
    Fail {
        /// Findings produced.
        found: usize,
    },
}

/// Which rules a plan runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// Every rule.
    All,
    /// Only these rules.
    Only(BTreeSet<RuleId>),
}

impl Selection {
    /// Whether the rule `id` runs.
    #[must_use]
    pub fn includes(&self, id: &str) -> bool {
        match self {
            Self::All => true,
            Self::Only(ids) => ids.contains(id),
        }
    }
}

/// The tree and environment a plan runs over; the caller assembles both.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Corpus {
    /// The files, each a file node under the root.
    pub tree: Tree,
    /// Targets, typed values, params, data, model and clock.
    pub environment: Environment,
}

impl Corpus {
    /// The frontends' `files` grafted in order under a [`CORPUS`] root.
    ///
    /// # Errors
    /// Returns [`TreeError::Full`] when the files hold more nodes than a
    /// tree can.
    pub fn new(
        files: impl IntoIterator<Item = Tree>,
        environment: Environment,
    ) -> Result<Self, TreeError> {
        let root = Origin::named(&Arc::from(CORPUS), None);
        let mut tree = Tree::new(Node::new(CORPUS, root));
        let id = tree.root().id();
        for file in files {
            tree.graft(id, file)?;
        }
        Ok(Self { tree, environment })
    }
}

/// Why compiling failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CompileError {
    /// Two layers define one module name.
    #[error("module {0:?} is defined twice")]
    DuplicateModule(String),
    /// A module is not valid Markdown with YAML frontmatter, or a
    /// frontmatter value has the wrong shape.
    #[error("module {module:?}: {message}")]
    Malformed {
        /// The module.
        module: String,
        /// What is wrong.
        message: String,
    },
    /// A module's frontmatter has a key neither satz nor the caller declares.
    #[error("module {module:?}: unknown frontmatter key {key:?}")]
    UnknownMetaKey {
        /// The module.
        module: String,
        /// The key.
        key: String,
    },
    /// A module's `sq` blocks did not read; the error's offset is within
    /// the block holding it.
    #[error("module {module:?}, line {line}")]
    Read {
        /// The module.
        module: String,
        /// The module line of the error.
        line: u32,
        /// The reader's error.
        source: ReadError,
    },
    /// An inline rule did not read; the error's offset is within its query.
    #[error("module {module:?}, rule {rule}")]
    Query {
        /// The inline rules' source label.
        module: String,
        /// The rule.
        rule: RuleId,
        /// The reader's error.
        source: ReadError,
    },
    /// A rule reports a `:score` without a known `:method`.
    #[error("module {module:?}, rule {rule}: :score needs :method jaccard, containment or cosine")]
    Method {
        /// The module.
        module: String,
        /// The rule.
        rule: RuleId,
    },
    /// Two modules, or a module and the inline rules, define one rule id.
    #[error("rule {rule} is defined in {first} and {second}")]
    DuplicateRule {
        /// The rule.
        rule: RuleId,
        /// The module defining it first.
        first: String,
        /// The module defining it again.
        second: String,
    },
    /// Settings configure or disable a rule no module defines.
    #[error("no rule {rule}; {hint}")]
    UnknownRule {
        /// The rule.
        rule: RuleId,
        /// The nearest rule ids.
        hint: String,
    },
    /// Settings or an example set a param or data table the rule lacks.
    #[error("rule {rule} has no param or data {key:?}; {hint}")]
    UnknownSetting {
        /// The rule.
        rule: RuleId,
        /// The param or table.
        key: String,
        /// The nearest names.
        hint: String,
    },
    /// A param's value does not have the type the rule gives it.
    #[error("rule {rule}: param {param} must be {expected}")]
    ParamType {
        /// The rule.
        rule: RuleId,
        /// The param.
        param: String,
        /// The type it must have.
        expected: ParamKind,
    },
    /// A data table does not have the shape the rule gives it.
    #[error("rule {rule}: data {table} must be {expected}")]
    TableShape {
        /// The rule.
        rule: RuleId,
        /// The table.
        table: String,
        /// The shape it must have.
        expected: TableKind,
    },
}

/// Why running a plan or an example failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RunError {
    /// A rule's query failed.
    #[error("rule {rule} failed")]
    Query {
        /// The rule.
        rule: RuleId,
        /// The query's failure.
        source: query::RunError,
    },
    /// An example names a rule the plan lacks.
    #[error("no rule {0}")]
    UnknownRule(RuleId),
}

/// Rules compiled from a configuration.
#[derive(Debug, Clone)]
pub struct Plan {
    rules: Vec<RuleMeta>,
    examples: Vec<Example>,
    compiled: Vec<Compiled>,
    disabled: BTreeSet<RuleId>,
    disable_key: String,
    semantic: bool,
}

impl Plan {
    /// The compiled rules, disabled ones included.
    #[must_use]
    pub fn rules(&self) -> &[RuleMeta] {
        &self.rules
    }

    /// Every rule's examples.
    #[must_use]
    pub fn examples(&self) -> &[Example] {
        &self.examples
    }
}

#[cfg(test)]
mod tests;
