//! The lint engine: rule modules from ordered layers, compiled into a plan
//! that runs over one corpus and returns tela's own findings.

use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::query::{self, Environment, Table, Value};
use crate::tree::{Origin, Tree};

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
/// caller is strict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Always fails.
    Error,
    /// Fails only when strict.
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

/// The similarity behind a finding, with what its method reports.
#[derive(Debug, Clone, PartialEq)]
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
    pub id: RuleId,
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
    /// Parameter values, over the environment's.
    pub params: BTreeMap<String, Value>,
    /// Data tables, over the environment's.
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

/// The semantic model options of the `[lint.semantic]` table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Semantic {
    /// Whether semantic rules run.
    pub enabled: bool,
    /// The `Model2Vec` directory the caller loads into the environment.
    pub model_path: Option<PathBuf>,
}

/// Names the caller gives tela's sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Labels {
    /// The source name of inline rules.
    pub inline: String,
    /// The prefix of std module names.
    pub std: String,
}

impl Default for Labels {
    fn default() -> Self {
        Self {
            inline: "tela.toml".to_owned(),
            std: "std".to_owned(),
        }
    }
}

/// What the calling program declares: its extra module frontmatter keys and
/// its labels.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Spec {
    /// Module frontmatter keys beyond tela's own, returned raw per rule.
    pub meta_keys: BTreeSet<String>,
    /// Source labels.
    pub labels: Labels,
}

/// What a user configures: the `[lint]` table.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Inline rules.
    pub rules: Vec<InlineRule>,
    /// Per-rule overrides.
    pub config: BTreeMap<RuleId, RuleConfig>,
    /// Disabled rules.
    pub disable: BTreeSet<RuleId>,
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
    #[error("inline rule {0} is declared twice")]
    DuplicateRule(RuleId),
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
            if !ids.insert(&rule.id) {
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
    pub id: RuleId,
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
pub struct Corpus {
    /// The files, each a file node under the root.
    pub tree: Tree,
    /// Targets, typed values, params, data, model and clock.
    pub environment: Environment,
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
    #[error("module {module:?}, rule {rule}")]
    Query {
        /// The module.
        module: String,
        /// The rule.
        rule: RuleId,
        /// The reader's error.
        source: query::Error,
    },
}

/// Rules compiled from a configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    rules: Vec<RuleMeta>,
    examples: Vec<Example>,
}

impl Plan {
    /// Compiles the layers' modules, tela's std modules and the inline rules.
    ///
    /// # Errors
    /// Returns [`CompileError`] when a module is defined twice, declares an
    /// unknown frontmatter key, or holds a query that does not read.
    #[expect(
        clippy::todo,
        reason = "the compiler is ported from henia's lint package"
    )]
    pub fn compile(config: &Config) -> Result<Self, CompileError> {
        let _ = config;
        todo!()
    }

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

    /// Runs the selected rules over `corpus`.
    ///
    /// # Errors
    /// Returns [`query::RunError`] when a rule's query fails to match.
    #[expect(
        clippy::todo,
        reason = "the runner is ported from henia's lint package"
    )]
    pub fn run(
        &self,
        corpus: &Corpus,
        rules: &Selection,
    ) -> Result<Vec<Diagnostic>, query::RunError> {
        let _ = (corpus, rules);
        todo!()
    }

    /// Runs one example against a corpus the caller built from it.
    #[must_use]
    #[expect(
        clippy::todo,
        reason = "the example runner is ported from henia's lint package"
    )]
    pub fn run_example(&self, example: &Example, corpus: &Corpus) -> Outcome {
        let _ = (example, corpus);
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(id: &str) -> RuleId {
        RuleId::new(id).unwrap()
    }

    fn layer(package: &str) -> Layer {
        Layer::from_files(Some(package.to_owned()), vec![]).unwrap()
    }

    #[test]
    fn rule_ids_reject_empty_and_spaced_text() {
        assert!(RuleId::new("").is_err());
        assert!(RuleId::new("two words").is_err());
        assert_eq!(
            "duplicate-heading".parse::<RuleId>().map(|r| r.to_string()),
            Ok("duplicate-heading".to_owned())
        );
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
            Spec::default(),
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
            Config::new(vec![], Spec::default(), settings),
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
    fn diagnostics_point_at_origins() {
        let at = Origin::new(
            "a.md",
            Some(crate::tree::Position::new(0..5, 14..=14, 1).unwrap()),
        )
        .unwrap();
        let d = Diagnostic::new(
            id("duplicate-heading"),
            at,
            Severity::Warning,
            "heading repeated",
        );
        assert_eq!(
            (d.rule().as_str(), d.origin().line()),
            ("duplicate-heading", Some(14))
        );
    }

    #[test]
    fn scores_carry_their_method_data() {
        let score = Score::Cosine {
            value: 0.98,
            model: "potion-base-2M".to_owned(),
        };
        assert!((score.value() - 0.98).abs() < f64::EPSILON);
    }

    #[test]
    fn selections_filter_rule_ids() {
        let only = Selection::Only(BTreeSet::from([id("a")]));
        assert!(only.includes("a"));
        assert!(!only.includes("b"));
        assert!(Selection::All.includes("b"));
    }
}
