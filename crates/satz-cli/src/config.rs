//! `satz.toml`: the `[lint]` table, read into [`Settings`] and the
//! semantic model's directory.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use satz::lint::{InlineRule, RuleConfig, RuleId, Settings, Severity};
use satz::query::{Table, Value};
use satz::similarity::Model2Vec;
use serde::Deserialize;

use crate::failure::Failure;

pub(crate) const DEFAULT: &str = "satz.toml";

/// What `satz.toml` configures.
#[derive(Debug, Default)]
pub(crate) struct Config {
    /// The lint settings.
    pub(crate) settings: Settings,
    /// `[lint.semantic] model_path`, resolved against the config file's
    /// directory.
    pub(crate) model: Option<PathBuf>,
}

/// Where the config comes from: the file `--config` names, which must exist,
/// or `satz.toml` in the current directory, if there is one.
#[derive(Debug)]
pub(crate) struct Source {
    named: Option<PathBuf>,
}

impl Source {
    pub(crate) fn new(named: Option<PathBuf>) -> Self {
        Self { named }
    }

    /// What the config file sets, or the defaults without one.
    pub(crate) fn load(&self) -> Result<Config, Failure> {
        let path = self.named.as_deref().unwrap_or(Path::new(DEFAULT));
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound && self.named.is_none() => {
                return Ok(Config::default());
            }
            Err(error) => return Err(Failure::from(error).context(path.display())),
        };
        let mut config = parse(&text).map_err(|failure| failure.context(path.display()))?;
        if let Some(model) = &mut config.model {
            let dir = path.parent().unwrap_or(Path::new(""));
            *model = resolve(model, dir, |name| env::var(name).ok(), env::home_dir());
        }
        Ok(config)
    }
}

/// The `Model2Vec` model in `dir`.
pub(crate) fn model(dir: &Path) -> Result<Model2Vec, Failure> {
    if !dir.is_dir() {
        return Err(Failure::new(format!(
            "semantic model {}: no such directory",
            dir.display()
        )));
    }
    Model2Vec::load(dir).map_err(|error| Failure::chain(&error))
}

/// `path` with `$VAR` and `${VAR}` expanded, an unset variable as empty
/// text, then a leading `~/` as `home` and a relative path under `dir`.
fn resolve(
    path: &Path,
    dir: &Path,
    var: impl Fn(&str) -> Option<String>,
    home: Option<PathBuf>,
) -> PathBuf {
    let expanded = expand(&path.to_string_lossy(), var);
    match (expanded.strip_prefix("~/"), home) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => dir.join(expanded),
    }
}

/// `text` with `$VAR` and `${VAR}` replaced by `var`'s values.
fn expand(text: &str, var: impl Fn(&str) -> Option<String>) -> String {
    let mut expanded = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(dollar) = rest.find('$') {
        expanded.push_str(&rest[..dollar]);
        let after = &rest[dollar + 1..];
        let (name, next) = if let Some(braced) = after.strip_prefix('{') {
            braced.split_once('}').unwrap_or(("", after))
        } else {
            let end = after
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(after.len());
            after.split_at(end)
        };
        if name.is_empty() {
            expanded.push('$');
        } else {
            expanded.push_str(&var(name).unwrap_or_default());
        }
        rest = next;
    }
    expanded.push_str(rest);
    expanded
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    lint: Lint,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Lint {
    rules: Vec<Rule>,
    config: BTreeMap<String, toml::Table>,
    disable: Vec<String>,
    disable_key: Option<String>,
    semantic: SemanticTable,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule {
    id: String,
    query: String,
    message: String,
    severity: Option<String>,
    at: Option<String>,
    related: Option<String>,
    #[serde(default)]
    params: toml::Table,
    #[serde(default)]
    data: toml::Table,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct SemanticTable {
    enabled: bool,
    model_path: Option<PathBuf>,
}

/// What `text` configures: TOML scalars become params, booleans as `true`
/// or `false` text, and tables and arrays become data tables.
fn parse(text: &str) -> Result<Config, Failure> {
    let file: File =
        toml::from_str(text).map_err(|error| Failure::new(error.to_string().trim_end()))?;
    let lint = file.lint;
    let mut settings = Settings::default();
    for (id, table) in lint.config {
        let rule = rule_id(&id)?;
        let overrides = rule_config(table).map_err(|f| f.context(format!("[lint.config.{id}]")))?;
        settings.config.insert(rule, overrides);
    }
    settings.rules = lint
        .rules
        .into_iter()
        .map(Rule::into_inline)
        .collect::<Result<_, _>>()?;
    settings.disable = lint
        .disable
        .iter()
        .map(|id| rule_id(id))
        .collect::<Result<_, _>>()?;
    if let Some(key) = lint.disable_key {
        settings.disable_key = key;
    }
    settings.semantic = lint.semantic.enabled;
    Ok(Config {
        settings,
        model: lint.semantic.model_path,
    })
}

impl Rule {
    fn into_inline(self) -> Result<InlineRule, Failure> {
        let context = format!("[[lint.rules]] {}", self.id);
        let rule = || -> Result<InlineRule, Failure> {
            let mut params = BTreeMap::new();
            for (key, value) in self.params {
                let value = scalar(&value)
                    .ok_or_else(|| Failure::new(format!("param {key} must be a scalar")))?;
                params.insert(key, value);
            }
            let mut data = BTreeMap::new();
            for (key, value) in self.data {
                let table = table(&value).ok_or_else(|| {
                    Failure::new(format!("data {key} must be a table or an array of scalars"))
                })?;
                data.insert(key, table);
            }
            let mut rule = InlineRule::new(rule_id(&self.id)?, self.query, self.message);
            rule.severity = self.severity.as_deref().map(severity).transpose()?;
            rule.at = self.at;
            rule.related = self.related;
            rule.params = params;
            rule.data = data;
            Ok(rule)
        };
        rule().map_err(|failure| failure.context(context))
    }
}

fn rule_config(rule: toml::Table) -> Result<RuleConfig, Failure> {
    let mut config = RuleConfig::default();
    for (key, value) in rule {
        if key == "severity" {
            let name = value
                .as_str()
                .ok_or_else(|| Failure::new("severity must be text"))?;
            config.severity = Some(severity(name)?);
        } else if let Some(param) = scalar(&value) {
            config.params.insert(key, param);
        } else {
            let table = table(&value).ok_or_else(|| {
                Failure::new(format!("data {key} must be a table or an array of scalars"))
            })?;
            config.data.insert(key, table);
        }
    }
    Ok(config)
}

fn rule_id(id: &str) -> Result<RuleId, Failure> {
    RuleId::new(id).map_err(|error| Failure::chain(&error))
}

fn severity(name: &str) -> Result<Severity, Failure> {
    name.parse().map_err(|error| Failure::chain(&error))
}

fn scalar(value: &toml::Value) -> Option<Value> {
    match value {
        toml::Value::Integer(n) => Some(Value::Number(integer(*n))),
        toml::Value::Float(n) => Some(Value::Number(*n)),
        toml::Value::Array(_) | toml::Value::Table(_) => None,
        other => text(other).map(Value::Text),
    }
}

#[expect(clippy::cast_precision_loss, reason = "query numbers are f64")]
fn integer(n: i64) -> f64 {
    n as f64
}

fn table(value: &toml::Value) -> Option<Table> {
    match value {
        toml::Value::Table(rows) => rows
            .iter()
            .map(|(key, value)| Some((key.clone(), text(value)?)))
            .collect::<Option<_>>()
            .map(Table::Map),
        toml::Value::Array(rows) => rows
            .iter()
            .map(text)
            .collect::<Option<_>>()
            .map(Table::List),
        _ => None,
    }
}

fn text(value: &toml::Value) -> Option<String> {
    match value {
        toml::Value::String(text) => Some(text.clone()),
        toml::Value::Integer(n) => Some(n.to_string()),
        toml::Value::Float(n) => Some(n.to_string()),
        toml::Value::Boolean(flag) => Some(flag.to_string()),
        toml::Value::Datetime(datetime) => Some(datetime.to_string()),
        toml::Value::Array(_) | toml::Value::Table(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_config_splits_params_and_data() {
        let settings = parse(
            r#"
            [lint]
            disable = ["duplicate-heading"]
            disable_key = "henia.lint.disable"

            [lint.config.similar-content]
            severity = "error"
            similarity = 0.3
            strict = true

            [lint.config.outdated-reference.outdated]
            zeta = "z"
            alpha = "a"
            "#,
        )
        .unwrap()
        .settings;
        assert_eq!(settings.disable_key, "henia.lint.disable");
        assert!(settings.disable.contains("duplicate-heading"));
        let similar = &settings.config["similar-content"];
        assert_eq!(similar.severity, Some(Severity::Error));
        assert_eq!(similar.params["similarity"], Value::Number(0.3));
        assert_eq!(similar.params["strict"], Value::Text("true".into()));
        assert_eq!(
            settings.config["outdated-reference"].data["outdated"],
            Table::Map(vec![
                ("zeta".into(), "z".into()),
                ("alpha".into(), "a".into())
            ])
        );
    }

    #[test]
    fn defaults_without_lint_table() {
        let config = parse("").unwrap();
        assert_eq!(config.settings.disable_key, "satz.lint.disable");
        assert!(!config.settings.semantic);
        assert_eq!(config.model, None);
    }

    #[test]
    fn model_paths_expand_variables_home_and_the_config_directory() {
        let var = |name: &str| (name == "MODELS").then(|| "/srv/models".to_owned());
        let home = Some(PathBuf::from("/home/me"));
        let dir = Path::new("conf");
        let resolved = |path: &str| resolve(Path::new(path), dir, var, home.clone());
        assert_eq!(resolved("$MODELS/potion"), Path::new("/srv/models/potion"));
        assert_eq!(
            resolved("${MODELS}/potion"),
            Path::new("/srv/models/potion")
        );
        assert_eq!(
            resolved("~/models/potion"),
            Path::new("/home/me/models/potion")
        );
        assert_eq!(resolved("models/potion"), Path::new("conf/models/potion"));
        assert_eq!(resolved("/abs/potion"), Path::new("/abs/potion"));
        assert_eq!(resolved("$UNSET/potion"), Path::new("/potion"));
        assert_eq!(resolved("a$/b"), Path::new("conf/a$/b"));
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let failure = parse("[lint]\nstage = \"source\"\n").unwrap_err();
        assert!(
            failure.to_string().contains("unknown field `stage`"),
            "{failure}"
        );
    }

    #[test]
    fn inline_rules_type_params_and_data() {
        let settings = parse(
            r#"
            [[lint.rules]]
            id = "short"
            query = "(paragraph :words (< $min)) @p"
            message = "short"
            severity = "warning"
            params = { min = 3 }
            data = { words = ["a", "b"] }
            "#,
        )
        .unwrap()
        .settings;
        let rule = &settings.rules[0];
        assert_eq!(rule.params["min"], Value::Number(3.0));
        assert_eq!(
            rule.data["words"],
            Table::List(vec!["a".into(), "b".into()])
        );
        assert_eq!(rule.severity, Some(Severity::Warning));
    }
}
