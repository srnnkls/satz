//! The lint engine: rule modules from ordered layers, compiled into a plan
//! that runs over one corpus and returns tela's own findings.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::SystemTime;

use crate::query::{self, Environment, Table, Value};
use crate::tree::Tree;

/// How a finding counts: errors fail lint, warnings only under `--strict`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Fails lint.
    Error,
    /// Fails lint only when strict.
    Warning,
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

/// A source position a finding points at.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Location {
    /// The file, as the corpus names it.
    pub file: String,
    /// The 1-based line.
    pub line: u32,
    /// The 1-based column.
    pub column: u32,
}

/// How a similarity score was computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Word-shingle Jaccard.
    Jaccard,
    /// Word-shingle containment.
    Containment,
    /// Cosine under the semantic model.
    Cosine,
}

/// The similarity behind a finding.
#[derive(Debug, Clone, PartialEq)]
pub struct Score {
    /// The score.
    pub value: f64,
    /// How it was computed.
    pub method: Method,
    /// Shared phrases, for shingle methods.
    pub shared: Vec<String>,
    /// The model, for cosine.
    pub model: Option<String>,
}

/// One finding.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    rule: String,
    location: Location,
    severity: Severity,
    message: String,
    related: Vec<Location>,
    score: Option<Score>,
}

/// A diagnostic without a rule id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("diagnostic names no rule")]
pub struct UnnamedRule;

impl Diagnostic {
    /// A finding of `rule` at `location`.
    ///
    /// # Errors
    /// Returns [`UnnamedRule`] when `rule` is empty.
    pub fn new(
        rule: impl Into<String>,
        location: Location,
        severity: Severity,
        message: impl Into<String>,
    ) -> Result<Self, UnnamedRule> {
        let rule = rule.into();
        if rule.is_empty() {
            return Err(UnnamedRule);
        }
        Ok(Self {
            rule,
            location,
            severity,
            message: message.into(),
            related: Vec::new(),
            score: None,
        })
    }

    /// The finding with `related` locations.
    #[must_use]
    pub fn with_related(mut self, related: Vec<Location>) -> Self {
        self.related = related;
        self
    }

    /// The finding with a similarity score.
    #[must_use]
    pub fn with_score(mut self, score: Score) -> Self {
        self.score = Some(score);
        self
    }

    /// The rule id.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// Where the finding points.
    #[must_use]
    pub fn location(&self) -> &Location {
        &self.location
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

    /// Related locations, such as the original of a copy.
    #[must_use]
    pub fn related(&self) -> &[Location] {
        &self.related
    }

    /// The similarity behind it, if any.
    #[must_use]
    pub fn score(&self) -> Option<&Score> {
        self.score.as_ref()
    }
}

/// A module file of a layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleFile {
    /// The path within the layer, which names the module.
    pub path: String,
    /// The Markdown source.
    pub text: String,
}

/// A set of rule modules, optionally named as a package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layer {
    package: Option<String>,
    files: Vec<ModuleFile>,
}

/// Why a layer could not be built.
#[derive(Debug, thiserror::Error)]
pub enum LayerError {
    /// Two files share a path.
    #[error("module file {0:?} appears twice")]
    DuplicateFile(String),
    /// A directory could not be read.
    #[error("read module directory {path:?}: {source}")]
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
    pub fn read_dir(package: Option<String>, dir: &Path) -> Result<Self, LayerError> {
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
            files.push(ModuleFile {
                path: relative.to_string_lossy().replace('\\', "/"),
                text,
            });
        }
    }
    Ok(())
}

/// A rule declared in config rather than in a module.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineRule {
    /// The rule id.
    pub id: String,
    /// The tq query.
    pub query: String,
    /// The message template.
    pub message: String,
    /// The severity, warning when absent.
    pub severity: Option<Severity>,
    /// The capture the finding points at.
    pub at: Option<String>,
    /// The capture of related locations.
    pub related: Option<String>,
    /// Parameter values.
    pub params: BTreeMap<String, Value>,
    /// Data tables.
    pub data: BTreeMap<String, Table>,
}

/// Per-rule overrides from config.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuleConfig {
    /// A replacement severity.
    pub severity: Option<Severity>,
    /// Parameter overrides.
    pub params: BTreeMap<String, Value>,
    /// Data table overrides.
    pub data: BTreeMap<String, Table>,
}

/// The semantic model options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Semantic {
    /// Whether semantic rules run.
    pub enabled: bool,
    /// The `Model2Vec` directory.
    pub model_path: Option<PathBuf>,
}

/// Names the caller gives tela's sources and messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Labels {
    /// The source name of inline rules.
    pub inline: String,
    /// The prefix of std module names.
    pub std: String,
    /// The prefix of error messages.
    pub error: String,
}

impl Default for Labels {
    fn default() -> Self {
        Self {
            inline: "tela.toml".to_owned(),
            std: "std".to_owned(),
            error: "tela".to_owned(),
        }
    }
}

/// What the calling program declares: its extra module frontmatter keys, its
/// labels, its data tables and its clock.
#[derive(Debug, Clone, PartialEq)]
pub struct Spec {
    /// Module frontmatter keys beyond tela's own, returned raw per rule.
    pub meta_keys: BTreeSet<String>,
    /// Source and message labels.
    pub labels: Labels,
    /// Data tables every rule reads.
    pub data: BTreeMap<String, Table>,
    /// The time rules count from.
    pub now: SystemTime,
}

impl Spec {
    /// A spec with no extra keys or data, tela's labels and `now`.
    #[must_use]
    pub fn new(now: SystemTime) -> Self {
        Self {
            meta_keys: BTreeSet::new(),
            labels: Labels::default(),
            data: BTreeMap::new(),
            now,
        }
    }
}

/// What a user configures: the `[lint]` table.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Inline rules.
    pub rules: Vec<InlineRule>,
    /// Per-rule overrides by id.
    pub config: BTreeMap<String, RuleConfig>,
    /// Disabled rule ids.
    pub disable: BTreeSet<String>,
    /// The frontmatter key a file disables rules with.
    pub disable_key: String,
    /// Semantic model options.
    pub semantic: Semantic,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            config: BTreeMap::new(),
            disable: BTreeSet::new(),
            disable_key: "tela.lint.disable".to_owned(),
            semantic: Semantic::default(),
        }
    }
}

/// Why a configuration was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    /// Two layers carry one package name.
    #[error("package {0:?} appears in two layers")]
    DuplicatePackage(String),
    /// Two inline rules share an id.
    #[error("inline rule {0:?} is declared twice")]
    DuplicateRule(String),
    /// The per-file disable key is empty.
    #[error("disable_key is empty")]
    EmptyDisableKey,
    /// Semantic rules are enabled without a model.
    #[error("semantic rules are enabled but no model_path is set")]
    MissingModel,
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
    /// an id, [`ConfigError::EmptyDisableKey`] for an empty disable key, and
    /// [`ConfigError::MissingModel`] when semantic rules are enabled without a
    /// model path.
    pub fn new(layers: Vec<Layer>, spec: Spec, settings: Settings) -> Result<Self, ConfigError> {
        let mut packages = BTreeSet::new();
        for package in layers.iter().filter_map(Layer::package) {
            if !packages.insert(package) {
                return Err(ConfigError::DuplicatePackage(package.to_owned()));
            }
        }
        let mut ids = BTreeSet::new();
        for rule in &settings.rules {
            if !ids.insert(rule.id.as_str()) {
                return Err(ConfigError::DuplicateRule(rule.id.clone()));
            }
        }
        if settings.disable_key.is_empty() {
            return Err(ConfigError::EmptyDisableKey);
        }
        if settings.semantic.enabled && settings.semantic.model_path.is_none() {
            return Err(ConfigError::MissingModel);
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
}

/// A compiled rule's identity and declared metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RuleMeta {
    /// The rule id.
    pub id: String,
    /// The module that defines it.
    pub module: String,
    /// Its effective severity.
    pub severity: Severity,
    /// Raw values of the caller's declared meta keys.
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
    /// The rule id.
    pub rule: String,
    /// The module that defines it.
    pub module: String,
    /// The example's line in the module.
    pub line: u32,
    /// What the example expects.
    pub expect: Expect,
    /// The example's Markdown.
    pub text: String,
    /// Data tables the example sets.
    pub data: BTreeMap<String, Table>,
}

/// The result of running one example.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The example held.
    Pass,
    /// The example failed.
    Fail {
        /// Findings produced.
        found: usize,
        /// What went wrong beyond the count, if anything.
        problem: Option<String>,
    },
}

/// Which rules a plan runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// Every rule.
    All,
    /// Only these ids.
    Only(BTreeSet<String>),
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
pub struct Corpus {
    /// The files, each a file node under the root.
    pub tree: Tree,
    /// Precomputed targets, typed values, params and data.
    pub environment: Environment,
}

/// Rules compiled from a configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    rules: Vec<RuleMeta>,
    examples: Vec<Example>,
}

impl Plan {
    /// The compiled rules.
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

/// Why compiling failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompileError {
    /// Two layers define one module name.
    #[error("module {0:?} is defined twice")]
    DuplicateModule(String),
    /// A module's frontmatter has a key neither tela nor the caller declares.
    #[error("module {module:?}: unknown frontmatter key {key:?}")]
    UnknownMetaKey {
        /// The module.
        module: String,
        /// The key.
        key: String,
    },
    /// A rule's query did not read.
    #[error("module {module:?}, rule {rule:?}: {error}")]
    Query {
        /// The module.
        module: String,
        /// The rule.
        rule: String,
        /// The reader's error.
        error: query::Error,
    },
    /// The semantic model did not load.
    #[error("semantic model {model:?}: {message}")]
    Model {
        /// The model path.
        model: String,
        /// Why it failed.
        message: String,
    },
}

/// Compiles the layers' modules, tela's std modules and inline rules.
pub type CompileFn = fn(config: &Config) -> Result<Plan, CompileError>;

/// Runs the selected rules over a corpus.
pub type RunFn =
    fn(plan: &Plan, corpus: &Corpus, rules: &Selection) -> Result<Vec<Diagnostic>, query::RunError>;

/// Runs one example against a corpus the caller built from it.
pub type RunExampleFn = fn(plan: &Plan, example: &Example, corpus: &Corpus) -> Outcome;

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(package: &str) -> Layer {
        Layer::from_files(Some(package.to_owned()), vec![]).unwrap()
    }

    fn now() -> SystemTime {
        SystemTime::UNIX_EPOCH
    }

    #[test]
    fn severities_round_trip_their_names() {
        for severity in [Severity::Error, Severity::Warning] {
            assert_eq!(severity.to_string().parse(), Ok(severity));
        }
        assert!("info".parse::<Severity>().is_err());
    }

    #[test]
    fn two_layers_of_one_package_are_rejected() {
        let result = Config::new(
            vec![layer("team"), layer("team")],
            Spec::new(now()),
            Settings::default(),
        );
        assert_eq!(
            result,
            Err(ConfigError::DuplicatePackage("team".to_owned()))
        );
    }

    #[test]
    fn semantic_rules_need_a_model() {
        let settings = Settings {
            semantic: Semantic {
                enabled: true,
                model_path: None,
            },
            ..Settings::default()
        };
        assert_eq!(
            Config::new(vec![], Spec::new(now()), settings),
            Err(ConfigError::MissingModel)
        );
    }

    #[test]
    fn layers_reject_duplicate_files() {
        let file = ModuleFile {
            path: "headings.md".to_owned(),
            text: String::new(),
        };
        assert!(matches!(
            Layer::from_files(None, vec![file.clone(), file]),
            Err(LayerError::DuplicateFile(path)) if path == "headings.md"
        ));
    }

    #[test]
    fn read_dir_collects_markdown_with_relative_paths() {
        let dir = std::env::temp_dir().join(format!("tela-layer-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("team")).unwrap();
        std::fs::write(dir.join("team/headings.md"), "# Headings\n").unwrap();
        std::fs::write(dir.join("notes.txt"), "skip").unwrap();
        let layer = Layer::read_dir(None, &dir).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(layer.files().len(), 1);
        assert_eq!(layer.files()[0].path, "team/headings.md");
    }

    #[test]
    fn diagnostics_need_a_rule() {
        let at = Location {
            file: "a.md".to_owned(),
            line: 14,
            column: 1,
        };
        assert_eq!(
            Diagnostic::new("", at.clone(), Severity::Warning, "x"),
            Err(UnnamedRule)
        );
        let d = Diagnostic::new(
            "duplicate-heading",
            at,
            Severity::Warning,
            "heading repeated",
        )
        .unwrap();
        assert_eq!((d.rule(), d.location().line), ("duplicate-heading", 14));
    }

    #[test]
    fn selections_filter_rule_ids() {
        let only = Selection::Only(BTreeSet::from(["a".to_owned()]));
        assert!(only.includes("a"));
        assert!(!only.includes("b"));
        assert!(Selection::All.includes("b"));
    }
}
